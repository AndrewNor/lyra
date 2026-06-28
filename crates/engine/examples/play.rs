//! Smoke-test example: play a file for up to 8 seconds, then stop.
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
    println!("[play] Opening engine…");

    let mut engine = lyra_engine::Engine::new()?;
    println!("[play] Engine state: {:?}", engine.state());

    println!("[play] Playing: {}", path.display());
    engine.play(path)?;
    println!("[play] Playback started, state: {:?}", engine.state());

    thread::sleep(Duration::from_secs(8));

    engine.stop();
    println!("[play] Stopped. Final state: {:?}", engine.state());
    Ok(())
}
