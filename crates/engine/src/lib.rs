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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::StreamTrait;
use rtrb::RingBuffer;
use thiserror::Error;

use decode_loop::DecodeThread;
use output::{OutputDevice, RING_BUFFER_SAMPLES, build_output_stream};

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

        // Fresh stop/pause flags.
        let stop_flag = Arc::new(AtomicBool::new(false));
        let paused_flag = Arc::new(AtomicBool::new(false));

        // Allocate the ring buffer.
        let (producer, consumer) = RingBuffer::<f32>::new(RING_BUFFER_SAMPLES);

        // Build and start the cpal output stream (consumer end).
        let stream = build_output_stream(&self.out, consumer)?;

        // Spawn the decode thread (producer end).
        let decode_thread = DecodeThread::spawn(
            path,
            producer,
            self.out.sample_rate,
            self.out.channels,
            Arc::clone(&stop_flag),
            Arc::clone(&paused_flag),
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
