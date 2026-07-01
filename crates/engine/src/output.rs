//! cpal output stream backed by an rtrb ring-buffer consumer.
//!
//! Real-time discipline: the audio callback ONLY pops f32 samples from the
//! `rtrb::Consumer`.  It must not allocate, lock, block, or log.  On
//! underrun it writes silence (0.0).
//!
//! The callback also pushes a downsampled/mono copy into a second lock-free
//! viz ring (for the spectrum analyzer).  The push is skip-if-full — RT safe.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, HostId, Stream, StreamConfig};
use rtrb::{Consumer, Producer};

use crate::Error;

/// Size of the ring buffer in samples (interleaved f32).
/// 48000 Hz * 2 ch * 2 s = 192 000.  Round up to next power of two: 262 144.
pub(crate) const RING_BUFFER_SAMPLES: usize = 1 << 18; // 262 144

/// Resolved output device + configuration chosen at `Engine::new()`.
pub(crate) struct OutputDevice {
    pub device: Device,
    pub config: StreamConfig,
    pub sample_rate: u32,
    pub channels: u16,
}

impl OutputDevice {
    /// Open the preferred PipeWire output device, falling back to the default
    /// host if PipeWire is not available.
    pub(crate) fn open() -> crate::Result<Self> {
        let device = Self::best_device()?;
        let supported = device
            .default_output_config()
            .map_err(|e| Error::Device(e.to_string()))?;

        let config = supported.config();
        let sample_rate = config.sample_rate;
        let channels = config.channels;

        Ok(Self { device, config, sample_rate, channels })
    }

    fn best_device() -> crate::Result<Device> {
        // Prefer the PipeWire host; fall back to whatever is available.
        if let Ok(host) = cpal::host_from_id(HostId::PipeWire) {
            if let Some(dev) = host.default_output_device() {
                return Ok(dev);
            }
        }

        // Generic fallback.
        let host = cpal::default_host();
        host.default_output_device()
            .ok_or_else(|| Error::Device("no output device available".into()))
    }
}

/// Build and play a cpal output stream that pops f32 samples from `consumer`.
///
/// The callback is RT-safe: it only calls `consumer.pop()` (lock-free,
/// wait-free) and fills the output buffer.
///
/// `frames_played` is incremented by the number of frames **actually popped
/// from the ring buffer** (not silence-fill frames).  This keeps the reported
/// position aligned with audible output: silence-fill is buffer underrun, not
/// real content, so it must not advance the position counter.
///
/// When `flushing` is `true`, the callback pops and discards all available
/// samples from the ring buffer (draining stale pre-seek audio) and outputs
/// silence.  It does NOT advance `frames_played` during a flush.
///
/// `viz_producer` — if provided, the callback taps the played (post-gain)
/// audio into this SPSC ring as mono (averaged across channels) samples.
/// The push is skip-if-full (non-blocking) so the RT discipline is preserved.
pub(crate) fn build_output_stream(
    out: &OutputDevice,
    mut consumer: Consumer<f32>,
    frames_played: Arc<AtomicU64>,
    flushing: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    channels: u16,
    volume: Arc<AtomicU32>,
    decode_done: Arc<AtomicBool>,
    finished: Arc<AtomicBool>,
    mut viz_producer: Option<Producer<f32>>,
) -> crate::Result<Stream> {
    let ch = channels as u64;
    let ch_usize = channels as usize;

    let stream = out
        .device
        .build_output_stream(
            out.config.clone(),
            // ---- RT callback: no alloc / lock / IO / log ----
            move |buf: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                // When paused: output silence immediately and return WITHOUT
                // draining the ring buffer.  This makes pause take effect now
                // (instead of after the ~3 s ring buffer finishes playing), and
                // because the ring is left intact, resume continues seamlessly.
                // The position counter is not advanced while paused.
                if paused.load(Ordering::Acquire) {
                    for sample in buf.iter_mut() {
                        *sample = 0.0;
                    }
                    return;
                }

                // When flushing (seek in progress): drain the ring buffer to
                // clear stale audio, then output silence.  Do not advance the
                // position counter during a flush.
                if flushing.load(Ordering::Acquire) {
                    // Pop-and-discard whatever is available.
                    while consumer.pop().is_ok() {}
                    for sample in buf.iter_mut() {
                        *sample = 0.0;
                    }
                    return;
                }

                // Load volume once per callback — RT-safe atomic read, no alloc/lock.
                let gain = f32::from_bits(volume.load(Ordering::Relaxed));
                let mut real_samples: u64 = 0;
                for sample in buf.iter_mut() {
                    match consumer.pop() {
                        Ok(s) => {
                            *sample = s * gain;
                            real_samples += 1;
                        }
                        Err(_) => {
                            // Underrun: fill with silence; do NOT advance counter.
                            *sample = 0.0;
                        }
                    }
                }
                // Accumulate frames (interleaved samples / channel count).
                if ch > 0 && real_samples > 0 {
                    frames_played.fetch_add(real_samples / ch, Ordering::Relaxed);
                }

                // End-of-track detection: the decode thread has finished
                // producing (decode_done) AND the ring buffer is fully drained
                // (no real samples this callback). Flag it so the UI can
                // auto-advance the queue. RT-safe: plain atomic load/store.
                if real_samples == 0 && decode_done.load(Ordering::Relaxed) {
                    finished.store(true, Ordering::Relaxed);
                }

                // ── Viz tap (RT-safe: lock-free push, skip-if-full) ───────────
                // Downsample to mono: push one averaged sample per output frame.
                // If the viz ring is full, silently skip — the analyzer is
                // non-critical and only needs recent samples.
                if let Some(ref mut vp) = viz_producer {
                    let frames = if ch_usize > 0 { buf.len() / ch_usize } else { 0 };
                    for frame_idx in 0..frames {
                        let offset = frame_idx * ch_usize;
                        let mut mono = 0.0f32;
                        for c in 0..ch_usize {
                            mono += buf[offset + c];
                        }
                        if ch_usize > 0 {
                            mono /= ch_usize as f32;
                        }
                        // push() returns Err if full — we just discard (skip-if-full).
                        let _ = vp.push(mono);
                    }
                }
            },
            // ---- error callback (called from a non-RT context) ----
            |err| {
                // eprintln is not RT-safe but this callback is called from
                // an error-handling path, not the hot audio path.
                eprintln!("[lyra-engine] cpal stream error: {err}");
            },
            None, // no timeout
        )
        .map_err(|e| Error::Stream(e.to_string()))?;

    stream.play().map_err(|e| Error::Stream(e.to_string()))?;
    Ok(stream)
}
