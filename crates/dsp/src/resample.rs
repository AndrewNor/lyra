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

    // Use polynomial resampler (faster, good enough for audio path)
    // chunk_size: use the full input as one chunk (max 65536 for sanity).
    let chunk_size = num_frames.min(65536);

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
}
