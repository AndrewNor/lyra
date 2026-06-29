//! Real-time playback engine: cpal/PipeWire output stream fed from a decode
//! thread via an `rtrb` lock-free ring buffer.
//!
//! # Real-time discipline
//! The cpal audio callback (see `output.rs`) is strictly real-time safe:
//! it only pops `f32` samples from an `rtrb::Consumer`.  All decoding, DSP,
//! and resampling happen on a dedicated OS thread (`decode_loop.rs`).

mod decode_loop;
mod output;
pub mod spectrum;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::StreamTrait;
use rtrb::RingBuffer;
use thiserror::Error;

use decode_loop::DecodeThread;
use output::{OutputDevice, RING_BUFFER_SAMPLES, build_output_stream};
use spectrum::{NUM_BANDS, SpectrumAnalyzerHandle, SpectrumLevels, start_analyzer};

// ── EQ configuration ─────────────────────────────────────────────────────────

/// 10-band graphic EQ center frequencies in Hz.
pub const EQ_FREQS_HZ: [f32; 10] = [31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0];

/// Q factor for all graphic EQ bands.
pub const EQ_Q: f32 = 1.0;

/// Shared EQ configuration, read by the decode thread.
#[derive(Debug, Clone)]
pub struct EqConfig {
    /// Whether the equalizer is active.
    pub enabled: bool,
    /// Per-band gain in dB, range −12..+12.  Index maps to `EQ_FREQS_HZ`.
    pub gains: [f32; 10],
}

impl Default for EqConfig {
    fn default() -> Self {
        Self { enabled: false, gains: [0.0; 10] }
    }
}

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

    /// Master volume gain, stored as f32 bits in an AtomicU32.
    /// Range 0.0..=1.0.  Default 1.0 (full volume).
    /// Written by `set_volume()` on the UI thread; read once per RT callback.
    volume: Arc<AtomicU32>,

    /// Shared EQ configuration (enabled flag + per-band gains).
    /// Written by UI thread; read by decode thread (mutex, not in RT callback).
    eq_config: Arc<Mutex<EqConfig>>,

    /// EQ generation counter — bumped whenever a band gain changes so the
    /// decode thread knows to rebuild its `Equalizer` instance.
    eq_generation: Arc<AtomicU64>,

    /// Bit-perfect mode flag.  When `true` the decode thread skips EQ
    /// (and skips resampling when rates match).
    bit_perfect: Arc<AtomicBool>,

    /// Crossfade window in seconds stored as f32 bits.  0.0 = disabled.
    /// Written by `set_crossfade_secs()` on the UI thread; read by the
    /// decode thread (single f32 load — no lock needed).
    crossfade_secs: Arc<AtomicU32>,

    /// Path of the next track to crossfade into.  Set by the Player when
    /// it knows what comes next; cleared by the decode thread once it has
    /// opened the next decoder and the crossfade begins.
    next_track_path: Arc<Mutex<Option<PathBuf>>>,

    /// Handle to the spectrum analyzer thread.  `None` when stopped.
    /// Replaced on each `play()`.
    spectrum_analyzer: Option<SpectrumAnalyzerHandle>,

    /// Shared 24-band levels (f32 bits in AtomicU32).  Created once in
    /// `Engine::new()` and reused across successive analyzer instances.
    spectrum_levels_store: SpectrumLevels,
}

impl Engine {
    /// Open the default PipeWire (or system default) output device and
    /// pre-allocate the ring buffer.  Returns an error if no device is
    /// available.
    pub fn new() -> Result<Self> {
        let out = OutputDevice::open()?;

        // Allocate the persistent spectrum levels store (zeroed).
        let spectrum_levels_store = {
            let arr: [AtomicU32; NUM_BANDS] = [
                AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
                AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
                AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
                AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
                AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
                AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0),
            ];
            Arc::new(arr)
        };

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
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            eq_config: Arc::new(Mutex::new(EqConfig::default())),
            eq_generation: Arc::new(AtomicU64::new(0)),
            bit_perfect: Arc::new(AtomicBool::new(false)),
            crossfade_secs: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            next_track_path: Arc::new(Mutex::new(None)),
            spectrum_analyzer: None,
            spectrum_levels_store,
        })
    }

    /// Return the current playback state.
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Start playing the audio file at `path`, replacing any current playback.
    pub fn play(&mut self, path: &Path) -> Result<()> {
        // Stop any existing playback cleanly (drops the old SpectrumAnalyzer too).
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

        // Spawn a fresh spectrum analyzer.  It writes into the shared levels store.
        // start_analyzer returns the SPSC producer (moved into the RT callback)
        // and a handle (stored in self for lifetime management).
        let (viz_producer, analyzer_handle) = start_analyzer(
            Arc::clone(&self.spectrum_levels_store),
            self.out.sample_rate,
            self.out.channels,
        );

        // Build and start the cpal output stream (consumer end).
        let stream = build_output_stream(
            &self.out,
            consumer,
            Arc::clone(&self.frames_played),
            Arc::clone(&self.flushing),
            self.out.channels,
            Arc::clone(&self.volume),
            Some(viz_producer),
        )?;

        self.spectrum_analyzer = Some(analyzer_handle);

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
            Arc::clone(&self.eq_config),
            Arc::clone(&self.eq_generation),
            Arc::clone(&self.bit_perfect),
            Arc::clone(&self.crossfade_secs),
            Arc::clone(&self.next_track_path),
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

    /// Set the master volume gain.  `v` is clamped to `0.0..=1.0`.
    /// Takes effect on the next audio callback (no latency guarantee).
    pub fn set_volume(&self, v: f32) {
        let clamped = v.clamp(0.0, 1.0);
        self.volume.store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// Return the current master volume gain (0.0..=1.0).
    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    /// Enable or disable the graphic equalizer.
    /// Takes effect on the decode thread within one chunk (no latency guarantee).
    pub fn set_eq_enabled(&self, enabled: bool) {
        if let Ok(mut cfg) = self.eq_config.lock() {
            cfg.enabled = enabled;
        }
        // Bump generation so the decode thread rebuilds the Equalizer.
        self.eq_generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Return whether the EQ is currently enabled.
    pub fn eq_enabled(&self) -> bool {
        self.eq_config.lock().map(|c| c.enabled).unwrap_or(false)
    }

    /// Set the gain for one EQ band.
    ///
    /// `band` is 0..10 (index into `EQ_FREQS_HZ`).
    /// `db` is clamped to −12..+12.
    /// Bumps the generation counter so the decode thread rebuilds its Equalizer.
    pub fn set_eq_gain(&self, band: usize, db: f32) {
        if band >= 10 {
            return;
        }
        let clamped = db.clamp(-12.0, 12.0);
        if let Ok(mut cfg) = self.eq_config.lock() {
            cfg.gains[band] = clamped;
        }
        self.eq_generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Return a snapshot of the current EQ gains ([f32; 10]).
    pub fn eq_gains(&self) -> [f32; 10] {
        self.eq_config.lock().map(|c| c.gains).unwrap_or([0.0; 10])
    }

    /// Reset all EQ band gains to 0 dB (flat).
    pub fn reset_eq(&self) {
        if let Ok(mut cfg) = self.eq_config.lock() {
            cfg.gains = [0.0; 10];
        }
        self.eq_generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Enable or disable bit-perfect mode.
    ///
    /// When enabled: EQ is bypassed, and resampling is skipped if the file
    /// sample rate matches the device rate.
    pub fn set_bit_perfect(&self, enabled: bool) {
        self.bit_perfect.store(enabled, Ordering::Relaxed);
    }

    /// Return whether bit-perfect mode is active.
    pub fn bit_perfect(&self) -> bool {
        self.bit_perfect.load(Ordering::Relaxed)
    }

    /// Set the crossfade window in seconds (0.0 = disabled, max clamped to 12 s).
    ///
    /// When > 0 and a next track path is known, the decode thread will begin
    /// mixing the outgoing and incoming tracks over this window using
    /// equal-power gain curves.  0 = OFF (default), normal gapless behaviour
    /// is fully preserved.
    pub fn set_crossfade_secs(&self, secs: f32) {
        let clamped = secs.clamp(0.0, 12.0);
        self.crossfade_secs.store(clamped.to_bits(), Ordering::Relaxed);
    }

    /// Return the current crossfade window in seconds (0.0 = disabled).
    pub fn crossfade_secs(&self) -> f32 {
        f32::from_bits(self.crossfade_secs.load(Ordering::Relaxed))
    }

    /// Inform the engine of the next track path so crossfade can be prepared.
    ///
    /// Call this whenever the play queue advances — typically right after
    /// `play()` starts a new track.  The decode thread reads this once it
    /// enters the crossfade window.  Pass `None` to clear (e.g. at end of queue).
    pub fn set_next_track_path(&self, path: Option<&Path>) {
        if let Ok(mut guard) = self.next_track_path.lock() {
            *guard = path.map(|p| p.to_path_buf());
        }
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

        // Drop the spectrum analyzer — this signals its thread to exit and joins.
        // Dropping after the stream is paused ensures the RT callback no longer
        // holds the viz producer (it was moved into the closure and dies with it).
        self.spectrum_analyzer = None;

        // Zero out all band levels so the visualizer rests at 0.
        for atom in self.spectrum_levels_store.iter() {
            atom.store(0, Ordering::Relaxed);
        }

        self.stop_flag = Arc::new(AtomicBool::new(false));
        self.paused_flag = Arc::new(AtomicBool::new(false));
        self.seek_ms = Arc::new(AtomicU64::new(SEEK_NONE));
        self.seek_generation = Arc::new(AtomicU64::new(0));
        self.flushing = Arc::new(AtomicBool::new(false));
        // Clear any pending next-track path so it doesn't leak into the next play.
        if let Ok(mut guard) = self.next_track_path.lock() {
            *guard = None;
        }
        self.state = PlaybackState::Stopped;
    }

    /// Return a snapshot of the 24 spectrum band levels (0.0..=1.0).
    /// Read from the shared AtomicU32 store; safe to call from any thread.
    pub fn spectrum_levels(&self) -> [f32; NUM_BANDS] {
        let mut out = [0.0f32; NUM_BANDS];
        for (i, atom) in self.spectrum_levels_store.iter().enumerate() {
            out[i] = f32::from_bits(atom.load(Ordering::Relaxed));
        }
        out
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
