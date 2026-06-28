//! EBU R128 loudness analysis and ReplayGain gain computation.

use crate::Error;
use ebur128::{EbuR128, Mode};

/// Analyze the integrated loudness (LUFS) of an interleaved f32 audio buffer.
///
/// Returns the integrated loudness in LUFS (Loudness Units relative to Full Scale).
/// For silence or very quiet signals, may return a very large negative number or -∞.
pub fn analyze_lufs(samples: &[f32], sample_rate: u32, channels: u16) -> crate::Result<f64> {
    let mut meter = EbuR128::new(channels as u32, sample_rate, Mode::I)
        .map_err(|e| Error::Ebur128(format!("{e}")))?;
    meter
        .add_frames_f32(samples)
        .map_err(|e| Error::Ebur128(format!("{e}")))?;
    let lufs = meter
        .loudness_global()
        .map_err(|e| Error::Ebur128(format!("{e}")))?;
    Ok(lufs)
}

/// Compute the ReplayGain 2.0 gain adjustment in dB given a measured loudness.
///
/// Target is −18 LUFS (ReplayGain 2.0 reference).
/// Returns `target_lufs - lufs` = `-18.0 - lufs`.
pub fn replaygain_gain_db(lufs: f64) -> f32 {
    (-18.0_f64 - lufs) as f32
}

/// Convert a dB value to a linear gain factor.
///
/// `db_to_linear(0.0) == 1.0`, `db_to_linear(-6.0) ≈ 0.501`.
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a 1 kHz sine wave at the given amplitude, stereo interleaved.
    fn sine_stereo(freq_hz: f32, amplitude: f32, sample_rate: u32, duration_secs: f32) -> Vec<f32> {
        let num_frames = (sample_rate as f32 * duration_secs) as usize;
        let mut samples = Vec::with_capacity(num_frames * 2);
        for i in 0..num_frames {
            let t = i as f32 / sample_rate as f32;
            let v = amplitude * (2.0 * std::f32::consts::PI * freq_hz * t).sin();
            samples.push(v); // L
            samples.push(v); // R
        }
        samples
    }

    #[test]
    fn silence_is_very_quiet() {
        // 1 second of stereo digital silence
        let silence = vec![0.0f32; 44100 * 2];
        let lufs = analyze_lufs(&silence, 44100, 2).expect("should not error on silence");
        // EBU R128 returns -inf or a very large negative number for silence
        assert!(
            lufs <= -60.0 || lufs.is_infinite(),
            "expected LUFS ≤ -60 or -inf for silence, got {lufs}"
        );
    }

    #[test]
    fn loud_sine_is_in_sane_range() {
        // 1 second stereo −6 dBFS 1 kHz sine
        let amplitude = 10.0_f32.powf(-6.0 / 20.0); // −6 dBFS
        let samples = sine_stereo(1000.0, amplitude, 44100, 1.0);
        let lufs = analyze_lufs(&samples, 44100, 2).expect("should not error on sine");
        assert!(
            lufs >= -12.0 && lufs <= -3.0,
            "expected LUFS in -12..-3 for −6 dBFS 1 kHz sine, got {lufs}"
        );
    }

    #[test]
    fn replaygain_at_target_is_zero() {
        let gain = replaygain_gain_db(-18.0);
        assert!(
            (gain - 0.0).abs() < 0.001,
            "replaygain_gain_db(-18.0) should ≈ 0.0, got {gain}"
        );
    }

    #[test]
    fn replaygain_louder_signal_gets_negative_gain() {
        let gain = replaygain_gain_db(-8.0);
        assert!(
            (gain - (-10.0)).abs() < 0.001,
            "replaygain_gain_db(-8.0) should ≈ -10.0, got {gain}"
        );
    }

    #[test]
    fn db_to_linear_unity() {
        assert_eq!(db_to_linear(0.0), 1.0);
    }

    #[test]
    fn db_to_linear_minus6() {
        let linear = db_to_linear(-6.0);
        assert!(
            (linear - 0.501).abs() < 0.01,
            "db_to_linear(-6.0) should ≈ 0.501, got {linear}"
        );
    }
}
