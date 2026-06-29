//! Real-time playback engine: cpal/PipeWire output stream fed from a decode
//! thread via an `rtrb` lock-free ring buffer.
//!
//! # Real-time discipline
//! The cpal audio callback (see `output.rs`) is strictly real-time safe:
//! it only pops `f32` samples from an `rtrb::Consumer`.  All decoding, DSP,
//! and resampling happen on a dedicated OS thread (`decode_loop.rs`).

mod decode_loop;
mod output;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::StreamTrait;
use rtrb::RingBuffer;
use thiserror::Error;

use decode_loop::DecodeThread;
use output::{OutputDevice, RING_BUFFER_SAMPLES, build_output_stream};

/// Sentinel value for `seek_ms` meaning "no seek pending".
pub(crate) const SEEK_NONE: u64 = u64::MAX;

// ── Error type ──────────────────────────────────────────────────────────────

/// Errors produced by `lyra-engine`.
#[derive(Debug, Error)]
pub enum Error {
    #[error("audio device error: {0}")]
    Device(String),

    #[error("stream error: {0}")]
    Stream(String),

    #[error("decoder error: {0}")]
    Decode(String),

    #[error("thread spawn error: {0}")]
    Thread(String),
}

/// Shorthand `Result` for `lyra-engine`.
pub type Result<T> = std::result::Result<T, Error>;

// ── Public API ───────────────────────────────────────────────────────────────

/// Playback state of the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// The playback engine.
///
/// Opens a cpal/PipeWire output device at construction time.  Call
/// [`Engine::play`] to start playing a file.
pub struct Engine {
    /// The resolved output device + its default config.
    out: OutputDevice,

    /// Current playback state (not shared with RT thread — guarded by
    /// non-RT code only).
    state: PlaybackState,

    /// The live cpal stream.  `None` when stopped.
    stream: Option<cpal::Stream>,

    /// Handle to the decode thread.  `None` when stopped.
    decode_thread: Option<DecodeThread>,

    /// Shared flag: when set to `true` the decode thread exits.
    stop_flag: Arc<AtomicBool>,

    /// Shared flag: when set to `true` the decode thread spin-waits.
    paused_flag: Arc<AtomicBool>,

    /// Frames played counter — incremented in the RT callback using only
    /// samples actually popped from the ring buffer (not silence-fill).
    /// Reset to 0 at the start of every `play()`.
    frames_played: Arc<AtomicU64>,

    /// Seek target in milliseconds.  `SEEK_NONE` (u64::MAX) means no seek
    /// is pending.  Written by `seek()` on the UI thread; read by the decode
    /// thread and cleared once the seek completes.
    seek_ms: Arc<AtomicU64>,

    /// Seek generation counter — bumped each time a new seek is requested.
    /// Lets the decode thread detect a new seek even if the old one hasn't
    /// been cleared yet (shouldn't happen in practice, but defensive).
    seek_generation: Arc<AtomicU64>,

    /// Flushing flag: when `true` the RT callback discards ring-buffer
    /// contents and outputs silence.  Set by `seek()`, cleared by the decode
    /// thread once the seek is complete.
    flushing: Arc<AtomicBool>,
}

impl Engine {
    /// Open the default PipeWire (or system default) output device and
    /// pre-allocate the ring buffer.  Returns an error if no device is
    /// available.
    pub fn new() -> Result<Self> {
        let out = OutputDevice::open()?;

        Ok(Self {
            out,
            state: PlaybackState::Stopped,
            stream: None,
            decode_thread: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            paused_flag: Arc::new(AtomicBool::new(false)),
            frames_played: Arc::new(AtomicU64::new(0)),
            seek_ms: Arc::new(AtomicU64::new(SEEK_NONE)),
            seek_generation: Arc::new(AtomicU64::new(0)),
            flushing: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Return the current playback state.
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Start playing the audio file at `path`, replacing any current playback.
    pub fn play(&mut self, path: &Path) -> Result<()> {
        // Stop any existing playback cleanly.
        self.stop_internal();

        // Reset the position counter for the new track.
        self.frames_played.store(0, Ordering::Relaxed);

        // Reset seek state for the new track.
        self.seek_ms.store(SEEK_NONE, Ordering::Relaxed);
        self.flushing.store(false, Ordering::Relaxed);

        // Fresh stop/pause flags.
        let stop_flag = Arc::new(AtomicBool::new(false));
        let paused_flag = Arc::new(AtomicBool::new(false));

        // Allocate the ring buffer.
        let (producer, consumer) = RingBuffer::<f32>::new(RING_BUFFER_SAMPLES);

        // Build and start the cpal output stream (consumer end).
        let stream = build_output_stream(
            &self.out,
            consumer,
            Arc::clone(&self.frames_played),
            Arc::clone(&self.flushing),
            self.out.channels,
        )?;

        // Spawn the decode thread (producer end).
        let decode_thread = DecodeThread::spawn(
            path,
            producer,
            self.out.sample_rate,
            self.out.channels,
            Arc::clone(&stop_flag),
            Arc::clone(&paused_flag),
            Arc::clone(&self.seek_ms),
            Arc::clone(&self.seek_generation),
            Arc::clone(&self.flushing),
            Arc::clone(&self.frames_played),
        )?;

        self.stream = Some(stream);
        self.decode_thread = Some(decode_thread);
        self.stop_flag = stop_flag;
        self.paused_flag = paused_flag;
        self.state = PlaybackState::Playing;

        Ok(())
    }

    /// Pause playback.  The audio callback continues running but the decode
    /// thread will stall; the ring buffer drains to silence.
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.paused_flag.store(true, Ordering::Relaxed);
            self.state = PlaybackState::Paused;
        }
    }

    /// Resume a paused playback.
    pub fn resume(&mut self) {
        if self.state == PlaybackState::Paused {
            self.paused_flag.store(false, Ordering::Relaxed);
            self.state = PlaybackState::Playing;
        }
    }

    /// Stop playback and release resources.
    pub fn stop(&mut self) {
        self.stop_internal();
    }

    /// Return the current playback position in seconds.
    ///
    /// Computed from the frames-played atomic counter (incremented in the RT
    /// callback for samples actually popped from the ring buffer — silence-fill
    /// on underrun is excluded so the position stays aligned with audible audio).
    /// Returns 0.0 when stopped.
    pub fn position_secs(&self) -> f64 {
        let frames = self.frames_played.load(Ordering::Relaxed);
        let rate = self.out.sample_rate as f64;
        if rate > 0.0 { frames as f64 / rate } else { 0.0 }
    }

    /// Return the device sample rate in Hz.
    pub fn device_sample_rate(&self) -> u32 {
        self.out.sample_rate
    }

    /// Seek to `secs` seconds into the current track (best-effort, coarse).
    ///
    /// Signals the decode thread to seek via a shared atomic; the RT callback
    /// drains and silences the ring buffer while the seek is in flight.
    /// The actual seek happens asynchronously on the decode thread.
    /// No-op when stopped.
    pub fn seek(&mut self, secs: f64) -> Result<()> {
        if self.state == PlaybackState::Stopped {
            return Ok(());
        }

        let target_ms = (secs.max(0.0) * 1000.0) as u64;

        // Set the flushing flag FIRST so the RT callback starts discarding
        // stale audio immediately.
        self.flushing.store(true, Ordering::Release);

        // Store the seek target and bump the generation so the decode thread
        // notices even if it is currently processing a previous seek.
        self.seek_ms.store(target_ms, Ordering::Release);
        self.seek_generation.fetch_add(1, Ordering::Release);

        Ok(())
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn stop_internal(&mut self) {
        // Signal and join the decode thread first.
        if let Some(mut dt) = self.decode_thread.take() {
            dt.stop();
        }

        // Pause the cpal stream so the callback no longer runs.
        if let Some(stream) = &self.stream {
            let _ = stream.pause();
        }
        self.stream = None;

        self.stop_flag = Arc::new(AtomicBool::new(false));
        self.paused_flag = Arc::new(AtomicBool::new(false));
        self.seek_ms = Arc::new(AtomicU64::new(SEEK_NONE));
        self.seek_generation = Arc::new(AtomicU64::new(0));
        self.flushing = Arc::new(AtomicBool::new(false));
        self.state = PlaybackState::Stopped;
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Construct an `Engine` and verify the initial state is `Stopped`.
    ///
    /// If no audio device is available (e.g. headless CI) the test passes
    /// gracefully by detecting the "no output device" error and skipping.
    #[test]
    fn engine_initial_state_is_stopped() {
        match Engine::new() {
            Ok(engine) => {
                assert_eq!(engine.state(), PlaybackState::Stopped);
            }
            Err(Error::Device(_)) => {
                // No device available — acceptable in headless CI.
                eprintln!("[test] no audio device — skipping engine state test");
            }
            Err(other) => {
                panic!("unexpected Engine::new() error: {other}");
            }
        }
    }
}
