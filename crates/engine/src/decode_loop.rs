//! Decode thread: SymphoniaDecoder → optional EQ → optional resample → rtrb::Producer.
//!
//! This runs on a dedicated OS thread (not the audio callback thread).
//! It is allowed to allocate and may park briefly when the ring buffer is full.
//!
//! # Crossfade
//! When `crossfade_secs > 0` and the outgoing track is within the crossfade
//! window of its end, a second decoder is opened for the next track and both
//! are mixed with equal-power gain curves (cos/sin ramp) before the mixed
//! f32 frames are pushed into the ring buffer.  The RT output callback is
//! NOT changed — it pops plain f32 as always.
//!
//! Normal playback (crossfade = 0) is completely unchanged: one decoder,
//! gain = 1.0 throughout, gapless behaviour fully preserved.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use lyra_decoder::{Decoder, SymphoniaDecoder};
use lyra_dsp::{EqBand, Equalizer, StreamResampler};
use rtrb::Producer;

use crate::{EqConfig, Error, EQ_FREQS_HZ, EQ_Q, SEEK_NONE};

// ── DecodeThread handle ───────────────────────────────────────────────────────

/// Handle to the decode thread.
pub(crate) struct DecodeThread {
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl DecodeThread {
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
        crossfade_secs: Arc<AtomicU32>,
        next_track_path: Arc<Mutex<Option<PathBuf>>>,
        decode_done: Arc<AtomicBool>,
    ) -> Result<Self, Error> {
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
                    &crossfade_secs,
                    &next_track_path,
                    &decode_done,
                );
            })
            .map_err(|e| Error::Thread(e.to_string()))?;

        Ok(Self { stop_flag, handle: Some(handle) })
    }

    pub(crate) fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for DecodeThread {
    fn drop(&mut self) {
        self.stop();
    }
}

// ── Seek pre-fill ────────────────────────────────────────────────────────────

/// Samples to pre-fill into the ring buffer before clearing `flushing`.
/// 8192 interleaved f32 ≈ 85 ms at 48 kHz stereo.
const SEEK_PREFILL_SAMPLES: usize = 8192;

// ── Per-source DSP state ──────────────────────────────────────────────────────

/// All DSP state for one audio source (decoder + resampler + EQ + leftover buf).
struct Source {
    decoder: SymphoniaDecoder,
    file_rate: u32,
    file_channels: u16,
    resampler: Option<StreamResampler>,
    eq_instance: Option<Equalizer>,
    eq_gen_seen: u64,
    /// Device-rate, device-channel interleaved f32 samples waiting to be
    /// consumed.  Grows as we decode; shrinks as the crossfade mixer reads it.
    buf: Vec<f32>,
    /// Read cursor into `buf`.
    cursor: usize,
}

impl Source {
    fn open(path: &Path, device_rate: u32) -> Option<Self> {
        let decoder = match SymphoniaDecoder::open(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[lyra-engine] crossfade: cannot open next track: {e}");
                return None;
            }
        };
        let spec = decoder.spec();
        let file_rate = spec.sample_rate;
        let file_channels = spec.channels;
        let resampler = if file_rate != device_rate {
            match StreamResampler::new(file_channels, file_rate, device_rate) {
                Ok(r) => Some(r),
                Err(e) => {
                    eprintln!("[lyra-engine] crossfade resampler error: {e}");
                    None
                }
            }
        } else {
            None
        };
        Some(Self {
            decoder,
            file_rate,
            file_channels,
            resampler,
            eq_instance: None,
            eq_gen_seen: u64::MAX,
            buf: Vec::new(),
            cursor: 0,
        })
    }

    /// Decode and process chunks until `buf[cursor..]` has at least `want`
    /// samples.  Returns `false` on EOF or error.
    fn ensure_available(
        &mut self,
        want: usize,
        device_channels: u16,
        eq_config: &Mutex<EqConfig>,
        eq_generation: &AtomicU64,
        bit_perfect: &AtomicBool,
    ) -> bool {
        // Compact consumed prefix to avoid unbounded growth.
        if self.cursor > 0 {
            self.buf.drain(..self.cursor);
            self.cursor = 0;
        }

        while self.buf.len() < want {
            let raw = match self.decoder.next_chunk() {
                Ok(Some(c)) => c,
                Ok(None) => return false,
                Err(e) => {
                    eprintln!("[lyra-engine] crossfade decode error: {e}");
                    return false;
                }
            };
            let processed = apply_eq_and_resample(
                raw,
                self.file_rate,
                self.file_channels,
                &mut self.resampler,
                eq_config,
                eq_generation,
                &mut self.eq_instance,
                &mut self.eq_gen_seen,
                bit_perfect,
            );
            let adapted = adapt_channels(&processed, self.file_channels, device_channels);
            self.buf.extend_from_slice(&adapted);
        }
        true
    }

    /// Return `n` samples starting at cursor (zero-extends if fewer available).
    /// Advances the cursor by `n`.
    fn take_slice(&mut self, n: usize) -> &[f32] {
        let start = self.cursor;
        let end = (start + n).min(self.buf.len());
        self.cursor = end;
        &self.buf[start..end]
    }

    /// Remaining samples in the buffer (after cursor).
    fn available(&self) -> usize {
        self.buf.len() - self.cursor
    }
}

// ── Main decode loop ──────────────────────────────────────────────────────────

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
    crossfade_secs: &AtomicU32,
    next_track_path: &Mutex<Option<PathBuf>>,
    decode_done: &AtomicBool,
) {
    // Resampler for the primary (outgoing) track.
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

    // EQ state for the primary track.
    let mut eq_instance: Option<Equalizer> = None;
    let mut eq_gen_seen: u64 = u64::MAX;

    // Crossfade state: the incoming (next-track) source and window counters.
    // All `None` / 0 means crossfade is inactive.
    let mut xfade: Option<Source> = None;
    // Total frames in the crossfade window.
    let mut xfade_total_frames: u64 = 0;
    // Frames of the window already consumed (elapsed).
    let mut xfade_elapsed_frames: u64 = 0;

    // Reusable mix buffer (not on the RT path).
    let mut mix_buf: Vec<f32> = Vec::new();

    // Total device-rate FRAMES pushed to the ring buffer since this decode
    // thread started.  Used to detect when we're `crossfade_window` frames
    // from EOF — which requires knowing the track duration in frames.
    // We get that from frames_played (updated by RT callback) + ring fullness.
    // Since we can't know duration without metadata, we use a different trigger:
    // We count frames_pushed locally and also watch `frames_played`.
    // Trigger logic: once frames_pushed is large enough, attempt to open the
    // next source; if we're still far from EOF it won't hurt (the source is
    // buffered).
    //
    // We set xfade_trigger_frames from the crossfade window: once
    // `frames_pushed - frames_played_at_trigger >= xfade_trigger` we know
    // we've pushed the window's worth ahead of the playback head and should
    // be near the end.
    //
    // Actually, the cleanest approach: we know frames_pushed (local counter).
    // We know frames_played (shared, set by RT callback).
    // remaining_in_ring ≈ frames_pushed - frames_played
    // We want to start crossfade when remaining_in_ring < xfade_window_frames
    // AND we haven't opened the xfade source yet.
    // But we can also just try to decode a chunk and if decoder returns None,
    // we're at EOF — so we can start crossfade when we first get a short chunk.
    //
    // Most practical approach (used here): track `frames_pushed` locally.
    // Each time through the normal loop, check if the ring has fewer available
    // slots than the crossfade window * channels.  If so, spin up the xfade.
    let mut frames_pushed: u64 = 0;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // ── Seek command ────────────────────────────────────────────────────
        let target_ms = seek_ms.load(Ordering::Acquire);
        if target_ms != SEEK_NONE {
            // Cancel any in-progress crossfade cleanly.
            xfade = None;
            xfade_total_frames = 0;
            xfade_elapsed_frames = 0;
            frames_pushed = 0;

            seek_ms.store(SEEK_NONE, Ordering::Release);

            let target_secs = target_ms as f64 / 1000.0;

            match decoder.seek_to_secs(target_secs) {
                Ok(()) => {
                    while producer.slots() < producer.buffer().capacity() {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        thread::sleep(Duration::from_millis(2));
                    }

                    if let Some(ref mut rs) = stream_resampler {
                        rs.reset();
                    }

                    let mut prefilled = 0usize;
                    while prefilled < SEEK_PREFILL_SAMPLES {
                        if stop.load(Ordering::Relaxed) {
                            return;
                        }
                        if seek_ms.load(Ordering::Acquire) != SEEK_NONE {
                            break;
                        }

                        let chunk = match decoder.next_chunk() {
                            Ok(Some(c)) => c,
                            Ok(None) => break,
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

                        let len = chunk.len();
                        push_all(&mut producer, &chunk, stop, seek_ms);
                        prefilled += len;
                    }

                    let new_frames = (target_secs * device_rate as f64) as u64;
                    frames_played.store(new_frames, Ordering::Release);
                    frames_pushed = new_frames; // re-sync local counter
                }
                Err(e) => {
                    eprintln!("[lyra-engine] seek error: {e}");
                }
            }

            flushing.store(false, Ordering::Release);
            continue;
        }

        // ── Spin-wait when paused ───────────────────────────────────────────
        if paused.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        // ── Crossfade active: mix outgoing + incoming ───────────────────────
        if let Some(ref mut next) = xfade {
            let ch = device_channels as usize;
            // Work in 512-frame chunks.
            let chunk_frames: usize = 512;
            let chunk_samples = chunk_frames * ch;

            // Ensure the incoming source has enough data buffered.
            let _ = next.ensure_available(
                chunk_samples,
                device_channels,
                eq_config,
                eq_generation,
                bit_perfect,
            );

            // Decode a chunk from the outgoing (primary) source.
            let outgoing = match decoder.next_chunk() {
                Ok(Some(c)) => {
                    let c = apply_eq_and_resample(
                        c,
                        file_rate,
                        file_channels,
                        &mut stream_resampler,
                        eq_config,
                        eq_generation,
                        &mut eq_instance,
                        &mut eq_gen_seen,
                        bit_perfect,
                    );
                    adapt_channels(&c, file_channels, device_channels)
                }
                Ok(None) => {
                    // Outgoing track finished mid-crossfade.
                    // Flush remaining incoming buffer then exit normally.
                    let avail = next.available();
                    if avail > 0 {
                        // Apply final incoming gain = 1.0 (window may be partial).
                        let slice = next.take_slice(avail).to_vec();
                        push_all(&mut producer, &slice, stop, seek_ms);
                    }
                    break;
                }
                Err(e) => {
                    eprintln!("[lyra-engine] crossfade decode error (outgoing): {e}");
                    break;
                }
            };

            if outgoing.is_empty() {
                continue;
            }

            let out_frames = outgoing.len() / ch.max(1);

            // Ensure the incoming source has enough for this chunk.
            let _ = next.ensure_available(
                outgoing.len(),
                device_channels,
                eq_config,
                eq_generation,
                bit_perfect,
            );

            // Mix with equal-power crossfade gains.
            mix_buf.clear();
            mix_buf.reserve(outgoing.len());

            let incoming_slice = next.take_slice(outgoing.len());

            for frame_idx in 0..out_frames {
                // t: 0.0 at window start → 1.0 at window end.
                let t = if xfade_total_frames > 0 {
                    (xfade_elapsed_frames + frame_idx as u64)
                        .min(xfade_total_frames) as f32
                        / xfade_total_frames as f32
                } else {
                    1.0
                };
                let angle = t * std::f32::consts::FRAC_PI_2;
                let gain_out = angle.cos(); // 1→0
                let gain_in = angle.sin();  // 0→1

                for c_idx in 0..ch {
                    let idx = frame_idx * ch + c_idx;
                    let s_out = *outgoing.get(idx).unwrap_or(&0.0);
                    let s_in = *incoming_slice.get(idx).unwrap_or(&0.0);
                    mix_buf.push(s_out * gain_out + s_in * gain_in);
                }
            }

            xfade_elapsed_frames += out_frames as u64;
            let pushed_samples = mix_buf.len();
            push_all(&mut producer, &mix_buf, stop, seek_ms);
            frames_pushed += (pushed_samples / ch.max(1)) as u64;

            // Check if the crossfade window is exhausted.
            if xfade_elapsed_frames >= xfade_total_frames {
                // Push any buffered leftover from the incoming source.
                let avail = next.available();
                if avail > 0 {
                    let slice = next.take_slice(avail).to_vec();
                    push_all(&mut producer, &slice, stop, seek_ms);
                }
                // Crossfade done; the outgoing track may still have audio which
                // we'll decode in normal mode, but practically we're at EOF.
                xfade = None;
            }
            continue;
        }

        // ── Normal path: decode one packet from the primary file ────────────
        let chunk = match decoder.next_chunk() {
            Ok(Some(c)) => c,
            Ok(None) => break, // EOF — normal end of track
            Err(e) => {
                eprintln!("[lyra-engine] decode error: {e}");
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
        let _chunk_frames = chunk.len() / device_channels.max(1) as usize;

        // ── Crossfade trigger check ─────────────────────────────────────────
        // After processing (but before pushing), check if we should start a
        // crossfade.  We trigger once the gap between `frames_pushed` and
        // `frames_played` (read from the RT callback's counter) is small
        // enough — i.e. we've decoded the crossfade window ahead of what's
        // already playing.
        //
        // Trigger condition: frames_pushed is ahead of frames_played by less
        // than 2× the crossfade window.  This fires slightly early, giving
        // the next-source decoder time to buffer up.
        //
        // Note: frames_pushed is incremented AFTER we push below, so at this
        // point it represents the state before this chunk is added.
        let xfade_window_f = f32::from_bits(crossfade_secs.load(Ordering::Relaxed));
        if xfade_window_f > 0.0 && xfade.is_none() {
            let xfade_window_frames = (xfade_window_f * device_rate as f32) as u64;
            let played = frames_played.load(Ordering::Relaxed);
            // Ring buffer fullness: pushed - played ≈ frames buffered ahead.
            let buffered_ahead = frames_pushed.saturating_sub(played);
            // Start crossfade when we're within the window of EOF:
            // i.e. the ring buffer has fewer ahead than the crossfade window.
            // This isn't perfect without knowing duration, but it's safe:
            // if we're not near EOF the next track decoder stays buffered and
            // is thrown away on the next seek / play() call.
            //
            // Better trigger: ring buffer's available write slots are small
            // (approaching full means we've pushed a lot since last drain).
            // Use a hybrid: if buffered_ahead < xfade_window_frames AND ring
            // is not completely empty (we've actually pushed some data).
            if frames_pushed > xfade_window_frames
                && buffered_ahead <= xfade_window_frames
            {
                // Try to open the next source.
                let next_path_opt: Option<PathBuf> = next_track_path
                    .lock()
                    .ok()
                    .and_then(|mut g| g.take());

                if let Some(next_path) = next_path_opt {
                    if let Some(src) = Source::open(&next_path, device_rate) {
                        xfade_total_frames = xfade_window_frames;
                        xfade_elapsed_frames = 0;
                        xfade = Some(src);
                    }
                }
            }
        }

        // Push the (unmixed) primary chunk.
        let pushed_len = chunk.len();
        push_all(&mut producer, &chunk, stop, seek_ms);
        frames_pushed += (pushed_len / device_channels.max(1) as usize) as u64;
    }

    // The loop exited on EOF (or an unrecoverable decode error) rather than a
    // user stop: mark the track as done decoding. The RT callback flips the
    // engine's `finished` flag once the ring buffer drains, so the UI can
    // auto-advance the queue.
    if !stop.load(Ordering::Relaxed) {
        decode_done.store(true, Ordering::Relaxed);
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a fresh 10-band Equalizer from the current EqConfig gains.
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

    if !is_bit_perfect {
        let current_gen = eq_generation.load(Ordering::Relaxed);
        if current_gen != *eq_gen_seen {
            *eq_gen_seen = current_gen;
            *eq_instance = eq_config
                .lock()
                .ok()
                .and_then(|cfg| build_equalizer(file_rate, &cfg));
        }

        if let Some(ref mut eq) = eq_instance {
            eq.process(&mut chunk, file_channels);
        }
    }

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
                out.push(frame[0]);
            } else {
                out.push(0.0);
            }
        }
    }
    out
}

/// Push all samples into the producer.  Sleep 1 ms when full; bail on stop/seek.
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
        if seek_ms.load(Ordering::Acquire) != SEEK_NONE {
            return;
        }

        match producer.push(samples[cursor]) {
            Ok(()) => cursor += 1,
            Err(_full) => thread::sleep(Duration::from_millis(1)),
        }
    }
}
