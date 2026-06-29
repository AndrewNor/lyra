//! lyra-cli — interactive REPL over the real Lyra engine.
//!
//! Commands:
//!   scan [dir]       scan a directory (default ~/Music) and print a ScanSummary
//!   list [n]         list first n tracks (default 30); caches results
//!   search <query>   full-text search; caches results
//!   play <i>         play the i-th cached result (1-based)
//!   pause            pause playback
//!   resume           resume playback
//!   stop             stop playback
//!   next             advance queue and play next track
//!   prev             go back in queue and play previous track
//!   status           show playback state + current track
//!   help             print help
//!   quit / exit      exit

mod cmd;

use std::collections::HashMap;
use std::io::{self, BufRead, Write as IoWrite};
use std::path::{Path, PathBuf};

use lyra_core::PlayQueue;
use lyra_db::Track;
use lyra_engine::{Engine, PlaybackState};

use cmd::Command;

// ── XDG path resolution ──────────────────────────────────────────────────────

fn data_dir() -> PathBuf {
    // $XDG_DATA_HOME / lyra  or  ~/.local/share/lyra
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        let p = PathBuf::from(xdg);
        if p.is_absolute() {
            return p.join("lyra");
        }
    }
    // Fallback: HOME
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".local/share/lyra")
}

fn default_music_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join("Music")
}

// ── State ────────────────────────────────────────────────────────────────────

struct App {
    db: lyra_db::Db,
    engine: Option<Engine>,
    queue: PlayQueue,
    /// Last list/search result — the source of truth for play indices.
    cache: Vec<Track>,
    /// id → path for fast lookup during next/prev.
    id_map: HashMap<i64, String>,
    /// The track currently known to be playing (by id).
    current_id: Option<i64>,
}

impl App {
    fn new(db: lyra_db::Db) -> Self {
        // Engine creation can fail if no audio device is present.
        let engine = match Engine::new() {
            Ok(e) => {
                println!("Audio device opened.");
                Some(e)
            }
            Err(e) => {
                println!("Warning: could not open audio device ({e}). Playback disabled.");
                None
            }
        };
        Self {
            db,
            engine,
            queue: PlayQueue::new(),
            cache: Vec::new(),
            id_map: HashMap::new(),
            current_id: None,
        }
    }

    // ── cache helpers ────────────────────────────────────────────────────────

    fn load_cache(&mut self, tracks: Vec<Track>) {
        self.id_map.clear();
        for t in &tracks {
            self.id_map.insert(t.id, t.path.clone());
        }
        self.cache = tracks;
    }

    fn print_cache_range(&self, n: usize) {
        if self.cache.is_empty() {
            println!("(no results)");
            return;
        }
        let limit = n.min(self.cache.len());
        for (i, t) in self.cache.iter().take(limit).enumerate() {
            let artist = t.artist.as_deref().unwrap_or("Unknown Artist");
            println!("[{}] {} — {}", i + 1, t.title, artist);
        }
        if self.cache.len() > limit {
            println!("  … and {} more", self.cache.len() - limit);
        }
    }

    // ── playback helpers ─────────────────────────────────────────────────────

    fn play_path(&mut self, path: &str, id: i64) {
        let p = Path::new(path);
        match &mut self.engine {
            Some(engine) => match engine.play(p) {
                Ok(()) => {
                    self.current_id = Some(id);
                    // Print current track info
                    if let Some(track) = self.cache.iter().find(|t| t.id == id) {
                        let artist = track.artist.as_deref().unwrap_or("Unknown Artist");
                        println!("▶ {} — {}", track.title, artist);
                    } else {
                        println!("▶ {path}");
                    }
                }
                Err(e) => println!("Play error: {e}"),
            },
            None => println!("No audio device available."),
        }
    }

    // ── command handlers ─────────────────────────────────────────────────────

    fn cmd_scan(&mut self, dir: Option<String>) {
        let root = dir
            .map(|d| PathBuf::from(shellexpand_tilde(&d)))
            .unwrap_or_else(default_music_dir);
        println!("Scanning {} …", root.display());
        let art_dir = {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".cache").join("lyra").join("art")
        };
        let _ = std::fs::create_dir_all(&art_dir);
        let art = lyra_library::ArtCache::new(art_dir);
        match lyra_library::scan(&root, &mut self.db, &art) {
            Ok(summary) => {
                println!(
                    "Scan complete: added={} updated={} unchanged={} failed={}",
                    summary.added, summary.updated, summary.unchanged, summary.failed
                );
                match self.db.list_tracks() {
                    Ok(tracks) => println!("Total tracks in library: {}", tracks.len()),
                    Err(e) => println!("Could not count tracks: {e}"),
                }
            }
            Err(e) => println!("Scan error: {e}"),
        }
    }

    fn cmd_list(&mut self, n: usize) {
        match self.db.list_tracks() {
            Ok(tracks) => {
                let count = tracks.len();
                self.load_cache(tracks);
                println!("Library has {count} tracks:");
                self.print_cache_range(n);
            }
            Err(e) => println!("List error: {e}"),
        }
    }

    fn cmd_search(&mut self, query: &str) {
        match self.db.search(query) {
            Ok(tracks) => {
                let count = tracks.len();
                self.load_cache(tracks);
                println!("Found {count} results for {:?}:", query);
                self.print_cache_range(count);
            }
            Err(e) => println!("Search error: {e}"),
        }
    }

    fn cmd_play(&mut self, idx: usize) {
        if self.cache.is_empty() {
            println!("No cached results. Run `list` or `search` first.");
            return;
        }
        if idx == 0 || idx > self.cache.len() {
            println!("Index {idx} out of range (1–{}).", self.cache.len());
            return;
        }
        // Set up the queue with all cached ids, jump to the chosen position.
        let ids: Vec<i64> = self.cache.iter().map(|t| t.id).collect();
        self.queue.set_items(ids);
        self.queue.jump_to(idx - 1); // set_items positions at 0; jump_to adjusts

        let track = self.cache[idx - 1].clone();
        let path = track.path.clone();
        let id = track.id;
        self.play_path(&path, id);
    }

    fn cmd_next(&mut self) {
        match self.queue.next() {
            Some(id) => {
                let path = match self.id_map.get(&id) {
                    Some(p) => p.clone(),
                    None => {
                        println!("Could not resolve path for track id {id}.");
                        return;
                    }
                };
                self.play_path(&path, id);
            }
            None => println!("No next track (end of queue)."),
        }
    }

    fn cmd_prev(&mut self) {
        match self.queue.prev() {
            Some(id) => {
                let path = match self.id_map.get(&id) {
                    Some(p) => p.clone(),
                    None => {
                        println!("Could not resolve path for track id {id}.");
                        return;
                    }
                };
                self.play_path(&path, id);
            }
            None => println!("No previous track (beginning of queue)."),
        }
    }

    fn cmd_pause(&mut self) {
        match &mut self.engine {
            Some(e) => {
                e.pause();
                println!("Paused.");
            }
            None => println!("No audio device."),
        }
    }

    fn cmd_resume(&mut self) {
        match &mut self.engine {
            Some(e) => {
                e.resume();
                println!("Resumed.");
            }
            None => println!("No audio device."),
        }
    }

    fn cmd_stop(&mut self) {
        match &mut self.engine {
            Some(e) => {
                e.stop();
                self.current_id = None;
                println!("Stopped.");
            }
            None => println!("No audio device."),
        }
    }

    fn cmd_status(&self) {
        let state = self
            .engine
            .as_ref()
            .map(|e| e.state())
            .unwrap_or(PlaybackState::Stopped);
        let state_str = match state {
            PlaybackState::Playing => "Playing",
            PlaybackState::Paused => "Paused",
            PlaybackState::Stopped => "Stopped",
        };
        print!("State: {state_str}");
        if let Some(id) = self.current_id {
            if let Some(track) = self.cache.iter().find(|t| t.id == id) {
                let artist = track.artist.as_deref().unwrap_or("Unknown Artist");
                print!("  |  {} — {}", track.title, artist);
            }
        }
        println!();
    }
}

// ── Shell tilde expansion (simple HOME replacement) ──────────────────────────

fn shellexpand_tilde(s: &str) -> String {
    if s.starts_with("~/") || s == "~" {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}{}", home, &s[1..])
    } else {
        s.to_string()
    }
}

// ── Help text ────────────────────────────────────────────────────────────────

fn print_help() {
    println!(
        r#"Commands:
  scan [dir]       Scan directory for audio (default: ~/Music)
  list [n]         List first n tracks from library (default: 30)
  search <query>   Full-text search the library
  play <i>         Play the i-th result from last list/search (1-based)
  pause            Pause playback
  resume           Resume playback
  stop             Stop playback
  next             Play next track in queue
  prev             Play previous track in queue
  status           Show playback state
  help             Show this help
  quit / exit      Exit"#
    );
}

// ── REPL ─────────────────────────────────────────────────────────────────────

fn main() {
    // Resolve persistent DB path.
    let db_dir = data_dir();
    if let Err(e) = std::fs::create_dir_all(&db_dir) {
        eprintln!("Could not create data directory {}: {e}", db_dir.display());
        std::process::exit(1);
    }
    let db_path = db_dir.join("library.db");

    let db = match lyra_db::Db::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open database at {}: {e}", db_path.display());
            std::process::exit(1);
        }
    };

    println!("╔═══════════════════════════════╗");
    println!("║        Lyra CLI  v0.1         ║");
    println!("╚═══════════════════════════════╝");
    println!("Database: {}", db_path.display());
    println!();
    print_help();
    println!();

    let mut app = App::new(db);
    let stdin = io::stdin();

    loop {
        // Prompt (only shown when stdin is a tty, but we print it regardless
        // for interactive use; non-interactive pipes just ignore the extra output).
        print!("> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match cmd::parse(line) {
            Command::Scan(dir) => app.cmd_scan(dir),
            Command::List(n) => app.cmd_list(n),
            Command::Search(query) => app.cmd_search(&query),
            Command::Play(idx) => app.cmd_play(idx),
            Command::Pause => app.cmd_pause(),
            Command::Resume => app.cmd_resume(),
            Command::Stop => app.cmd_stop(),
            Command::Next => app.cmd_next(),
            Command::Prev => app.cmd_prev(),
            Command::Status => app.cmd_status(),
            Command::Help => print_help(),
            Command::Quit => {
                // Stop engine cleanly before exit.
                if let Some(ref mut e) = app.engine {
                    e.stop();
                }
                println!("Goodbye.");
                break;
            }
            Command::Unknown(s) => {
                println!("Unknown command: {s:?}. Type `help` for commands.");
            }
        }
    }
}
