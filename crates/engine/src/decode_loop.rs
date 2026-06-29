//! Decode thread: SymphoniaDecoder → optional EQ → optional resample → rtrb::Producer.
//!
//! This runs on a dedicated OS thread (not the audio callback thread).
//! It is allowed to allocate and may park briefly when the ring buffer is full.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use lyra_decoder::{Decoder, SymphoniaDecoder};
use lyra_dsp::{EqBand, Equalizer, StreamResampler};
use rtrb::Producer;

use crate::{EqConfig, Error, EQ_FREQS_HZ, EQ_Q, SEEK_NONE};

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
    /// 2. Unless `bit_perfect` is set AND file_rate == device_rate: applies
    ///    a 10-band graphic `Equalizer` on the file-rate PCM (rebuilt whenever
    ///    `eq_generation` changes), then resamples to `device_rate` via a
    ///    persistent `lyra_dsp::StreamResampler`.
    /// 3. If the file's channel count ≠ `device_channels`, adapts channels.
    /// 4. Pushes interleaved f32 into `producer`, parking briefly when full.
    /// 5. Exits when `stop_flag` is set, `paused_flag` causes it to spin-wait,
    ///    or the file is exhausted.
    /// 6. Responds to seek commands written to `seek_ms` by:
    ///    a. Seeking the FormatReader.
    ///    b. Waiting for the ring buffer to drain (RT callback outputs silence).
    ///    c. Immediately decoding and pushing a small pre-fill of audio into
    ///       the ring buffer *before* clearing `flushing`, so audio resumes
    ///       with minimal gap.
    ///    d. Updating `frames_played` and clearing `flushing`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn spawn(
        path: &Path,
        producer: Producer<f32>,
        device_rate: u32,
        device_channels: u16,
        stop_flag: Arc<AtomicBool>,
        paused_flag: Arc<AtomicBool>,
        seek_ms: Arc<AtomicU64>,
        seek_generation: Arc<AtomicU64>,
        flushing: Arc<AtomicBool>,
        frames_played: Arc<AtomicU64>,
        eq_config: Arc<Mutex<EqConfig>>,
        eq_generation: Arc<AtomicU64>,
        bit_perfect: Arc<AtomicBool>,
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
                    &seek_ms,
                    &seek_generation,
                    &flushing,
                    &frames_played,
                    &eq_config,
                    &eq_generation,
                    &bit_perfect,
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

// ── Seek pre-fill target ─────────────────────────────────────────────────────

/// How many ring-buffer samples to push before clearing the `flushing` flag.
///
/// This is the number of interleaved f32 samples (not frames) we aim to fill
/// into the ring buffer between "drain complete" and "flushing = false".  The
/// RT callback will see data immediately when it first checks after flushing
/// clears, eliminating the gap caused by the decode-thread's decode latency.
///
/// 8192 interleaved samples = 4096 stereo frames ≈ 85 ms at 48 kHz.
/// That is enough to cover one or two RT callback periods and a scheduling
/// round-trip without making the pre-fill loop run overly long.
const SEEK_PREFILL_SAMPLES: usize = 8192;

/// The body of the decode thread.
#[allow(clippy::too_many_arguments)]
fn decode_loop(
    decoder: &mut SymphoniaDecoder,
    mut producer: Producer<f32>,
    file_rate: u32,
    file_channels: u16,
    device_rate: u32,
    device_channels: u16,
    stop: &AtomicBool,
    paused: &AtomicBool,
    seek_ms: &AtomicU64,
    _seek_generation: &AtomicU64,
    flushing: &AtomicBool,
    frames_played: &AtomicU64,
    eq_config: &Mutex<EqConfig>,
    eq_generation: &AtomicU64,
    bit_perfect: &AtomicBool,
) {
    // Build a persistent resampler once per track.  Only created when the file
    // rate differs from the device rate; otherwise it stays `None` and we skip
    // resampling entirely (zero overhead).
    //
    // Holding ONE resampler across chunks is critical: the polynomial filter
    // has an internal delay line that must carry over between chunks.  If a
    // fresh resampler were created per chunk (the old behaviour), the filter
    // would re-prime from zeros at every boundary, producing a transient
    // click/pop artefact.  On seek we call `resampler.reset()` instead of
    // re-creating, which zeroes the delay line without reallocating.
    let mut stream_resampler: Option<StreamResampler> = if file_rate != device_rate {
        match StreamResampler::new(file_channels, file_rate, device_rate) {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("[lyra-engine] failed to create stream resampler: {e}");
                return;
            }
        }
    } else {
        None
    };

    // EQ state: the Equalizer instance and the generation at which it was built.
    // We rebuild it whenever `eq_generation` has changed since last build.
    let mut eq_instance: Option<Equalizer> = None;
    let mut eq_gen_seen: u64 = u64::MAX; // force rebuild on first use

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // ── Seek command ────────────────────────────────────────────────────
        let target_ms = seek_ms.load(Ordering::Acquire);
        if target_ms != SEEK_NONE {
            // Clear the seek request immediately so we don't re-enter.
            seek_ms.store(SEEK_NONE, Ordering::Release);

            let target_secs = target_ms as f64 / 1000.0;

            // Perform the seek in the format reader.
            match decoder.seek_to_secs(target_secs) {
                Ok(()) => {
                    // Wait until the ring buffer is fully drained by the RT
                    // callback (which is silencing + popping while flushing=true).
                    // We check every 2 ms; bail on stop.
                    while producer.slots() < producer.buffer().capacity() {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        thread::sleep(Duration::from_millis(2));
                    }

                    // Reset the resampler's filter delay line so no pre-seek
                    // audio bleeds into the post-seek output.
                    if let Some(ref mut rs) = stream_resampler {
                        rs.reset();
                    }

                    // ── Seek pre-fill ──────────────────────────────────────
                    // Decode and push a small batch of audio into the ring
                    // buffer BEFORE clearing the `flushing` flag.  When the RT
                    // callback next fires it will find real audio waiting,
                    // eliminating the silence gap caused by the scheduling
                    // round-trip between "flushing = false" and the first
                    // decoded chunk arriving.
                    let mut prefilled = 0usize;
                    while prefilled < SEEK_PREFILL_SAMPLES {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        // Bail if a new seek has arrived while pre-filling.
                        if seek_ms.load(Ordering::Acquire) != SEEK_NONE {
                            break;
                        }

                        let chunk = match decoder.next_chunk() {
                            Ok(Some(c)) => c,
                            Ok(None) => break, // EOF reached
                            Err(e) => {
                                eprintln!("[lyra-engine] prefill decode error: {e}");
                                break;
                            }
                        };

                        let chunk = apply_eq_and_resample(
                            chunk,
                            file_rate,
                            file_channels,
                            &mut stream_resampler,
                            eq_config,
                            eq_generation,
                            &mut eq_instance,
                            &mut eq_gen_seen,
                            bit_perfect,
                        );
                        let chunk = adapt_channels(&chunk, file_channels, device_channels);

                        // Push the chunk; bail on stop or new seek.
                        push_all(&mut producer, &chunk, stop, seek_ms);
                        prefilled += chunk.len();
                    }

                    // Update the position counter to reflect the new position.
                    let new_frames = (target_secs * device_rate as f64) as u64;
                    frames_played.store(new_frames, Ordering::Release);
                }
                Err(e) => {
                    eprintln!("[lyra-engine] seek error (ignoring, resuming from current pos): {e}");
                }
            }

            // Clear the flush flag — the RT callback now resumes normal
            // operation.  The ring buffer already has pre-filled audio waiting.
            flushing.store(false, Ordering::Release);
            continue;
        }

        // ── Spin-wait when paused ───────────────────────────────────────────
        if paused.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        // ── Decode one packet from the file ─────────────────────────────────
        let chunk = match decoder.next_chunk() {
            Ok(Some(c)) => c,
            Ok(None) => break, // EOF
            Err(e) => {
                eprintln!("[lyra-engine] decode error: {e}");
                break;
            }
        };

        // Apply EQ (at file rate) and resample (if needed).
        let chunk = apply_eq_and_resample(
            chunk,
            file_rate,
            file_channels,
            &mut stream_resampler,
            eq_config,
            eq_generation,
            &mut eq_instance,
            &mut eq_gen_seen,
            bit_perfect,
        );

        // Adapt channel count if necessary.
        let chunk = adapt_channels(&chunk, file_channels, device_channels);

        // Push into the ring buffer; park briefly when full so we don't spin.
        push_all(&mut producer, &chunk, stop, seek_ms);
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a fresh 10-band Equalizer from the current EqConfig gains.
///
/// Called whenever the generation counter changes.  Returns `None` if all
/// gains are 0 dB (identity — no point running filter math) or if
/// construction fails.
fn build_equalizer(sample_rate: u32, cfg: &EqConfig) -> Option<Equalizer> {
    if !cfg.enabled {
        return None;
    }
    let bands: Vec<EqBand> = EQ_FREQS_HZ
        .iter()
        .zip(cfg.gains.iter())
        .map(|(&freq_hz, &gain_db)| EqBand { freq_hz, gain_db, q: EQ_Q })
        .collect();
    match Equalizer::new(sample_rate, &bands) {
        Ok(eq) => Some(eq),
        Err(e) => {
            eprintln!("[lyra-engine] EQ build error: {e}");
            None
        }
    }
}

/// Apply EQ (if enabled and not bit-perfect) then resample (if needed and
/// not bit-perfect with matching rates).
///
/// EQ runs at the file sample rate, before resampling — this is the correct
/// order so filter cutoffs are in terms of the source material's frequency
/// grid.
#[allow(clippy::too_many_arguments)]
fn apply_eq_and_resample(
    mut chunk: Vec<f32>,
    file_rate: u32,
    file_channels: u16,
    stream_resampler: &mut Option<StreamResampler>,
    eq_config: &Mutex<EqConfig>,
    eq_generation: &AtomicU64,
    eq_instance: &mut Option<Equalizer>,
    eq_gen_seen: &mut u64,
    bit_perfect: &AtomicBool,
) -> Vec<f32> {
    let is_bit_perfect = bit_perfect.load(Ordering::Relaxed);

    // ── EQ ──────────────────────────────────────────────────────────────────
    if !is_bit_perfect {
        // Check if the EQ instance needs to be rebuilt.
        let current_gen = eq_generation.load(Ordering::Relaxed);
        if current_gen != *eq_gen_seen {
            *eq_gen_seen = current_gen;
            // Lock briefly to snapshot the config; don't hold across process().
            *eq_instance = eq_config
                .lock()
                .ok()
                .and_then(|cfg| build_equalizer(file_rate, &cfg));
        }

        if let Some(ref mut eq) = eq_instance {
            eq.process(&mut chunk, file_channels);
        }
    }

    // ── Resample ────────────────────────────────────────────────────────────
    // In bit-perfect mode, skip resampling if the resampler is absent
    // (rates already match).  If rates differ we must still resample to
    // produce audio at the device rate — true device-exclusive bit-perfect
    // requires OS-level exclusive mode, which is a deeper follow-on.
    if is_bit_perfect && stream_resampler.is_none() {
        return chunk;
    }

    match stream_resampler {
        None => chunk,
        Some(rs) => match rs.process_chunk(&chunk) {
            Ok(out) => out,
            Err(e) => {
                eprintln!("[lyra-engine] resample error: {e}");
                Vec::new()
            }
        },
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
/// briefly and retry — but bail out if the stop flag is set or a seek command
/// arrives (we will restart the push from scratch after the seek anyway).
fn push_all(
    producer: &mut Producer<f32>,
    samples: &[f32],
    stop: &AtomicBool,
    seek_ms: &AtomicU64,
) {
    let mut cursor = 0;
    while cursor < samples.len() {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        // If a seek arrives mid-push, abort pushing this stale chunk.
        if seek_ms.load(Ordering::Acquire) != SEEK_NONE {
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
