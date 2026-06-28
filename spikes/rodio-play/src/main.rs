//! Phase-0 spike A: decode + play an audio file end-to-end via rodio 0.22.2.
//! API (verified against rodio 0.22.2 examples/music_wav.rs):
//!   open_default_sink() -> mixer() -> Player::connect_new(mixer)
//!   -> Decoder::try_from(file) -> append -> sleep_until_end.

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: lyra-rodio-spike <AUDIO_FILE>")?;

    let stream_handle = rodio::DeviceSinkBuilder::open_default_sink()?;
    let player = rodio::Player::connect_new(stream_handle.mixer());

    println!("Playing: {path}");
    let file = std::fs::File::open(&path)?;
    player.append(rodio::Decoder::try_from(file)?);

    // Keep `stream_handle` in scope until playback ends, or audio cuts off.
    player.sleep_until_end();
    println!("Done.");
    Ok(())
}
