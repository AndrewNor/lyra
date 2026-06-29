//! Audio decoder trait and symphonia-based implementation.

mod symphonia_backend;

pub use symphonia_backend::SymphoniaDecoder;

use thiserror::Error;

/// Audio specification: sample rate and channel count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

/// Errors produced by the decoder layer.
#[derive(Debug, Error)]
pub enum Error {
    #[error("symphonia error: {0}")]
    Symphonia(#[from] symphonia::core::errors::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("no audio track found")]
    NoAudioTrack,

    #[error("unsupported codec parameters")]
    UnsupportedParams,
}

/// Shorthand result type for the decoder layer.
pub type Result<T> = std::result::Result<T, Error>;

/// A streaming audio decoder.
///
/// Implementors open a media file and yield interleaved `f32` samples in
/// per-packet chunks.  The channel layout and sample rate are available via
/// [`spec`](Decoder::spec).
pub trait Decoder {
    /// Return the audio specification (sample rate + channel count) for this
    /// stream.  The spec is determined at open time and is stable for the
    /// lifetime of the decoder.
    fn spec(&self) -> AudioSpec;

    /// Decode the next chunk of audio.
    ///
    /// Returns `Ok(Some(samples))` where `samples` is an interleaved `f32`
    /// vector (channels × frames).  Returns `Ok(None)` when the stream is
    /// exhausted.  Any other error is propagated as `Err(…)`.
    fn next_chunk(&mut self) -> Result<Option<Vec<f32>>>;

    /// Seek to `secs` seconds from the start of the stream.
    ///
    /// Uses coarse (keyframe) seeking: the actual position may be slightly
    /// before the requested time.  On success returns the actual timestamp
    /// that was seeked to (in the stream's timebase ticks).
    ///
    /// Callers should call [`next_chunk`] immediately after a seek; any
    /// in-flight audio from before the seek is the caller's responsibility
    /// to discard.
    fn seek_to_secs(&mut self, secs: f64) -> Result<()>;
}
