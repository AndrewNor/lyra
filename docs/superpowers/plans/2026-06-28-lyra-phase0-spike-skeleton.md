# Lyra Phase 0 — Spike & Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the Lyra cargo workspace and prove the chosen stack end-to-end — a Rust core driving a CXX-Qt/Kirigami window that is themed by Breeze, plus audio out through PipeWire — then pass two go/no-go gates before committing to Phase 1.

**Architecture:** A cargo workspace of pure-Rust logic crates (no Qt, fast `cargo test`) plus a CXX-Qt 0.8.1 `lyra-ui` staticlib that CMake/Corrosion links into a small C++/Qt executable. Audio is de-risked with two throwaway spike crates: one plays a file via `rodio`, one enumerates the PipeWire output device via `cpal`.

**Tech Stack:** Rust 1.93 · CXX-Qt 0.8.1 (+ cxx-qt-cmake v0.8.1, Corrosion v0.5) · Qt6 ≥6.8 + KF6 Kirigami 6.x · CMake ≥3.28 / Ninja · cpal =0.18.1 (`pipewire` feature) · rodio =0.22.2 · KDE Plasma 6 / Wayland / PipeWire 1.4.2.

## Global Constraints

These apply to **every** task. Values are copied verbatim from the design spec and the verified scaffolding research.

- **Pin the CXX-Qt family EXACTLY** — `cxx-qt = "=0.8.1"`, `cxx-qt-lib = "=0.8.1"`, `cxx-qt-build = "=0.8.1"`, and the `cxx-qt-cmake` FetchContent `GIT_TAG v0.8.1`. Use `=` (exact), never caret: the macro/build API differs across 0.6/0.7/0.8/0.9. The live cxx-qt book is on 0.9 and shows a *different* build API — do not copy 0.9 snippets.
- **Audio pins** — `cpal = { version = "=0.18.1", features = ["pipewire"] }`, `rodio = "=0.22.2"`.
- **QML URI `ai.drivee.lyra` must match byte-for-byte** in `crates/ui/build.rs`, the top-level `CMakeLists.txt`, and `crates/ui/qml/Main.qml`. A mismatch fails at **runtime** (`module "..." is not installed`), not at build time.
- **Workspace `resolver = "2"`** (the feature-heavy `lyra-ui` deps must not unify into the pure crates).
- **Versions floors** — Qt6 ≥ 6.8, KF6 Kirigami ≥ 6.0, ECM ≥ 6.0, CMake ≥ 3.28 (3.31 present), Rust ≥ 1.93 (rodio MSRV 1.87, cpal MSRV 1.85).
- **`CMAKE_AUTOMOC ON`** — CXX-Qt emits headers with `Q_OBJECT`; without moc you get undefined-reference link errors that masquerade as Rust/Corrosion failures.
- **Two build paths, on purpose:** the **windowed app is built by CMake** (`cmake --build build && ./build/lyra`) — `cargo run` is *not* the supported path for the UI under CXX-Qt 0.8.x. **Pure logic crates build/test with plain cargo** (`cargo test -p lyra-core`), no Qt/CMake/qmake. This division is the whole point of Gate 2.
- **Runtime target:** KDE Plasma 6 / Wayland, PipeWire 1.4.2. Launch the UI with `QT_QPA_PLATFORM=wayland` if the platform plugin is not auto-selected.
- **Project root:** `/home/andrew/Documents/Personal Projects/lyra` (already a git repo). All paths below are **relative to it**; `cd` there first. The path contains a space — cargo/cmake handle it, but always quote it in shell commands.

## File Structure

```
lyra/
├── Cargo.toml                      # [workspace] resolver=2, members, [workspace.dependencies]
├── CMakeLists.txt                  # CMake-drives-Cargo: Corrosion + cxx-qt-cmake -> lyra-ui -> ./build/lyra
├── .gitignore                      # /target, /build, .superpowers/  (Cargo.lock IS committed — app, not lib)
├── PHASE0-RESULTS.md               # Gate 1 + Gate 2 evidence and the keep-Kirigami-or-pivot-to-Slint decision
├── crates/
│   ├── core/                       # lyra-core: pure domain logic, zero deps, the fast-test anchor
│   │   ├── Cargo.toml
│   │   └── src/lib.rs              # next_index() + RepeatMode (TDD)
│   └── ui/                         # lyra-ui: CXX-Qt 0.8.1 staticlib (the only Qt-touching crate)
│       ├── Cargo.toml
│       ├── build.rs               # CxxQtBuilder + QmlModule { uri: "ai.drivee.lyra" }
│       ├── src/lib.rs             # declares `pub mod bridge;`
│       ├── src/bridge.rs          # #[cxx_qt::bridge] LibraryController QObject
│       ├── cpp/main.cpp           # tiny C++ entrypoint: loadFromModule("ai.drivee.lyra","Main")
│       └── qml/Main.qml           # Kirigami.ApplicationWindow + ListView bound to Rust
└── spikes/                         # throwaway; delete after Gate 1 passes
    ├── rodio-play/                 # lyra-rodio-spike: play an audio file end-to-end
    │   ├── Cargo.toml
    │   └── src/main.rs
    └── cpal-pipewire/              # lyra-cpal-spike: enumerate the PipeWire output device
        ├── Cargo.toml
        └── src/main.rs
```

**Decomposition rationale:** all logic lives in pure crates (`core` now; `engine`/`db`/`library`/… arrive in Phase 1) that never import Qt, so the day-to-day loop is fast `cargo` with rust-analyzer. Only `lyra-ui` touches CXX-Qt, and only CMake builds it. The spikes are isolated and disposable.

---

### Task 0: Workspace skeleton, toolchain, and system dependencies

Creates a fully valid workspace (all member crates exist as compiling stubs) and installs everything the later tasks need. Ends with `cargo metadata` succeeding and `qmake6` on PATH.

**Files:**
- Modify: `.gitignore`
- Create: `Cargo.toml` (workspace root)
- Create: `crates/core/Cargo.toml`, `crates/core/src/lib.rs` (stub)
- Create: `crates/ui/Cargo.toml`, `crates/ui/src/lib.rs` (stub)
- Create: `spikes/rodio-play/Cargo.toml`, `spikes/rodio-play/src/main.rs` (stub)
- Create: `spikes/cpal-pipewire/Cargo.toml`, `spikes/cpal-pipewire/src/main.rs` (stub)

**Interfaces:**
- Produces: a valid `[workspace]` with members `crates/core`, `crates/ui`, `spikes/rodio-play`, `spikes/cpal-pipewire`; a `[workspace.dependencies]` block pinning `cxx-qt*`, `cpal`, `rodio`. Later tasks inherit these pins with `dep = { workspace = true }`.

- [ ] **Step 1: Install system build + runtime dependencies**

Run (one line per the verified research; package names can drift on Debian — see Step 2 fallback):

```bash
sudo apt update && sudo apt install -y \
  build-essential pkg-config ninja-build cmake \
  libasound2-dev libpipewire-0.3-dev libclang-dev clang \
  extra-cmake-modules \
  qt6-base-dev qt6-declarative-dev \
  qml6-module-qtquick qml6-module-qtquick-controls qml6-module-qtquick-layouts \
  qml6-module-qtquick-window qml6-module-qtquick-templates \
  libkf6kirigami-dev qml6-module-org-kde-kirigami kirigami-addons-dev \
  qqc2-desktop-style
```

Why each matters: `libasound2-dev` — cpal compiles the ALSA backend **unconditionally** on Linux even for a pipewire-only build (without it the build panics in `alsa-sys/build.rs`). `libpipewire-0.3-dev` — ships both `libpipewire-0.3.pc` and `libspa-0.2.pc` that cpal's `pipewire` feature probes. `libclang-dev`+`clang` — `pipewire-sys`/`libspa-sys` run `bindgen` at build time and load libclang. `qqc2-desktop-style` — makes QtQuick Controls render with Breeze on Plasma (without it the window is unstyled).

- [ ] **Step 2: Verify the toolchain and dev packages resolved**

```bash
cargo --version && rustc --version && cmake --version && qmake6 --version && pkg-config --exists libpipewire-0.3 && echo "pipewire .pc OK"
```

Expected: `cargo 1.93.x` / `rustc 1.93.x`, `cmake version 3.31.x`, `QMake version ... Using Qt version 6.8.2`, and `pipewire .pc OK`. If `qmake6` is missing, install `qt6-base-dev-tools`. If `libkf6kirigami-dev` or `qml6-module-org-kde-kirigami` 404, run `apt-cache search kirigami` and substitute the names it reports (KF6 Debian names drift; the QML runtime plugin is the load-bearing one).

- [ ] **Step 3: Fix `.gitignore` (Lyra is an application — commit `Cargo.lock`)**

Replace the file contents with:

```gitignore
/target
/build
.superpowers/
```

Rationale: `cxx_qt_import_crate(... LOCKED)` needs a committed `Cargo.lock`; apps commit their lockfile anyway. (The earlier brainstorming `.gitignore` ignored `Cargo.lock` — remove that line.)

- [ ] **Step 4: Create the workspace root `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/core",
    "crates/ui",
    "spikes/rodio-play",
    "spikes/cpal-pipewire",
]

# Single source of truth for shared version pins. Member crates write
# `dep = { workspace = true }` to inherit. Phase 1 adds rusqlite, lofty,
# symphonia, ebur128, rubato, biquad, rtrb, tokio here — NOT in Phase 0.
[workspace.dependencies]
# CXX-Qt family — exact pins; the API differs across 0.6/0.7/0.8/0.9.
cxx          = "1.0"
cxx-qt       = "=0.8.1"
cxx-qt-lib   = { version = "=0.8.1", features = ["qt_full", "link_qt_object_files"] }
cxx-qt-build = { version = "=0.8.1", features = ["link_qt_object_files"] }
# Audio (verified).
cpal  = { version = "=0.18.1", features = ["pipewire"] }
rodio = "=0.22.2"

[profile.dev]
opt-level = 1   # symphonia/audio decode is painfully slow at opt-level=0 (matters in Phase 1)
```

- [ ] **Step 5: Create compiling stubs for all four member crates**

`crates/core/Cargo.toml`:
```toml
[package]
name = "lyra-core"
version = "0.0.0"
edition = "2021"
publish = false
```
`crates/core/src/lib.rs`:
```rust
//! lyra-core: pure domain logic, no I/O, no Qt, no async.
```
`crates/ui/Cargo.toml`:
```toml
[package]
name = "lyra-ui"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
crate-type = ["staticlib", "rlib"]
```
`crates/ui/src/lib.rs`:
```rust
//! lyra-ui: CXX-Qt bridge crate (stub until Task 4).
```
`spikes/rodio-play/Cargo.toml`:
```toml
[package]
name = "lyra-rodio-spike"
version = "0.0.0"
edition = "2021"
publish = false
```
`spikes/rodio-play/src/main.rs`:
```rust
fn main() {}
```
`spikes/cpal-pipewire/Cargo.toml`:
```toml
[package]
name = "lyra-cpal-spike"
version = "0.0.0"
edition = "2021"
publish = false
```
`spikes/cpal-pipewire/src/main.rs`:
```rust
fn main() {}
```

- [ ] **Step 6: Verify the workspace is valid**

```bash
cd "/home/andrew/Documents/Personal Projects/lyra" && cargo metadata --format-version 1 >/dev/null && echo "workspace OK"
```

Expected: `workspace OK` (no "failed to load manifest" errors). This does **not** build the UI (no cxx-qt yet), so it needs no Qt.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "chore: scaffold Lyra cargo workspace + install Phase 0 deps"
```

---

### Task 1: `lyra-core` — `next_index()` play-queue logic (TDD)

The pure, fast-to-test heart. Proves the `cargo test -p lyra-core` inner loop that makes Gate 2 tolerable.

**Files:**
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces: `pub enum RepeatMode { Off, All, One }` and `pub fn next_index(current: usize, len: usize, mode: RepeatMode) -> Option<usize>`. Consumed by `lyra-ui` in Task 4.

- [ ] **Step 1: Write the failing test**

Put this in `crates/core/src/lib.rs` (replacing the stub comment):

```rust
//! lyra-core: pure domain logic, no I/O, no Qt, no async.
//! Unit-testable with plain `cargo test -p lyra-core`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advances_within_queue() {
        assert_eq!(next_index(0, 3, RepeatMode::Off), Some(1));
        assert_eq!(next_index(1, 3, RepeatMode::Off), Some(2));
    }

    #[test]
    fn stops_at_end_when_repeat_off() {
        assert_eq!(next_index(2, 3, RepeatMode::Off), None);
    }

    #[test]
    fn wraps_when_repeat_all() {
        assert_eq!(next_index(2, 3, RepeatMode::All), Some(0));
    }

    #[test]
    fn repeat_one_holds_position() {
        assert_eq!(next_index(1, 3, RepeatMode::One), Some(1));
    }

    #[test]
    fn empty_queue_yields_none() {
        assert_eq!(next_index(0, 0, RepeatMode::All), None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd "/home/andrew/Documents/Personal Projects/lyra" && cargo test -p lyra-core
```

Expected: FAILS to compile — `cannot find function next_index` / `cannot find type RepeatMode`. That is the intended red state.

- [ ] **Step 3: Write the minimal implementation**

Insert above the `#[cfg(test)]` module:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Off,
    All,
    One,
}

/// Next index in a play queue. `None` means "stop" (end of queue with
/// `Off`, or an empty queue). Pure: same inputs -> same output.
pub fn next_index(current: usize, len: usize, mode: RepeatMode) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match mode {
        RepeatMode::One => Some(current),
        RepeatMode::All => Some((current + 1) % len),
        RepeatMode::Off => {
            let next = current + 1;
            if next < len { Some(next) } else { None }
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo test -p lyra-core
```

Expected: `test result: ok. 5 passed; 0 failed`, in well under a second, without touching `lyra-ui` or Qt.

- [ ] **Step 5: Commit**

```bash
git add crates/core && git commit -m "feat(core): next_index play-queue logic with tests"
```

---

### Task 2: Audio spike A — play a file end-to-end with `rodio`

Satisfies the audio half of Gate 1 ("one file plays"). Uses a real MP3 from `~/Music` (the library is ~98% MP3).

**Files:**
- Modify: `spikes/rodio-play/Cargo.toml`, `spikes/rodio-play/src/main.rs`

- [ ] **Step 1: Set the dependency**

`spikes/rodio-play/Cargo.toml`:
```toml
[package]
name = "lyra-rodio-spike"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
rodio = { workspace = true }
```
(rodio 0.22.2's default features bundle a symphonia front-end for wav/flac/mp3/mp4/ogg/vorbis — do **not** add `symphonia` separately here.)

- [ ] **Step 2: Write the player**

`spikes/rodio-play/src/main.rs`:
```rust
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
```

- [ ] **Step 3: Build (verifies the ALSA/audio toolchain from Task 0)**

```bash
cd "/home/andrew/Documents/Personal Projects/lyra" && cargo build -p lyra-rodio-spike
```

Expected: `Compiling rodio v0.22.2 ... Finished`. If it panics in `alsa-sys/build.rs` ("Package alsa was not found"), Task 0's `libasound2-dev` did not install — fix that first.

- [ ] **Step 4: Play a real track and confirm sound**

```bash
TRACK=$(find ~/Music -iname '*.mp3' | head -1) && cargo run -p lyra-rodio-spike -- "$TRACK"
```

Expected: prints `Playing: /home/andrew/Music/...mp3`, **audio is audible**, then `Done.` and exit 0. (This is a manual/observational check — listen for sound. If PipeWire is muted, unmute and rerun.)

- [ ] **Step 5: Commit**

```bash
git add spikes/rodio-play && git commit -m "spike(audio): play a file end-to-end via rodio"
```

---

### Task 3: Audio spike B — enumerate the PipeWire output device with `cpal`

Proves the **load-bearing** `cpal =0.18.1` `pipewire` feature actually builds and runs (a top spec risk — 0.18.1 is the first cpal with that feature).

**Files:**
- Modify: `spikes/cpal-pipewire/Cargo.toml`, `spikes/cpal-pipewire/src/main.rs`

- [ ] **Step 1: Set the dependency**

`spikes/cpal-pipewire/Cargo.toml`:
```toml
[package]
name = "lyra-cpal-spike"
version = "0.0.0"
edition = "2021"
publish = false

[dependencies]
cpal = { workspace = true }   # =0.18.1, features = ["pipewire"]
```

- [ ] **Step 2: Write the enumerator**

`spikes/cpal-pipewire/src/main.rs`:
```rust
//! Phase-0 spike B: prove cpal builds + runs with the `pipewire` feature.
//! cpal 0.18.x: DeviceTrait::name() was REMOVED — Device impls Display;
//! use device.id()/device.description() for structured metadata.

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::HostId;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Hosts compiled in : {:?}", cpal::ALL_HOSTS);
    println!("Hosts available   : {:?}", cpal::available_hosts());

    let host = cpal::host_from_id(HostId::PipeWire)
        .expect("PipeWire host unavailable - is the pipewire daemon running?");
    println!("Selected host     : {}", host.id().name());

    let device = host
        .default_output_device()
        .ok_or("no default output device on the PipeWire host")?;
    println!("Default output    : {device}");
    if let Ok(id) = device.id() {
        println!("Device id         : {id}");
    }

    let cfg = device.default_output_config()?;
    println!("Default out config: {cfg:?}");

    println!("\nAll PipeWire output devices:");
    for (i, dev) in host.output_devices()?.enumerate() {
        println!("  {}. {dev}", i + 1);
    }
    Ok(())
}
```

- [ ] **Step 3: Build (exercises pipewire-sys/libspa-sys/bindgen)**

```bash
cargo build -p lyra-cpal-spike
```

Expected: `Compiling libspa-sys ... Compiling pipewire-sys ... Compiling cpal v0.18.1 ... Finished`. If it fails loading `libclang`, Task 0's `libclang-dev`/`clang` is missing. If it fails `Cannot find libpipewire`, `libpipewire-0.3-dev` is missing.

- [ ] **Step 4: Run and confirm the PipeWire path**

```bash
cargo run -p lyra-cpal-spike
```

Expected: `Selected host : PipeWire`, a default output device name, and `Default out config: SupportedStreamConfig { channels: 2, sample_rate: SampleRate(48000), ..., sample_format: F32 }` (exact device/rate vary). Exit 0.

- [ ] **Step 5: Prove the feature wiring**

```bash
cargo tree -p lyra-cpal-spike -e features -i pipewire
```

Expected: shows `cpal v0.18.1` (feature "pipewire") → `pipewire v0.10.0` → `pipewire-sys` / `libspa-sys`. This is the documented evidence that the `pipewire` feature is real and resolved.

- [ ] **Step 6: Commit**

```bash
git add spikes/cpal-pipewire && git commit -m "spike(audio): enumerate PipeWire output device via cpal 0.18.1"
```

---

### Task 4: `lyra-ui` — the CXX-Qt 0.8.1 Rust bridge

Builds the Rust side of the UI: a `LibraryController` QObject exposing a greeting, a track list (drives a ListView), a core-delegating invokable, and a background-thread demo. Compiling this proves the `#[cxx_qt::bridge]` macro expands and links Qt under 0.8.1.

**Files:**
- Modify: `crates/ui/Cargo.toml`
- Create: `crates/ui/build.rs`, `crates/ui/src/bridge.rs`
- Modify: `crates/ui/src/lib.rs`

**Interfaces:**
- Consumes: `lyra_core::{next_index, RepeatMode}` (Task 1).
- Produces: a `#[qml_element]` QObject `LibraryController` in QML module `ai.drivee.lyra`, with QML-visible members `greeting` (string), `tracks` (string list), `current`/`len` (int), `nextIndex()` (int invokable), `simulateScan()` (invokable). Consumed by `Main.qml`/`main.cpp` in Task 5.

- [ ] **Step 1: Set crate dependencies**

`crates/ui/Cargo.toml`:
```toml
[package]
name = "lyra-ui"
version = "0.0.0"
edition = "2021"
publish = false

[lib]
crate-type = ["staticlib", "rlib"]

[dependencies]
lyra-core  = { path = "../core" }
cxx        = { workspace = true }
cxx-qt     = { workspace = true }
cxx-qt-lib = { workspace = true }

[build-dependencies]
cxx-qt-build = { workspace = true }
```

- [ ] **Step 2: Write `build.rs`**

`crates/ui/build.rs`:
```rust
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    // 0.8.1 form: struct-literal QmlModule (NOT the 0.9 new_qml_module API).
    // URI must match CMakeLists.txt and Main.qml byte-for-byte.
    CxxQtBuilder::new()
        .qml_module(QmlModule {
            uri: "ai.drivee.lyra",
            rust_files: &["src/bridge.rs"],
            qml_files: &["qml/Main.qml"],
            ..Default::default()
        })
        .qt_module("Gui")
        .qt_module("Quick")
        .build();
}
```

- [ ] **Step 3: Write the bridge**

`crates/ui/src/bridge.rs`:
```rust
//! CXX-Qt 0.8.1 bridge. The QObject is declared in `extern "RustQt"`; the
//! backing struct lives OUTSIDE the bridge and is referenced via `super::`.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, greeting)]
        #[qproperty(QStringList, tracks)]
        #[qproperty(i32, current)]
        #[qproperty(i32, len)]
        type LibraryController = super::LibraryControllerRust;

        // Delegates to the pure lyra-core logic. -1 means "stop" (None).
        #[qinvokable]
        #[cxx_name = "nextIndex"]
        fn next_index(self: &LibraryController) -> i32;

        // Background work marshalled back to the Qt thread (future scan demo).
        #[qinvokable]
        #[cxx_name = "simulateScan"]
        fn simulate_scan(self: core::pin::Pin<&mut LibraryController>);
    }

    impl cxx_qt::Threading for LibraryController {}
}

use core::pin::Pin;
use cxx_qt_lib::{QString, QStringList};
use lyra_core::{next_index as core_next_index, RepeatMode};

pub struct LibraryControllerRust {
    greeting: QString,
    tracks: QStringList,
    current: i32,
    len: i32,
}

impl Default for LibraryControllerRust {
    fn default() -> Self {
        let mut tracks = QStringList::default();
        tracks.append(&QString::from("Boards of Canada — Roygbiv"));
        tracks.append(&QString::from("Aphex Twin — Avril 14th"));
        tracks.append(&QString::from("Tycho — Awake"));
        Self {
            greeting: QString::from("Welcome to Lyra"),
            tracks,
            current: 0,
            len: 3,
        }
    }
}

// Methods are implemented on the GENERATED type, not on the Rust struct.
impl qobject::LibraryController {
    fn next_index(&self) -> i32 {
        // qproperty getters return &T; deref to read. If the compiler says a
        // getter already returns i32 by value, drop the `*`.
        let cur = (*self.current()).max(0) as usize;
        let len = (*self.len()).max(0) as usize;
        match core_next_index(cur, len, RepeatMode::All) {
            Some(i) => i as i32,
            None => -1,
        }
    }

    fn simulate_scan(self: Pin<&mut Self>) {
        let thread = self.qt_thread(); // Send+Sync handle, from Threading impl
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            thread
                .queue(|mut qobject| {
                    let mut list = qobject.tracks().clone();
                    list.append(&QString::from("Scanned — New Track"));
                    qobject.as_mut().set_tracks(list);
                })
                .expect("failed to queue onto Qt thread");
        });
    }
}
```

- [ ] **Step 4: Wire the module into `lib.rs`**

`crates/ui/src/lib.rs`:
```rust
//! lyra-ui: thin CXX-Qt bridge between the Rust core and QML/Kirigami.
//! Compiled as a staticlib and linked by CMake/Corrosion into the Qt app.

pub mod bridge;
```

- [ ] **Step 5: Build the staticlib (the macro-expansion gate)**

```bash
cd "/home/andrew/Documents/Personal Projects/lyra" && cargo build -p lyra-ui
```

Expected: `Compiling cxx-qt v0.8.1 ... Compiling lyra-ui ... Finished`. cxx-qt-build invokes moc/qmlcachegen during the build (needs the Qt6 dev packages from Task 0). If a `qproperty` getter type mismatch appears, adjust the `*` deref per the comment in Step 3 — let the compiler guide it.

- [ ] **Step 6: Sanity-check the resolved cxx-qt versions**

```bash
cargo tree -p lyra-ui -i cxx-qt && cargo tree -p lyra-ui -i cxx-qt-lib
```

Expected: both resolve to `0.8.1` exactly (not 0.7/0.9). If not, fix the `=0.8.1` pins.

- [ ] **Step 7: Commit**

```bash
git add crates/ui && git commit -m "feat(ui): CXX-Qt 0.8.1 LibraryController bridge over lyra-core"
```

---

### Task 5: CMake + QML + C++ shim — assemble the Breeze-themed window

The Gate 1 main event: CMake drives Cargo to build `lyra-ui`, links it into a C++/Qt executable, and runs a Kirigami window that inherits Breeze and shows the Rust-driven list.

**Files:**
- Create: `CMakeLists.txt` (root), `crates/ui/cpp/main.cpp`, `crates/ui/qml/Main.qml`

**Interfaces:**
- Consumes: the `ai.drivee.lyra` QML module and `LibraryController` from Task 4.

- [ ] **Step 1: Pull the canonical v0.8.1 reference for the one volatile macro**

The `cxx_qt_import_crate` / QML-module CMake signature is the most version-sensitive piece. Before trusting the template below, open the **pinned** reference and mirror its exact macro call:

```bash
curl -s https://raw.githubusercontent.com/KDAB/cxx-qt/v0.8.1/examples/qml_minimal/CMakeLists.txt -o /tmp/cxxqt-qml_minimal-CMakeLists.txt && sed -n '1,80p' /tmp/cxxqt-qml_minimal-CMakeLists.txt
```

Expected: the v0.8.1 example's CMake. If its `cxx_qt_import_crate` / QML-module call differs from Step 2 below (arg names, the linked module target name), **use the example's form** — it is the source of truth for 0.8.1.

- [ ] **Step 2: Write the root `CMakeLists.txt`**

`CMakeLists.txt`:
```cmake
cmake_minimum_required(VERSION 3.28)
project(lyra LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)   # CXX-Qt emits Q_OBJECT headers; moc is mandatory

find_package(Qt6 6.8 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2)
qt_standard_project_setup(REQUIRES 6.8)

# Corrosion (CMake <-> Cargo). Fetched at configure time (needs network once).
include(FetchContent)
FetchContent_Declare(Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG        v0.5)
FetchContent_MakeAvailable(Corrosion)

# cxx-qt-cmake: provides cxx_qt_import_crate. Pin to v0.8.1 to match the crates.
find_package(CxxQt 0.8 QUIET)
if(NOT CxxQt_FOUND)
    FetchContent_Declare(CxxQt
        GIT_REPOSITORY https://github.com/KDAB/cxx-qt-cmake.git
        GIT_TAG        v0.8.1)
    FetchContent_MakeAvailable(CxxQt)
endif()

# Import ONLY the ui crate (pure crates stay cargo-only). Corrosion runs cargo.
cxx_qt_import_crate(
    MANIFEST_PATH crates/ui/Cargo.toml
    CRATES lyra-ui
    LOCKED
    QML_MODULES "URI=ai.drivee.lyra;SOURCE_CRATE=lyra-ui"
)

qt_add_executable(lyra crates/ui/cpp/main.cpp)
target_link_libraries(lyra PRIVATE
    Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2
    # QML-module target minted by cxx_qt_import_crate. If the configure log
    # prints a different target name, link THAT (see Step 1 reference).
    lyra-ui_ai_drivee_lyra
)
qt_import_qml_plugins(lyra)
```

Note: no `find_package(KF6 Kirigami)` — Kirigami is a **runtime QML import** satisfied by the Task 0 apt packages; a configure-time guard tends to break on the `Kirigami`/`KirigamiPlatform` component-name drift, so we rely on the runtime import + the Step 6 diagnostic instead.

- [ ] **Step 3: Write the C++ entrypoint**

`crates/ui/cpp/main.cpp`:
```cpp
// Lyra Phase 0 — minimal C++ shim. All logic is in Rust; this only spins
// up the Qt event loop and loads the Rust-registered QML module.
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    QQmlApplicationEngine engine;
    engine.loadFromModule("ai.drivee.lyra", "Main");
    if (engine.rootObjects().isEmpty())
        return -1;
    return app.exec();
}
```

- [ ] **Step 4: Write the Kirigami QML**

`crates/ui/qml/Main.qml`:
```qml
// Proves the QML <-> Rust seam: a Q_PROPERTY in a header, a Rust list driving
// a ListView, an invokable delegating to lyra-core, and a background thread.
import QtQuick
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import ai.drivee.lyra

Kirigami.ApplicationWindow {
    id: root
    title: "Lyra (Phase 0 spike)"
    width: 520
    height: 680

    LibraryController { id: controller }

    pageStack.initialPage: Kirigami.ScrollablePage {
        title: "Library"

        // Toolbar button -> background thread -> appends a track ~0.5s later.
        actions: [
            Kirigami.Action {
                text: "Simulate scan"
                icon.name: "media-playback-start"
                onTriggered: controller.simulateScan()
            }
        ]

        // ScrollablePage takes ONE flickable child: the ListView.
        ListView {
            model: controller.tracks   // QStringList -> use `modelData`
            header: Controls.Label {
                width: ListView.view ? ListView.view.width : implicitWidth
                padding: Kirigami.Units.largeSpacing
                font.bold: true
                text: controller.greeting + "   ·   nextIndex() = " + controller.nextIndex()
            }
            delegate: Controls.ItemDelegate {
                width: ListView.view.width
                text: modelData
            }
        }
    }
}
```

- [ ] **Step 5: Configure and build (CMake drives Cargo)**

```bash
cd "/home/andrew/Documents/Personal Projects/lyra"
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Debug -DCMAKE_PREFIX_PATH="$(qmake6 -query QT_INSTALL_PREFIX)"
cmake --build build
```

Expected: configure logs `Found Qt6 ... 6.8.2`, fetches Corrosion + CxxQt (first run only), imports crate `lyra-ui`; the build then shows `Compiling lyra-ui ...` followed by C++ compile/link, producing `./build/lyra`. If linking fails on the module target name, correct it per Step 1/Step 2's note.

- [ ] **Step 6: Run the window**

```bash
QT_QPA_PLATFORM=wayland ./build/lyra
```

Expected: a Kirigami window titled **"Lyra (Phase 0 spike)"** opens on Plasma/Wayland, **styled by Breeze** (system colors/fonts/accent, rounded Breeze controls), showing the bold greeting + `nextIndex() = 1` header above a scrolling list of three seeded tracks. Clicking **Simulate scan** appends "Scanned — New Track" after ~0.5s (proves the `qt_thread()` round-trip). If the window is blank or unstyled, run the diagnostic:

```bash
QT_LOGGING_RULES="qt.qml.import=true" ./build/lyra 2>&1 | head -40
```
A `module "org.kde.kirigami" is not installed` points at a missing runtime package (recheck Task 0); a `module "ai.drivee.lyra" is not installed` points at a URI mismatch across build.rs / CMakeLists / Main.qml.

- [ ] **Step 7: Commit**

```bash
git add CMakeLists.txt crates/ui/cpp crates/ui/qml Cargo.lock && git commit -m "feat(ui): CMake/Kirigami window driven by the Rust bridge"
```

---

### Task 6: GATE 1 — technical go/no-go verification

Not code: a checkpoint that records hard evidence the stack works end-to-end. A reviewer can reject here if any item fails.

**Files:**
- Create: `PHASE0-RESULTS.md`

- [ ] **Step 1: Run the full checklist and capture results**

Verify each, on Plasma 6 / Wayland:

1. `cargo test -p lyra-core` → `5 passed` (pure logic, no Qt). 
2. `cargo run -p lyra-rodio-spike -- <mp3>` → audible sound + `Done.` 
3. `cargo run -p lyra-cpal-spike` → `Selected host : PipeWire` + device + `F32` config. 
4. `./build/lyra` → Kirigami window **visibly themed by Breeze** (toggle System Settings dark/light or accent and relaunch — it should follow). 
5. The window shows the **Rust-driven track list** and `nextIndex() = 1`. 
6. **Simulate scan** appends a track (~0.5s) — background→Qt thread marshalling works.

- [ ] **Step 2: Write `PHASE0-RESULTS.md`**

Record, for each of the 6 items: PASS/FAIL, the exact observed output (paste the cpal config line and a screenshot path for the window), Qt/Kirigami/PipeWire versions in play, and any deviations from the plan. Be specific — this is the evidence Phase 1 is built on.

- [ ] **Step 3: Commit**

```bash
git add PHASE0-RESULTS.md && git commit -m "docs: Gate 1 technical verification results"
```

**Gate:** if any of items 1–6 FAIL, stop and resolve before Task 7. A failure of item 4 specifically (not actually Breeze-themed) undermines priority #1 and is grounds to reconsider the stack.

---

### Task 7: GATE 2 — build-ergonomics decision (keep Kirigami vs pivot to Slint)

The spec's one sanctioned pivot point. You live in the real loop for ~a day, then decide honestly.

**Files:**
- Modify: `PHASE0-RESULTS.md`

- [ ] **Step 1: Set up the two-loop workflow**

Add to your shell rc (`~/.zshrc`):
```bash
alias lyra-run='cmake --build "/home/andrew/Documents/Personal Projects/lyra/build" && QT_QPA_PLATFORM=wayland "/home/andrew/Documents/Personal Projects/lyra/build/lyra"'
```
Daily reality: **logic** work = `cargo test -p <crate>` / rust-analyzer (fast, no Qt). **UI** work = `lyra-run` (CMake incremental rebuild of only changed UI code, then launch). There is no clean `cargo run` for the window — that is expected and inherent to CXX-Qt 0.8.x.

- [ ] **Step 2: Use it for a day on real edits**

Make a few representative changes (edit `next_index`, add a QML element, add a bridge invokable) and feel the inner-loop friction. Note: how long is an incremental `lyra-run`? Does rust-analyzer work across the cxx-qt macro? Is the logic/UI split comfortable?

- [ ] **Step 3: Record the decision in `PHASE0-RESULTS.md`**

Write one of:
- **KEEP Kirigami** — the CMake-for-UI / cargo-for-logic split is tolerable for years. Proceed to write the Phase 1 plan against this stack.
- **PIVOT to Slint** — the build friction is not acceptable. The audio (`cpal`/`rodio`/`symphonia`), `lyra-core`, and the whole data/source architecture **carry over unchanged**; only the UI layer changes to a pure-Rust `cargo run` toolkit, accepting a non-native (non-Breeze) look. The Phase 1 plan is then written against Slint.

Capture the reasoning (incremental build times, IDE experience), not just the verdict.

- [ ] **Step 4: Commit**

```bash
git add PHASE0-RESULTS.md && git commit -m "docs: Gate 2 build-ergonomics decision"
```

- [ ] **Step 5: (Optional) Remove the spikes once both gates pass**

```bash
git rm -r spikes && \
  sed -i '/spikes\/rodio-play/d; /spikes\/cpal-pipewire/d' Cargo.toml && \
  cargo metadata --format-version 1 >/dev/null && echo "workspace still valid" && \
  git commit -am "chore: remove throwaway Phase 0 audio spikes"
```

---

## Phase 0 Exit Criteria

Phase 0 is complete when: the workspace builds, `lyra-core` tests pass with no Qt, both audio spikes run, `./build/lyra` shows a Breeze-themed Kirigami window driven by Rust (Gate 1 PASS), and `PHASE0-RESULTS.md` records the Gate 2 KEEP/PIVOT decision. The **next step is to write the Phase 1 plan** against whichever UI stack Gate 2 selected — that plan is authored separately, after this one is executed.
