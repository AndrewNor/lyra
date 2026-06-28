//! Parametric biquad EQ.

use crate::Error;
use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type};

/// A single parametric EQ band.
#[derive(Debug, Clone, Copy)]
pub struct EqBand {
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
}

/// A multi-band parametric equalizer using peaking biquad filters.
///
/// One `DirectForm2Transposed` filter per (band × channel) is maintained
/// so that the filter state is kept separately per channel.
pub struct Equalizer {
    sample_rate: u32,
    bands: Vec<EqBand>,
    /// `filters[band_idx]` is a Vec of per-channel filter instances.
    filters: Vec<Vec<DirectForm2Transposed<f32>>>,
}

impl Equalizer {
    /// Create a new `Equalizer`.
    ///
    /// Filters are initially built for 1 channel and expanded lazily on the first
    /// `process` call.  An error is returned if any band has an out-of-range frequency
    /// (≥ Nyquist) or a non-positive Q.
    pub fn new(sample_rate: u32, bands: &[EqBand]) -> crate::Result<Self> {
        // Validate every band eagerly (1-channel check is enough for validation).
        let mut filters = Vec::with_capacity(bands.len());
        for band in bands {
            let c = Self::make_coeffs(sample_rate, band)?;
            filters.push(vec![DirectForm2Transposed::<f32>::new(c)]);
        }

        Ok(Equalizer {
            sample_rate,
            bands: bands.to_vec(),
            filters,
        })
    }

    fn make_coeffs(sample_rate: u32, band: &EqBand) -> crate::Result<Coefficients<f32>> {
        Coefficients::<f32>::from_params(
            Type::PeakingEQ(band.gain_db),
            (sample_rate as f32).hz(),
            band.freq_hz.hz(),
            band.q,
        )
        .map_err(|e| Error::EqParam(format!("{e:?}")))
    }

    /// Apply all EQ bands in-place to an interleaved buffer.
    ///
    /// `channels` must match the interleaving of `interleaved`.
    /// If the channel count differs from previous calls, filter instances are rebuilt.
    pub fn process(&mut self, interleaved: &mut [f32], channels: u16) {
        if channels == 0 || interleaved.is_empty() || self.bands.is_empty() {
            return;
        }
        let ch = channels as usize;

        // Ensure each band row has `ch` filter instances.
        for (row, band) in self.filters.iter_mut().zip(self.bands.iter()) {
            while row.len() < ch {
                // All channels share the same coefficients; create a fresh filter instance.
                if let Ok(c) = Self::make_coeffs(self.sample_rate, band) {
                    row.push(DirectForm2Transposed::<f32>::new(c));
                } else {
                    break;
                }
            }
        }

        // Apply bands sequentially.  Each band processes all samples with its per-channel filter.
        for (band_idx, filter_row) in self.filters.iter_mut().enumerate() {
            if band_idx >= self.bands.len() {
                break;
            }
            for (i, sample) in interleaved.iter_mut().enumerate() {
                let ch_idx = i % ch;
                if ch_idx < filter_row.len() {
                    *sample = filter_row[ch_idx].run(*sample);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_mono(freq_hz: f32, amplitude: f32, sample_rate: u32, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| {
                amplitude
                    * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        (sum_sq / samples.len() as f32).sqrt()
    }

    #[test]
    fn zero_db_band_is_identity() {
        let band = EqBand {
            freq_hz: 1000.0,
            gain_db: 0.0,
            q: 0.707,
        };
        let mut eq = Equalizer::new(44100, &[band]).unwrap();
        let original = sine_mono(440.0, 0.5, 44100, 4410);
        let mut buf = original.clone();
        eq.process(&mut buf, 1);
        // Every sample should be essentially unchanged
        for (orig, out) in original.iter().zip(buf.iter()) {
            assert!(
                (orig - out).abs() < 1e-5,
                "0 dB EQ band changed sample: {orig} -> {out}"
            );
        }
    }

    #[test]
    fn positive_boost_increases_rms_at_target_freq() {
        let band = EqBand {
            freq_hz: 1000.0,
            gain_db: 12.0,
            q: 1.0,
        };
        let mut eq = Equalizer::new(44100, &[band]).unwrap();
        // Mono 1 kHz sine
        let original = sine_mono(1000.0, 0.5, 44100, 4410);
        let rms_before = rms(&original);
        let mut buf = original.clone();
        eq.process(&mut buf, 1);
        let rms_after = rms(&buf);
        assert!(
            rms_after > rms_before,
            "+12 dB at 1 kHz should increase RMS of 1 kHz sine: before={rms_before}, after={rms_after}"
        );
    }
}
