//! Real-time spectrum analyzer: audio tap → FFT → 24 log-spaced band levels.
//!
//! # Architecture
//! The cpal output callback (RT thread) taps playing audio into a bounded
//! lock-free `rtrb` SPSC ring (viz ring).  If the ring is full the tap is
//! silently skipped — the analyzer only needs *recent* samples.
//!
//! A dedicated analyzer thread pops from the viz ring, accumulates an FFT
//! frame (~1024 samples), runs `rustfft`, folds the magnitude spectrum into
//! 24 logarithmically-spaced bands, applies exponential smoothing/decay, and
//! stores the 24 f32 levels (0.0..=1.0) as AtomicU32 bits in a shared array.
//!
//! # RT safety
//! The only RT-callback work is a single `rtrb::Producer::push()` call per
//! output sample — wait-free, no allocation, no locking, no logging.
//! Everything else (FFT, smoothing, storage) happens on the analyzer thread.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

// ── Public constants ──────────────────────────────────────────────────────────

/// Number of spectrum bands exposed to the UI.
pub const NUM_BANDS: usize = 24;

/// Viz ring capacity in mono f32 samples.  ≈ 0.5 s at 48 kHz / 2 ch.
/// Power of two required by rtrb.
pub(crate) const VIZ_RING_SAMPLES: usize = 1 << 14; // 16 384

// ── Band levels store ────────────────────────────────────────────────────────

/// Shared array of 24 band levels, stored as f32 bits in AtomicU32.
/// The analyzer thread writes; `Engine::spectrum_levels()` reads.
pub type SpectrumLevels = Arc<[AtomicU32; NUM_BANDS]>;

// ── SpectrumAnalyzer ─────────────────────────────────────────────────────────

/// Holds the stop/join handle for the analyzer thread.
/// Separate from the viz ring producer so the producer can be moved into the
/// RT callback while this handle stays in `Engine`.
pub struct SpectrumAnalyzerHandle {
    /// Stop signal for the analyzer thread.
    pub(crate) stop: Arc<AtomicBool>,
    /// Analyzer thread join handle.
    pub(crate) handle: Option<thread::JoinHandle<()>>,
}

impl Drop for SpectrumAnalyzerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Create the viz ring and spawn the analyzer thread.
///
/// Returns `(producer, handle)` where `producer` is to be moved into the
/// cpal RT callback, and `handle` is to be stored in `Engine`.
///
/// `levels`      is the shared `SpectrumLevels` Arc the analyzer writes into.
/// `sample_rate` is the device sample rate (used to build log-band boundaries).
/// `channels`    is the output channel count.
pub fn start_analyzer(
    levels: SpectrumLevels,
    sample_rate: u32,
    channels: u16,
) -> (rtrb::Producer<f32>, SpectrumAnalyzerHandle) {
    let (viz_producer, viz_consumer) = rtrb::RingBuffer::<f32>::new(VIZ_RING_SAMPLES);

    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop);

    let handle = thread::Builder::new()
        .name("lyra-spectrum".into())
        .spawn(move || {
            analyzer_loop(viz_consumer, levels, stop_clone, sample_rate, channels);
        })
        .ok(); // If spawn fails, the visualizer just won't update — non-fatal.

    let analyzer_handle = SpectrumAnalyzerHandle { stop, handle };
    (viz_producer, analyzer_handle)
}

// ── Analyzer loop ─────────────────────────────────────────────────────────────

/// FFT frame size in samples.  Gives good frequency resolution (~47 Hz bins
/// at 48 kHz) while staying fast.
const FFT_SIZE: usize = 1024;

/// Smoothing factor for rising levels (fast attack).
const ATTACK: f32 = 0.85;
/// Smoothing factor for falling levels (slow decay).
const DECAY: f32 = 0.12;

fn analyzer_loop(
    mut consumer: rtrb::Consumer<f32>,
    levels: SpectrumLevels,
    stop: Arc<AtomicBool>,
    sample_rate: u32,
    _channels: u16,
) {
    let mut planner = FftPlanner::<f32>::new();
    let fft: Arc<dyn Fft<f32>> = planner.plan_fft_forward(FFT_SIZE);

    // Pre-compute a Hann window.
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|i| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (FFT_SIZE - 1) as f32).cos())
        })
        .collect();

    // Build log-spaced band edges (Hz).  24 bands from 20 Hz to Nyquist/2.
    let nyquist = sample_rate as f32 / 2.0;
    let lo_hz = 20.0_f32;
    let hi_hz = nyquist.min(20_000.0);
    let band_edges_hz: Vec<f32> = (0..=NUM_BANDS)
        .map(|i| lo_hz * (hi_hz / lo_hz).powf(i as f32 / NUM_BANDS as f32))
        .collect();

    // Convert Hz edges to FFT bin indices.
    let hz_per_bin = sample_rate as f32 / FFT_SIZE as f32;
    let band_bins: Vec<(usize, usize)> = (0..NUM_BANDS)
        .map(|i| {
            let lo = ((band_edges_hz[i] / hz_per_bin).floor() as usize).max(1);
            let hi = ((band_edges_hz[i + 1] / hz_per_bin).ceil() as usize)
                .min(FFT_SIZE / 2)
                .max(lo + 1);
            (lo, hi)
        })
        .collect();

    let mut buf: Vec<Complex<f32>> = vec![Complex::default(); FFT_SIZE];
    let mut scratch: Vec<Complex<f32>> = vec![Complex::default(); fft.get_inplace_scratch_len()];
    let mut accum: Vec<f32> = Vec::with_capacity(FFT_SIZE);
    let mut smoothed: [f32; NUM_BANDS] = [0.0; NUM_BANDS];

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // Drain what's available from the viz ring into accum.
        let available = consumer.slots();
        if available == 0 {
            // Nothing to process — apply decay and sleep.
            let any_nonzero = smoothed.iter().any(|&v| v > 0.0001);
            if any_nonzero {
                for v in smoothed.iter_mut() {
                    *v *= DECAY;
                    if *v < 0.0001 {
                        *v = 0.0;
                    }
                }
                write_levels(&levels, &smoothed);
            }
            thread::sleep(Duration::from_millis(16));
            continue;
        }

        for _ in 0..available {
            if let Ok(s) = consumer.pop() {
                accum.push(s);
            }
        }

        // Process as many complete FFT frames as we have.
        while accum.len() >= FFT_SIZE {
            // Apply window and copy into complex buffer.
            for (i, (&s, w)) in accum.iter().take(FFT_SIZE).zip(window.iter()).enumerate() {
                buf[i] = Complex { re: s * w, im: 0.0 };
            }
            accum.drain(..FFT_SIZE);

            // Forward FFT in-place.
            fft.process_with_scratch(&mut buf, &mut scratch);

            // Compute magnitude (normalised by FFT size).
            let norm = 1.0 / FFT_SIZE as f32;
            let magnitudes: Vec<f32> = buf[..FFT_SIZE / 2]
                .iter()
                .map(|c| c.norm() * norm)
                .collect();

            // Fold into log bands, applying dB-like compression → 0..1.
            for (i, &(lo, hi)) in band_bins.iter().enumerate() {
                let count = (hi - lo).max(1) as f32;
                let rms: f32 = magnitudes[lo..hi].iter().map(|&m| m * m).sum::<f32>() / count;
                let rms = rms.sqrt();

                // Convert to approximate dB, map to 0..1.
                // 0 dB (rms=1.0 for full-scale) → 1.0; silence → 0.0.
                // Practical range: ~−60 dB to 0 dB.
                let db = 20.0 * rms.max(1e-9).log10();
                let level = ((db + 60.0) / 60.0).clamp(0.0, 1.0);

                // Exponential smoothing (attack/decay).
                let prev = smoothed[i];
                smoothed[i] = if level > prev {
                    ATTACK * prev + (1.0 - ATTACK) * level
                } else {
                    DECAY * prev + (1.0 - DECAY) * level
                };
            }

            write_levels(&levels, &smoothed);
        }
    }

    // Silence all bands on exit.
    let zeros = [0.0f32; NUM_BANDS];
    write_levels(&levels, &zeros);
}

/// Write smoothed levels to the shared AtomicU32 array.
#[inline]
fn write_levels(levels: &SpectrumLevels, values: &[f32; NUM_BANDS]) {
    for (atom, &v) in levels.iter().zip(values.iter()) {
        atom.store(v.to_bits(), Ordering::Relaxed);
    }
}
