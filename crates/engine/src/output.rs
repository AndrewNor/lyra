//! cpal output stream backed by an rtrb ring-buffer consumer.
//!
//! Real-time discipline: the audio callback ONLY pops f32 samples from the
//! `rtrb::Consumer`.  It must not allocate, lock, block, or log.  On
//! underrun it writes silence (0.0).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, HostId, Stream, StreamConfig};
use rtrb::Consumer;

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
pub(crate) fn build_output_stream(
    out: &OutputDevice,
    mut consumer: Consumer<f32>,
) -> crate::Result<Stream> {
    let stream = out
        .device
        .build_output_stream(
            out.config.clone(),
            // ---- RT callback: no alloc / lock / IO / log ----
            move |buf: &mut [f32], _info: &cpal::OutputCallbackInfo| {
                for sample in buf.iter_mut() {
                    *sample = consumer.pop().unwrap_or(0.0);
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
