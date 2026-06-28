//! Rubato-based resampling.

use crate::Error;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Async, FixedAsync, PolynomialDegree, Resampler};

/// Resample an interleaved f32 buffer from `from_rate` to `to_rate`.
///
/// Returns a new interleaved `Vec<f32>` at `to_rate`.
/// If `from_rate == to_rate`, the input is returned unchanged.
///
/// The output length will be approximately `input_frames * to_rate / from_rate`
/// (within a small tolerance imposed by the resampler's chunk-size alignment).
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
}
