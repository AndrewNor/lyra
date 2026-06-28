//! Decode thread: SymphoniaDecoder → optional resample → rtrb::Producer.
//!
//! This runs on a dedicated OS thread (not the audio callback thread).
//! It is allowed to allocate and may park briefly when the ring buffer is full.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use lyra_decoder::{Decoder, SymphoniaDecoder};
use rtrb::Producer;

use crate::Error;

/// Handle to the decode thread.  Dropping this signals the thread to stop and
/// joins it (best-effort; the thread may still be parking on a full buffer but
/// will exit once it wakes).
pub(crate) struct DecodeThread {
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl DecodeThread {
    /// Spawn a decode thread that:
    /// 1. Opens `path` with `SymphoniaDecoder`.
    /// 2. If the file's sample rate ≠ `device_rate`, resamples to `device_rate`
    ///    via `lyra_dsp::resample`.
    /// 3. If the file's channel count ≠ `device_channels`, adapts (upmix mono
    ///    to stereo by duplicating; mixdown N→stereo by summing first two
    ///    channels; or truncate/pad to match).
    /// 4. Pushes interleaved f32 into `producer`, parking briefly when full.
    /// 5. Exits when `stop_flag` is set, `paused_flag` causes it to spin-wait,
    ///    or the file is exhausted.
    pub(crate) fn spawn(
        path: &Path,
        producer: Producer<f32>,
        device_rate: u32,
        device_channels: u16,
        stop_flag: Arc<AtomicBool>,
        paused_flag: Arc<AtomicBool>,
    ) -> Result<Self, Error> {
        // Open the decoder eagerly on the calling thread so open errors
        // propagate to Engine::play() synchronously.
        let mut decoder = SymphoniaDecoder::open(path)
            .map_err(|e| Error::Decode(e.to_string()))?;

        let file_spec = decoder.spec();
        let file_rate = file_spec.sample_rate;
        let file_channels = file_spec.channels;

        let stop = Arc::clone(&stop_flag);
        let paused = paused_flag;

        let handle = thread::Builder::new()
            .name("lyra-decode".into())
            .spawn(move || {
                decode_loop(
                    &mut decoder,
                    producer,
                    file_rate,
                    file_channels,
                    device_rate,
                    device_channels,
                    &stop,
                    &paused,
                );
            })
            .map_err(|e| Error::Thread(e.to_string()))?;

        Ok(Self { stop_flag, handle: Some(handle) })
    }

    /// Signal the decode thread to stop and join it (non-blocking best-effort).
    pub(crate) fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            // Ignore join errors.
            let _ = h.join();
        }
    }
}

impl Drop for DecodeThread {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The body of the decode thread.
fn decode_loop(
    decoder: &mut SymphoniaDecoder,
    mut producer: Producer<f32>,
    file_rate: u32,
    file_channels: u16,
    device_rate: u32,
    device_channels: u16,
    stop: &AtomicBool,
    paused: &AtomicBool,
) {
    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // Spin-wait when paused.
        if paused.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        // Decode one packet from the file.
        let chunk = match decoder.next_chunk() {
            Ok(Some(c)) => c,
            Ok(None) => break, // EOF
            Err(e) => {
                eprintln!("[lyra-engine] decode error: {e}");
                break;
            }
        };

        // Resample if the file rate differs from the device rate.
        let chunk = if file_rate != device_rate {
            match lyra_dsp::resample(&chunk, file_channels, file_rate, device_rate) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[lyra-engine] resample error: {e}");
                    break;
                }
            }
        } else {
            chunk
        };

        // Adapt channel count if necessary.
        let chunk = adapt_channels(&chunk, file_channels, device_channels);

        // Push into the ring buffer; park briefly when full so we don't spin.
        push_all(&mut producer, &chunk, stop);
    }
}

/// Adapt an interleaved f32 buffer from `from_ch` channels to `to_ch` channels.
///
/// - mono → stereo: duplicate the single sample per frame.
/// - N → 1:  take only the first channel per frame.
/// - N → M where N ≠ M: copy min(N, M) channels per frame, zero-pad the rest.
/// - N == M: return as-is (zero-copy path).
fn adapt_channels(input: &[f32], from_ch: u16, to_ch: u16) -> Vec<f32> {
    if from_ch == to_ch {
        return input.to_vec();
    }

    let from = from_ch as usize;
    let to = to_ch as usize;
    let frames = if from > 0 { input.len() / from } else { 0 };
    let mut out = Vec::with_capacity(frames * to);

    for frame in input.chunks_exact(from) {
        for ch in 0..to {
            if ch < from {
                out.push(frame[ch]);
            } else if from == 1 {
                // mono → stereo (and beyond): duplicate the mono sample.
                out.push(frame[0]);
            } else {
                out.push(0.0);
            }
        }
    }
    out
}

/// Push all samples into the producer.  When the ring buffer is full, sleep
/// briefly and retry — but bail out if the stop flag is set.
fn push_all(producer: &mut Producer<f32>, samples: &[f32], stop: &AtomicBool) {
    let mut cursor = 0;
    while cursor < samples.len() {
        if stop.load(Ordering::Relaxed) {
            return;
        }

        match producer.push(samples[cursor]) {
            Ok(()) => {
                cursor += 1;
            }
            Err(_full) => {
                // Buffer is full — give the audio callback time to drain it.
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
