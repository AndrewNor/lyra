/// Integration test for lyra-decoder: generate a synthetic WAV, open it with
/// SymphoniaDecoder, and verify the spec and sample output.

use std::io::Write as _;
use std::path::Path;

use lyra_decoder::{AudioSpec, Decoder, SymphoniaDecoder};

/// Write a minimal, valid 16-bit stereo PCM WAV containing `frames` frames of
/// a 440 Hz sine wave at `sample_rate` Hz.
fn write_sine_wav(path: &Path, sample_rate: u32, frames: usize) {
    let channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * u32::from(block_align);
    let data_bytes = (frames * channels as usize * (bits_per_sample as usize / 8)) as u32;

    // Build the WAV header (44 bytes).
    let mut hdr = Vec::<u8>::with_capacity(44);
    // RIFF chunk descriptor
    hdr.extend_from_slice(b"RIFF");
    let riff_size: u32 = 36 + data_bytes;
    hdr.extend_from_slice(&riff_size.to_le_bytes());
    hdr.extend_from_slice(b"WAVE");
    // fmt sub-chunk
    hdr.extend_from_slice(b"fmt ");
    hdr.extend_from_slice(&16u32.to_le_bytes());      // sub-chunk size
    hdr.extend_from_slice(&1u16.to_le_bytes());        // PCM = 1
    hdr.extend_from_slice(&channels.to_le_bytes());
    hdr.extend_from_slice(&sample_rate.to_le_bytes());
    hdr.extend_from_slice(&byte_rate.to_le_bytes());
    hdr.extend_from_slice(&block_align.to_le_bytes());
    hdr.extend_from_slice(&bits_per_sample.to_le_bytes());
    // data sub-chunk header
    hdr.extend_from_slice(b"data");
    hdr.extend_from_slice(&data_bytes.to_le_bytes());

    let mut file = std::fs::File::create(path).expect("create wav");
    file.write_all(&hdr).expect("write header");

    // Write samples: interleaved L+R sine, 440 Hz
    let freq = 440.0_f64;
    for i in 0..frames {
        let t = i as f64 / sample_rate as f64;
        let sample_f = (2.0 * std::f64::consts::PI * freq * t).sin() * 0.5; // ±0.5
        let sample_i16 = (sample_f * i16::MAX as f64) as i16;
        let bytes = sample_i16.to_le_bytes();
        file.write_all(&bytes).expect("write L");
        file.write_all(&bytes).expect("write R");
    }
}

#[test]
fn decode_sine_wav_spec_and_samples() {
    let dir = tempfile::tempdir().expect("tempdir");
    let wav_path = dir.path().join("sine440.wav");

    const SAMPLE_RATE: u32 = 44100;
    const FRAMES: usize = 4410; // 0.1 s of audio

    write_sine_wav(&wav_path, SAMPLE_RATE, FRAMES);

    let mut dec = SymphoniaDecoder::open(&wav_path).expect("open decoder");

    // Check spec
    let spec = dec.spec();
    assert_eq!(
        spec,
        AudioSpec { sample_rate: SAMPLE_RATE, channels: 2 },
        "expected 44100 Hz stereo"
    );

    // Drain all chunks
    let mut total_interleaved: usize = 0;
    let terminated_cleanly = loop {
        match dec.next_chunk().expect("next_chunk should not error") {
            Some(chunk) => {
                assert!(!chunk.is_empty(), "chunk must not be empty");
                total_interleaved += chunk.len();
            }
            None => break true,
        }
    };

    assert!(terminated_cleanly, "stream must terminate with Ok(None)");

    // Allow ±500 interleaved samples tolerance for decoder framing/priming/gapless trim.
    let expected = FRAMES * 2; // interleaved samples (L+R)
    let tolerance = 500;
    assert!(
        total_interleaved >= expected.saturating_sub(tolerance)
            && total_interleaved <= expected + tolerance,
        "expected ≈{expected} interleaved samples (±{tolerance}), got {total_interleaved}"
    );
}
