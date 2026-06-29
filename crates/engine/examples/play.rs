//! Smoke-test example: play a file for ~2 seconds, seek to 30s, confirm position
//! jumps, then play for a few more seconds and stop.
//!
//! Usage:
//!   cargo run -p lyra-engine --example play -- /path/to/file.mp3

use std::path::Path;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <audio-file>", args[0]);
        std::process::exit(1);
    }

    let path = Path::new(&args[1]);
    println!("[play] Opening engine...");

    let mut engine = lyra_engine::Engine::new()?;
    println!("[play] Engine state: {:?}", engine.state());

    println!("[play] Playing: {}", path.display());
    engine.play(path)?;
    println!("[play] Playback started, state: {:?}", engine.state());
    println!("[play] Sample rate: {} Hz", engine.device_sample_rate());

    // Play for 2 seconds so we can confirm normal playback advancing.
    for i in 1..=2u64 {
        thread::sleep(Duration::from_secs(1));
        let pos = engine.position_secs();
        println!("[play] pre-seek position: {:.2}s (tick {})", pos, i);
    }

    // Seek to 30 seconds.
    println!("[play] Seeking to 30s...");
    engine.seek(30.0)?;

    // Give the seek time to complete (decode thread processes it).
    thread::sleep(Duration::from_millis(500));

    // Confirm position has jumped to ~30s and continues advancing.
    for i in 1..=5u64 {
        thread::sleep(Duration::from_secs(1));
        let pos = engine.position_secs();
        println!("[play] post-seek position: {:.2}s (tick {})", pos, i);
    }

    engine.stop();
    println!("[play] Stopped. Final state: {:?}", engine.state());
    Ok(())
}
