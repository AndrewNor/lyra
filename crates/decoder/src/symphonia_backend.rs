//! Symphonia-backed audio decoder implementation.

use std::path::Path;

use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::formats::probe::Hint;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::{AudioSpec, Decoder, Error, Result};

/// A decoder that uses Symphonia to read and decode any supported audio file
/// format.  On construction, it probes the file format, selects the default
/// audio track, and creates a codec decoder.  `next_chunk` drives the
/// packet-level decode loop.
pub struct SymphoniaDecoder {
    /// The format (demux) reader — boxed because the concrete type is erased.
    format: Box<dyn FormatReader>,
    /// The codec decoder.
    codec: Box<dyn AudioDecoder>,
    /// Audio specification derived from codec parameters.
    spec: AudioSpec,
    /// Track ID to filter packets for our chosen track.
    track_id: u32,
    /// Set to true once we've seen the end-of-stream from the format reader.
    done: bool,
}

impl SymphoniaDecoder {
    /// Open the file at `path`, probe its format, and instantiate a decoder
    /// for the default audio track.
    pub fn open(path: &Path) -> Result<Self> {
        // Build a hint from the file extension so the probe has a head-start.
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        // Open the file and wrap it in a MediaSourceStream.
        let file = std::fs::File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Probe for a container format.
        let format = symphonia::default::get_probe()
            .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
            .map_err(Error::Symphonia)?;

        // Select the default audio track.
        let track = format
            .default_track(TrackType::Audio)
            .ok_or(Error::NoAudioTrack)?;

        let track_id = track.id;

        // Extract audio codec parameters.
        let audio_params = match track.codec_params.as_ref() {
            Some(CodecParameters::Audio(p)) => p,
            _ => return Err(Error::UnsupportedParams),
        };

        // Build our AudioSpec.  Both fields must be present; if they're
        // missing the file is malformed enough that we can't usefully decode.
        let sample_rate = audio_params.sample_rate.ok_or(Error::UnsupportedParams)?;
        let channels = audio_params
            .channels
            .as_ref()
            .map(|c| c.count() as u16)
            .ok_or(Error::UnsupportedParams)?;

        let spec = AudioSpec { sample_rate, channels };

        // Instantiate the codec decoder.
        let codec = symphonia::default::get_codecs()
            .make_audio_decoder(audio_params, &AudioDecoderOptions::default())
            .map_err(Error::Symphonia)?;

        Ok(Self { format, codec, spec, track_id, done: false })
    }

    /// Convert a `GenericAudioBufferRef` to an interleaved `Vec<f32>`.
    fn buffer_to_f32(buf: GenericAudioBufferRef<'_>) -> Vec<f32> {
        let mut out = Vec::new();
        buf.copy_to_vec_interleaved::<f32>(&mut out);
        out
    }
}

impl Decoder for SymphoniaDecoder {
    fn spec(&self) -> AudioSpec {
        self.spec
    }

    fn next_chunk(&mut self) -> Result<Option<Vec<f32>>> {
        if self.done {
            return Ok(None);
        }

        loop {
            // Pull the next packet from the demuxer.
            let packet = match self.format.next_packet() {
                Ok(Some(pkt)) => pkt,
                Ok(None) => {
                    self.done = true;
                    return Ok(None);
                }
                Err(SymphoniaError::ResetRequired) => {
                    // Re-sync: reset the codec and continue.
                    self.codec.reset();
                    continue;
                }
                Err(e) => return Err(Error::Symphonia(e)),
            };

            // Skip packets that belong to other tracks.
            if packet.track_id != self.track_id {
                continue;
            }

            // Decode the packet.
            let buf = match self.codec.decode(&packet) {
                Ok(buf) => buf,
                Err(SymphoniaError::ResetRequired) => {
                    self.codec.reset();
                    continue;
                }
                Err(SymphoniaError::DecodeError(_)) => {
                    // Undecodeable packet — skip and continue.
                    continue;
                }
                Err(e) => return Err(Error::Symphonia(e)),
            };

            // Convert to interleaved f32.
            let samples = Self::buffer_to_f32(buf);
            if samples.is_empty() {
                // Some decoders emit an empty first buffer during priming — skip it.
                continue;
            }

            return Ok(Some(samples));
        }
    }
}
