//! Digital signal processing: loudness analysis, parametric EQ, and resampling.

pub mod eq;
pub mod loudness;
pub mod resample;

pub use eq::{EqBand, Equalizer};
pub use loudness::{analyze_lufs, db_to_linear, replaygain_gain_db};
pub use resample::{resample, StreamResampler};

/// Errors produced by `lyra-dsp`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("EBU R128 error: {0}")]
    Ebur128(String),

    #[error("EQ parameter error: {0}")]
    EqParam(String),

    #[error("Resampler error: {0}")]
    Resample(String),
}

pub type Result<T> = std::result::Result<T, Error>;
