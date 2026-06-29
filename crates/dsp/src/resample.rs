//! Rubato-based resampling.
//!
//! Two interfaces are provided:
//!
//! - [`resample`] — stateless one-shot conversion of a complete buffer.
//!   Suitable for offline/batch use (e.g. loudness pre-analysis).
//!   Creates a fresh resampler each call; do **not** use in a decode loop.
//!
//! - [`StreamResampler`] — stateful streaming resampler for real-time
//!   decode loops.  Feed arbitrary-length chunks through
//!   [`StreamResampler::process_chunk`]; call [`StreamResampler::reset`]
//!   after a seek or track change to flush the filter's internal delay
//!   line without re-allocating.

use crate::Error;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Async, FixedAsync, Indexing, PolynomialDegree, Resampler};

// ── StreamResampler ──────────────────────────────────────────────────────────

/// Chunk size (in frames) used for the internal rubato `Async` resampler.
///
/// 1024 frames ≈ 21 ms at 48 kHz — a good balance between per-call overhead
/// and output latency.  Must be > 0 and <= the max_chunk_size baked into the
/// resampler at construction time.
const CHUNK_SIZE: usize = 1024;

/// A stateful streaming resampler that can be fed variable-length chunks.
///
/// Internally holds a rubato [`Async`] polynomial resampler with a fixed
/// `CHUNK_SIZE`, and an input staging buffer that collects frames until a full
/// `input_frames_next()` chunk is ready.  On flush the incomplete staging
/// frames are preserved but the resampler's filter delay line is zeroed via
/// [`Async::reset`], ensuring no discontinuity artifacts carry over from
/// pre-seek audio.
///
/// # Usage pattern in a decode loop
/// ```ignore
/// let mut rs = StreamResampler::new(channels, from_rate, to_rate)?;
/// loop {
///     let chunk = decoder.next_chunk()?;   // Vec<f32>, variable length
///     let resampled = rs.process_chunk(&chunk)?;
///     push_to_ring_buffer(&resampled);
///
///     // On seek:
///     rs.reset();
/// }
/// ```
pub struct StreamResampler {
    inner: Async<f32>,
    /// Interleaved staging buffer: frames waiting to be fed to `inner`.
    staging: Vec<f32>,
    channels: usize,
    /// Output frames to skip after a reset (resampler startup delay).
    /// Decremented as frames are produced; 0 once primed.
    delay_remaining: usize,
}

impl StreamResampler {
    /// Create a new streaming resampler.
    ///
    /// Returns `Err` if `channels == 0`, rates are 0, or rubato fails to
    /// construct the resampler.
    pub fn new(channels: u16, from_rate: u32, to_rate: u32) -> crate::Result<Self> {
        let ch = channels as usize;
        if ch == 0 {
            return Err(Error::Resample("channels must be > 0".into()));
        }
        if from_rate == 0 || to_rate == 0 {
            return Err(Error::Resample("sample rates must be > 0".into()));
        }

        let ratio = to_rate as f64 / from_rate as f64;

        // max_relative_ratio = 1.0 → ratio is fixed (no dynamic adjustment).
        let inner = Async::<f32>::new_poly(
            ratio,
            1.0,
            PolynomialDegree::Cubic,
            CHUNK_SIZE,
            ch,
            FixedAsync::Input,
        )
        .map_err(|e| Error::Resample(format!("{e:?}")))?;

        let delay_remaining = inner.output_delay();

        Ok(Self {
            inner,
            staging: Vec::new(),
            channels: ch,
            delay_remaining,
        })
    }

    /// Reset the resampler after a seek or track change.
    ///
    /// Zeroes the internal filter delay line (preventing pre-seek audio
    /// from bleeding into post-seek output) and clears the staging buffer.
    /// Does **not** re-allocate; the object is reused as-is.
    pub fn reset(&mut self) {
        self.inner.reset();
        self.staging.clear();
        // Re-arm the startup delay skip.
        self.delay_remaining = self.inner.output_delay();
    }

    /// Feed an interleaved `f32` chunk through the resampler.
    ///
    /// The chunk may be any length (including 0).  Internally, frames are
    /// staged until a full `input_frames_next()` block is ready, then
    /// `process_into_buffer` is called once per full block.  Any leftover
    /// frames are retained in the staging buffer for the next call.
    ///
    /// Returns the interleaved resampled output.  May be empty if there
    /// were not enough staged frames to produce a full block.
    pub fn process_chunk(&mut self, input: &[f32]) -> crate::Result<Vec<f32>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        // Append new frames to the staging buffer.
        self.staging.extend_from_slice(input);

        let mut out = Vec::new();

        loop {
            let frames_in_staging = self.staging.len() / self.channels;
            let need = self.inner.input_frames_next();

            if frames_in_staging < need {
                // Not enough frames for a full block yet.
                break;
            }

            // Prepare the input slice for exactly `need` frames.
            let in_samples = need * self.channels;
            let block = &self.staging[..in_samples];

            // Wrap in a rubato adapter.
            let buf_in = InterleavedSlice::new(block, self.channels, need)
                .map_err(|e| Error::Resample(format!("{e:?}")))?;

            // Allocate output buffer for this block.
            let out_cap = self.inner.output_frames_next();
            let mut buf_out = InterleavedOwned::<f32>::new(0.0_f32, self.channels, out_cap);

            let indexing = Indexing {
                input_offset: 0,
                output_offset: 0,
                active_channels_mask: None,
                partial_len: None,
            };

            let (_, n_out) = self
                .inner
                .process_into_buffer(&buf_in, &mut buf_out, Some(&indexing))
                .map_err(|e| Error::Resample(format!("{e:?}")))?;

            // Drain the consumed frames from staging.
            self.staging.drain(..in_samples);

            // Extract interleaved output for the produced frames.
            let raw = buf_out.take_data();
            let produced = &raw[..n_out * self.channels];

            // Skip startup delay frames on first use / after reset.
            if self.delay_remaining > 0 {
                let to_skip = self.delay_remaining.min(n_out);
                let skip_samples = to_skip * self.channels;
                out.extend_from_slice(&produced[skip_samples..]);
                self.delay_remaining -= to_skip;
            } else {
                out.extend_from_slice(produced);
            }
        }

        Ok(out)
    }
}

// ── One-shot resample (kept for batch/offline use) ───────────────────────────

/// Resample an interleaved f32 buffer from `from_rate` to `to_rate`.
///
/// Returns a new interleaved `Vec<f32>` at `to_rate`.
/// If `from_rate == to_rate`, the input is returned unchanged.
///
/// The output length will be approximately `input_frames * to_rate / from_rate`
/// (within a small tolerance imposed by the resampler's chunk-size alignment).
///
/// **Do not call this per-chunk in a decode loop** — it creates a fresh
/// resampler each time, causing filter-startup transients at every boundary.
/// Use [`StreamResampler`] instead.
pub fn resample(
    input: &[f32],
    channels: u16,
    from_rate: u32,
    to_rate: u32,
) -> crate::Result<Vec<f32>> {
    if from_rate == to_rate {
        return Ok(input.to_vec());
    }

    let ch = channels as usize;
    if ch == 0 {
        return Err(Error::Resample("channels must be > 0".into()));
    }
    if input.is_empty() {
        return Ok(Vec::new());
    }

    let num_frames = input.len() / ch;
    if num_frames == 0 {
        return Ok(Vec::new());
    }

    let ratio = to_rate as f64 / from_rate as f64;

    // Use polynomial resampler (faster, good enough for audio path).
    // chunk_size == num_frames: configure the resampler for exactly the full
    // input so that process_all_into_buffer processes it in a single call
    // without any mismatch between the configured chunk size and the actual
    // input length.  rubato 3's process_all_into_buffer already loops
    // internally for inputs larger than chunk_size, but using num_frames here
    // is simpler and avoids any buffer-size validation edge cases.
    let chunk_size = num_frames;

    let mut resampler = Async::<f32>::new_poly(
        ratio,
        1.0, // ratio is fixed, no dynamic adjustment needed
        PolynomialDegree::Cubic,
        chunk_size,
        ch,
        FixedAsync::Input,
    )
    .map_err(|e| Error::Resample(format!("{e:?}")))?;

    // Calculate output buffer size.
    let needed_out = resampler.process_all_needed_output_len(num_frames);

    // Wrap the interleaved input as an InterleavedSlice adapter.
    let buf_in = InterleavedSlice::new(input, ch, num_frames)
        .map_err(|e| Error::Resample(format!("{e:?}")))?;

    // Allocate the output buffer.
    let mut buf_out = InterleavedOwned::<f32>::new(0.0, ch, needed_out);

    let (_, out_frames) = resampler
        .process_all_into_buffer(&buf_in, &mut buf_out, num_frames, None)
        .map_err(|e| Error::Resample(format!("{e:?}")))?;

    // Extract the interleaved output, trimmed to the actual output frame count.
    let full = buf_out.take_data();
    Ok(full[..out_frames * ch].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_when_same_rate() {
        let input: Vec<f32> = (0..88200).map(|i| (i as f32) / 88200.0).collect();
        let output = resample(&input, 2, 44100, 44100).unwrap();
        assert_eq!(output.len(), input.len(), "identity resample should preserve length");
    }

    #[test]
    fn resample_length_ratio_approximately_correct() {
        // 44100 -> 48000: expect ~N * 48000/44100 frames
        let n_frames = 44100usize; // 1 second stereo
        let channels = 2u16;
        let input: Vec<f32> = vec![0.5f32; n_frames * channels as usize];

        let output = resample(&input, channels, 44100, 48000).unwrap();

        let expected_frames = (n_frames as f64 * 48000.0 / 44100.0).round() as usize;
        let actual_frames = output.len() / channels as usize;

        // Allow ±2% tolerance
        let tolerance = (expected_frames as f64 * 0.02).ceil() as usize;
        assert!(
            actual_frames.abs_diff(expected_frames) <= tolerance,
            "resample 44100->48000: expected ≈{expected_frames} frames, got {actual_frames} (tolerance ±{tolerance})"
        );
    }

    /// Regression test: inputs exceeding the old 65536-frame cap must work correctly.
    ///
    /// 100_000 stereo frames (200_000 interleaved samples) at 44100 Hz resampled
    /// to 48000 Hz.  The expected output frame count is 100_000 * 48000/44100 ≈ 108_843.
    /// We assert the result is `Ok` and within ±2% of that expectation, exercising
    /// the path that was previously misconfigured when num_frames > 65536.
    #[test]
    fn resample_large_input_over_65536_frames() {
        let n_frames = 100_000usize; // well above the old 65536 cap
        let channels = 2u16;
        // Generate a simple sine wave so the content is realistic.
        let input: Vec<f32> = (0..n_frames * channels as usize)
            .map(|i| (i as f32 * std::f32::consts::TAU / 44100.0).sin() * 0.5)
            .collect();

        let result = resample(&input, channels, 44100, 48000);
        assert!(
            result.is_ok(),
            "resample of 100_000-frame stereo input should succeed, got: {:?}",
            result.err()
        );

        let output = result.unwrap();
        let expected_frames = (n_frames as f64 * 48000.0 / 44100.0).round() as usize;
        let actual_frames = output.len() / channels as usize;
        let tolerance = (expected_frames as f64 * 0.02).ceil() as usize;
        assert!(
            actual_frames.abs_diff(expected_frames) <= tolerance,
            "large resample 44100->48000: expected ≈{expected_frames} frames, got {actual_frames} (tolerance ±{tolerance})"
        );
    }

    // ── StreamResampler tests ─────────────────────────────────────────────────

    /// A StreamResampler fed one large chunk should produce approximately the
    /// same number of output frames as the one-shot `resample()` function.
    #[test]
    fn stream_resampler_total_output_close_to_oneshot() {
        let n_frames = 44100usize;
        let channels = 2u16;
        let input: Vec<f32> = vec![0.5f32; n_frames * channels as usize];

        // One-shot reference.
        let oneshot = resample(&input, channels, 44100, 48000).unwrap();
        let expected = oneshot.len() / channels as usize;

        // StreamResampler in a single call.
        let mut sr = StreamResampler::new(channels, 44100, 48000).unwrap();
        let out = sr.process_chunk(&input).unwrap();
        let actual = out.len() / channels as usize;

        // Allow ±2% tolerance (resampler chunk-size rounding differs slightly).
        let tolerance = (expected as f64 * 0.02).ceil() as usize + CHUNK_SIZE;
        assert!(
            actual.abs_diff(expected) <= tolerance,
            "StreamResampler: expected ≈{expected} frames, got {actual} (tolerance ±{tolerance})"
        );
    }

    /// After reset(), the StreamResampler should produce output again (not
    /// permanently broken) and not panic.
    #[test]
    fn stream_resampler_reset_does_not_panic() {
        let channels = 2u16;
        let input: Vec<f32> = vec![0.25f32; 8192 * channels as usize];

        let mut sr = StreamResampler::new(channels, 44100, 48000).unwrap();
        let _ = sr.process_chunk(&input).unwrap();
        sr.reset();
        // After reset, should still produce output without panic.
        let out = sr.process_chunk(&input).unwrap();
        assert!(!out.is_empty(), "StreamResampler should produce output after reset");
    }

    /// Feeding chunks in small pieces should produce the same total output as
    /// one large chunk (within the staging-buffer rounding tolerance).
    #[test]
    fn stream_resampler_small_chunks_same_total_as_large() {
        let n_frames = 44100usize;
        let channels = 2u16;
        let ch = channels as usize;
        let input: Vec<f32> = (0..n_frames * ch)
            .map(|i| (i as f32 * std::f32::consts::TAU / 44100.0).sin() * 0.5)
            .collect();

        // Large chunk reference.
        let mut sr_large = StreamResampler::new(channels, 44100, 48000).unwrap();
        let large_out = sr_large.process_chunk(&input).unwrap();

        // Small chunks (512 frames each).
        let mut sr_small = StreamResampler::new(channels, 44100, 48000).unwrap();
        let mut small_out = Vec::new();
        for chunk in input.chunks(512 * ch) {
            small_out.extend(sr_small.process_chunk(chunk).unwrap());
        }

        // Both should be within CHUNK_SIZE frames of each other (staging
        // buffer may hold up to CHUNK_SIZE-1 frames at end of input).
        let large_frames = large_out.len() / ch;
        let small_frames = small_out.len() / ch;
        let tolerance = CHUNK_SIZE + 4;
        assert!(
            large_frames.abs_diff(small_frames) <= tolerance,
            "large={large_frames} small={small_frames} tolerance=±{tolerance}"
        );
    }
}
