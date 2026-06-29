# Lyra Phase 1C-core — Functional UI (real library + playback in Kirigami)

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]` checkboxes.

**Goal:** Wire the real `lyra-db` / `lyra-library` / `lyra-engine` / `lyra-core` into the `lyra-ui` CXX-Qt crate as two QObjects (`Library`, `Player`), and a *functional* Kirigami window that **scans `~/Music`, lists/searches the real library, and plays a selected track through the engine** with pause/resume/stop. This is the data-and-playback skeleton of the GUI — correctness over beauty. The full Layout-B aesthetic build, MPRIS, and lyrics are explicit follow-on sub-phases (1C-ui, 1C-mpris, 1C-lyrics).

**Architecture:** Extend `lyra-ui` (the Phase-0 CXX-Qt 0.8.1 crate). Add a `Library` QObject (owns a `lyra_db::Db` at the XDG path; exposes search/scan/loadAll and a JSON results string QML parses into a ListView model — avoids a full QAbstractListModel for now) and a `Player` QObject (owns a `lyra_engine::Engine` + `lyra_core::PlayQueue`; play/pause/resume/stop). Both live on the Qt thread; `scan()` does its filesystem/db work on a background thread via CXX-Qt's `qt_thread()` and posts results back. QML (`Main.qml`) is rebuilt into a real-but-plain library browser.

**Tech Stack:** cxx-qt 0.8.1 + Kirigami (as Phase 0), + path deps on lyra-db/library/engine/core.

## Global Constraints

- **CXX-Qt 0.8.1 API is the Phase-0-verified one.** Reuse the patterns in the existing `crates/ui/src/bridge.rs` / `build.rs` / `CMakeLists.txt` (builder `new_qml_module`, two-macro CMake, crate `lyra_ui`, URI `ai.drivee.lyra`, bare `Pin`, `QStringList::append` by value, `use cxx_qt::Threading;`). Treat the committed Phase-0 files as the source of truth for the API. Correct any new call against the installed crate; report BLOCKED only if a capability is genuinely missing.
- **Threading:** the cpal `Engine` (held by `Player`) is NOT `Send` — it must only be created and used on the Qt thread (inside invokables). Do NOT move it across threads. `scan()` (on `Library`) must NOT share the Qt-thread `Db` connection with a background thread; instead the background closure opens its OWN `Db` connection to the same file, scans, and on completion uses `qt_thread().queue(...)` to re-run `load_all()` on the Qt-thread `Db` and flip `scanning=false`.
- **DB path:** `$XDG_DATA_HOME/lyra/library.db` (fallback `~/.local/share/lyra/library.db`); create parent dirs. (Same file the `lyra-cli` uses — they interoperate.)
- **No `unwrap`/`expect` in Rust library code** (the QObject impls return early / set an error status on failure; never panic the UI). The cpal callback rule from the engine still holds (engine internal).
- **JSON contract (Library→QML):** `resultsJson` is a JSON array of objects `{ "id": i64, "title": string, "artist": string, "album": string, "path": string, "durationMs": number }`. Build it with a tiny hand-rolled JSON string writer (escape `"` and `\`) or `serde_json` if added as a dep — keep it simple and correct.
- **Verification is partly visual.** The window's *correctness* is checked by a headless `QT_QPA_PLATFORM=offscreen` load (no QML errors) + a screenshot on the live session showing real track titles. Audio playback is confirmed by clicking a track (manually, by the human) — the build/smoke proves it wires up.
- **Project root:** `/home/andrew/Documents/Personal Projects/lyra` (git, branch `phase-1c-ui`). Quote the path.

## File Structure

```
crates/ui/
  Cargo.toml                 # + lyra-db, lyra-library, lyra-engine, lyra-core path deps (+ serde_json if used)
  build.rs                   # + register the new bridge files / qml
  src/
    bridge.rs                # (existing LibraryController spike — may keep or remove)
    library.rs               # NEW: Library QObject (db + scan/search/loadAll -> resultsJson)
    player.rs                # NEW: Player QObject (engine + queue + transport)
    paths.rs                 # NEW: xdg db path helper
    lib.rs                   # + pub mod library; pub mod player; pub mod paths;
  qml/
    Main.qml                 # REWRITTEN: functional library browser + transport
```

---

### Task 0: Extend `lyra-ui` dependencies + db-path helper

- [ ] **Step 1:** Add to `crates/ui/Cargo.toml` `[dependencies]`: `lyra-db = { path = "../db" }`, `lyra-library = { path = "../library" }`, `lyra-engine = { path = "../engine" }` (lyra-core already reachable; add if not). Optionally `serde_json = "1"` (add `serde_json = "1"` to `[workspace.dependencies]` and inherit) for safe JSON building.
- [ ] **Step 2:** Create `crates/ui/src/paths.rs`: `pub fn library_db_path() -> std::path::PathBuf` returning `$XDG_DATA_HOME/lyra/library.db` (fallback `$HOME/.local/share/lyra/library.db`), creating the parent dir (ignore the create error if it already exists). Add a unit test that the returned path ends with `lyra/library.db`.
- [ ] **Step 3:** Wire `pub mod paths;` into `lib.rs`. `cargo build -p lyra-ui` (needs Qt env as before — build via cargo is fine for the staticlib). `cargo test -p lyra-ui` for the paths test. Commit `chore(ui): add db/library/engine deps + xdg db path helper`.

---

### Task 1: `Library` QObject — real db search/scan exposed to QML

**Files:** `crates/ui/src/library.rs`; `build.rs` (+ rust_files); `lib.rs`.

**Interfaces (QML-visible):**
- Properties: `resultsJson: QString`, `trackCount: i32`, `scanning: bool`, `statusText: QString`.
- Invokables: `loadAll()` (db.list_tracks → resultsJson + trackCount), `search(QString)` (db.search → resultsJson; empty query → loadAll), `scan()` (background scan of the music dir, then refresh).

- [ ] **Step 1:** Define a `Library` `#[qml_element]` QObject (backing struct holds `db: lyra_db::Db` and a `music_dir: PathBuf`). In `Default`/construction, open the db at `paths::library_db_path()` and call the equivalent of `load_all` to populate `resultsJson`. Implement a private `tracks_to_json(&[lyra_db::Track]) -> String` (escaping). `loadAll`/`search` set `resultsJson`+`trackCount` and emit the property-changed signals.
- [ ] **Step 2:** Implement `scan()`: set `scanning=true`, `statusText="Scanning…"`; grab `qt_thread()`; spawn a thread that opens a FRESH `lyra_db::Db` to the same path, runs `lyra_library::scan(&music_dir, &mut db)`, then `queue`s a closure onto the Qt thread that calls `load_all()` (on the Qt-thread db, which now sees the new rows), sets `scanning=false`, and `statusText` to a summary (e.g. "N tracks"). (Open the music dir as `$HOME/Music`.)
- [ ] **Step 3:** Register `src/library.rs` in `build.rs`'s `rust_files`, wire `pub mod library;` in `lib.rs`. `cargo build -p lyra-ui` → Finished. (No unit test for the QObject behavior beyond compile; the JSON escaper CAN have a unit test — add one asserting a title with `"`/`\` is escaped correctly.)
- [ ] **Step 4:** Commit `feat(ui): Library QObject (db search/scan -> JSON results)`.

**Correction guidance:** exposing two QObjects from one QML module — add `#[qml_element]` to each; both get registered. Holding a non-trivial Rust value (`Db`) in the backing struct is fine; `Default` can't easily open a db fallibly, so consider a `#[qinvokable] fn init()` called from QML `Component.onCompleted`, OR implement `Default` to open the db and fall back to `open_in_memory()` on error (set `statusText` to the error). Pick whichever compiles cleanly under cxx-qt 0.8.1.

---

### Task 2: `Player` QObject — engine transport

**Files:** `crates/ui/src/player.rs`; `build.rs`; `lib.rs`.

**Interfaces (QML-visible):**
- Properties: `stateText: QString` ("Stopped"/"Playing"/"Paused"), `currentTitle: QString`, `currentArtist: QString`.
- Invokables: `play(path: QString, title: QString, artist: QString)`, `pause()`, `resume()`, `stop()`.

- [ ] **Step 1:** Define a `Player` `#[qml_element]` QObject. Backing struct holds `engine: Option<lyra_engine::Engine>` (lazily `Engine::new()` on first `play`, since it opens a device; store the error in `stateText` if it fails) and the current track fields.
- [ ] **Step 2:** `play(path,title,artist)`: lazily init the engine; `engine.play(Path::new(&path.to_string()))`; on success set `currentTitle/currentArtist`, `stateText="Playing"`; on error set `stateText` to a short error. `pause`/`resume`/`stop` call the engine and update `stateText`. No panics.
- [ ] **Step 3:** Register in `build.rs` + `lib.rs`. `cargo build -p lyra-ui` → Finished. Commit `feat(ui): Player QObject (engine play/pause/resume/stop)`.

**Correction guidance:** `Engine` is `!Send`; keep it in the QObject (Qt thread). `Engine::play` takes `&mut self` so the backing struct field must be mutable via the `Pin<&mut Self>` receiver — store `engine: Option<Engine>` and `as_mut`/`get_mut` it. Convert `QString`→`String`→`Path` for the call.

---

### Task 3: Functional `Main.qml` + build the window (build + verify gate)

**Files:** `crates/ui/qml/Main.qml` (rewrite); the existing `cpp/main.cpp` + `CMakeLists.txt` already load `Main`.

- [ ] **Step 1:** Rewrite `Main.qml` (Kirigami.ApplicationWindow) to a functional browser:
  - Instantiate `Library { id: library }` and `Player { id: player }`.
  - A header `Kirigami.SearchField` (or a TextField) → `onAccepted: library.search(text)`; a "Scan" `Kirigami.Action` → `library.scan()` (disabled while `library.scanning`); show `library.statusText` + `library.trackCount`.
  - The main view: a `ListView` whose `model` is `JSON.parse(library.resultsJson)` (recompute on a `resultsJson`-changed binding); delegate = `Controls.ItemDelegate` showing `modelData.title` + `modelData.artist`; `onClicked: player.play(modelData.path, modelData.title, modelData.artist)`.
  - A bottom `footer`/`ToolBar`: `player.currentTitle — player.currentArtist` + `player.stateText` + Pause/Resume/Stop buttons wired to the invokables.
- [ ] **Step 2: build** via CMake: `cmake -S . -B build -G Ninja -DCMAKE_PREFIX_PATH="$(qmake6 -query QT_INSTALL_PREFIX)"` (reconfigure if needed) then `cmake --build build` → `./build/lyra`.
- [ ] **Step 3: headless verify:** `QT_QPA_PLATFORM=offscreen timeout 15 ./build/lyra 2>&1 | tee /tmp/lyra-1c.log` — must show NO `module/type is not installed`, no QML errors, non-empty root object (exit 124 = PASS). If the db is empty, the list is empty but the window must still load cleanly.
- [ ] **Step 4: commit** `feat(ui): functional library browser + transport (real db + engine)`.

**Human/visual gate (post-merge or pre-merge with the user):** run `QT_QPA_PLATFORM=wayland ./build/lyra` on the live session → click **Scan** (indexes ~/Music) → real track titles appear → click a track → **audio plays** → Pause/Resume/Stop work. A screenshot is captured for the record. (The agent verifies the headless load + build; the human confirms titles render and audio plays.)

---

## Phase 1C-core Exit Criteria

`cargo build -p lyra-ui` and `cmake --build build` succeed; `cargo test -p lyra-ui` passes (paths + JSON-escape unit tests); the offscreen load is clean; and on the live session the window scans, lists/searches the real library, and plays a clicked track with working pause/resume/stop. Then whole-branch review → merge. **Next sub-phases:** **1C-ui** (rebuild the QML into the beautiful Layout-B: sidebar nav, album grid, collapsible Now-Playing/queue panel, transport bar, album art via `ArtCache`, with the owner refining aesthetics), **1C-mpris** (`mpris-server` + media keys), **1C-lyrics** (.lrc + embedded). Sample-accurate gapless/seek (deferred from 1B.1) also wire in here.
