# Lyra Phase 1C-ui — Layout-B GUI

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. This is a VISUAL phase — the backend bits are TDD'd; the QML is verified by a clean offscreen load + a screenshot, and refined with the owner.

**Goal:** Turn the functional-but-plain 1C-core window into the **Layout-B** design: a left library sidebar, a content list with album-art thumbnails, a **collapsible** right Now-Playing + queue panel, and a full bottom transport bar — all native Breeze. Wire the features that are ready (search, scan, play, queue/next/prev, now-playing cover, Up Next); show-but-defer the not-yet-built ones (real seek position, album-grid view) cleanly.

**Architecture:** A small backend extension to `lyra-library` (populate `cover_thumb` during scan via `ArtCache`) and `lyra-ui`'s `Player` (use `lyra_core::PlayQueue`: `playFromList`, `next`, `prev`, expose `current_cover_thumb` + `queue_json`), then a full rewrite of `crates/ui/qml/Main.qml` into Layout B. **Remember: cxx-qt exposes Q_PROPERTYs to QML in snake_case** (`current_cover_thumb`, not `currentCoverThumb`); invokables use `#[cxx_name]` for camelCase.

## Global Constraints

- **cxx-qt 0.8.1 / QML naming:** qproperties → snake_case in QML; invokables → `#[cxx_name="camelCase"]`. (This bit us once — see the project memory.) Reuse the verified patterns in `crates/ui/src/{library,player}.rs`.
- **Crate is `lyra_ui`** (underscore). Build the window via `cmake --build build` (the `cmake/fix-link-order.cmake` workaround may need re-running only after a fresh `cmake -S . -B build`). Recommend the owner `sudo apt install mold` to remove that workaround + speed the relink — not required to proceed.
- **No `unwrap`/`expect` in non-test Rust;** never panic the UI/QML.
- **Breeze-native, desktop-grade QtQuick layouts** — NOT mobile page-stack chrome. Follow the approved Layout-B mockup (sidebar · content · collapsible right Now-Playing/queue panel · bottom transport). The reference look is in `.superpowers/brainstorm/.../layout-b-detail.html` and the published design artifact.
- **Defer cleanly (don't fake):** real audio position/seek (engine doesn't report position yet — show a non-interactive progress placeholder, label it, no fake animation), and the album-grid-by-album view (start with a songs list + thumbnails; album grid is a follow-on). Volume slider may be visual-only for now if the engine has no volume API — wire it if `Engine` exposes one, else mark visual.
- **Project root:** `/home/andrew/Documents/Personal Projects/lyra` (git, branch `phase-1c-ui-layout`). Quote the path.

---

### Task A: Backend — album art in scan + Player queue/cover/next-prev (TDD where testable)

**Files:** `crates/library/src/lib.rs` (scan), `crates/cli/src/main.rs` + `crates/library/tests/scan.rs` (signature update), `crates/ui/src/player.rs`, possibly `crates/ui/src/paths.rs` (art-cache dir helper).

- [ ] **A1 — cover thumbnails in scan.** Change `scan` to `pub fn scan(root: &Path, db: &mut Db, art: &ArtCache) -> Result<ScanSummary>`. For each new/changed track: after reading tags, call `lyra_metadata::read_cover(path)`; if `Some(cover)`, `art.store(&cover.data)` → set `NewTrack.cover_thumb = Some(path_string)`; on any cover error, leave `cover_thumb=None` (don't fail the track). Update the `lyra-library` scan integration test to pass an `ArtCache` (a tempdir) and assert a track with an embedded cover gets a non-empty `cover_thumb` pointing at an existing file. Add a `paths::art_cache_dir()` helper in `lyra-ui` (e.g. `$XDG_CACHE_HOME/lyra/art` fallback `~/.cache/lyra/art`).
- [ ] **A2 — update callers.** `lyra-cli`'s `scan` command: construct an `ArtCache` (at `~/.cache/lyra/art`) and pass it. `lyra-ui` `Library::scan` background closure: same. Keep `cargo build` + tests green across the workspace.
- [ ] **A3 — Player queue + cover + Up Next.** Extend the `Player` QObject:
  - Hold a `lyra_core::PlayQueue` + a cached `Vec` of the current playlist rows (`{id,title,artist,path,cover_thumb}` parsed from JSON).
  - `#[cxx_name="playFromList"] fn play_from_list(self, results_json: QString, index: i32)` — parse the Library results JSON, set the queue ids, jump to `index`, and play that row (engine.play(path)); set `current_title/current_artist/current_cover_thumb` + `state_text`.
  - `#[cxx_name="next"]`/`prev` — advance the `PlayQueue`, resolve the id→row from the cached list, play it, update current fields.
  - New qproperties: `current_cover_thumb: QString`, `queue_json: QString` (the upcoming rows after the current position, for the Up Next list).
  - Keep `pause/resume/stop`. No panics on bad index/empty list.
  - (Position/seek stays deferred — do not add a fake position property.)
- [ ] **A4** — `cargo test` workspace green (scan-cover test + existing). `cargo build -p lyra_ui`. Commit per logical step (`feat(library): album-art thumbnails during scan`, `feat(ui): Player play-queue + now-playing cover + Up Next`).

### Task B: Layout-B QML

**Files:** rewrite `crates/ui/qml/Main.qml` (+ optional small QML components under `crates/ui/qml/`, registered in `build.rs` `qml_files`).

- [ ] **B1 — Build the Layout-B shell** (Kirigami.ApplicationWindow, Breeze): a top bar with the app name + a search field; a **left sidebar** (sections: Library → Songs/Albums/Artists/Genres/Recently Added; Playlists; greyed "Sources · soon" → Podcasts/Radio/YouTube — styled placeholders); a **main content** area = a polished track list, each row showing the **album-art thumbnail** (`model.cover_thumb` via `file://`, with a neutral fallback rectangle when empty), title, artist, duration (mm:ss from `durationMs`); a **collapsible right panel** (a toggle in the top bar) showing the large current cover (`player.current_cover_thumb`), `player.current_title`/`current_artist`, and an **Up Next** list from `JSON.parse(player.queue_json)`; a **bottom transport bar**: current art + title/artist, shuffle/prev/play-pause/next/repeat buttons, a non-interactive progress placeholder (labeled, since seek is deferred), and a volume control (wire if available, else visual). Clicking a track → `player.playFromList(library.results_json, index)`. Use snake_case for all qproperty reads.
- [ ] **B2 — Build + verify.** `cmake --build build` → `./build/lyra`. Headless: `QT_QPA_PLATFORM=offscreen timeout 15 ./build/lyra 2>&1 | tee /tmp/lyra-1cui.log` — NO QML errors, non-empty root, the `[lyra] load_all: N tracks` log present (N≈679). Fix any QML error (snake_case! `file://` paths! guard `JSON.parse`).
- [ ] **B3 — Commit** `feat(ui): Layout-B GUI (sidebar, art thumbnails, now-playing/queue panel, transport)`.

**Visual gate (owner):** run `QT_QPA_PLATFORM=wayland ./build/lyra` → it looks like Layout B in Breeze, the library shows with cover thumbnails, clicking plays and the right panel + transport reflect the track, the queue panel toggles. The agent captures a screenshot for the record; the owner judges the aesthetics and we iterate (spacing, colors via accent, density, album-grid view, real seek).

---

## Phase 1C-ui Exit Criteria

Workspace tests green (incl. the scan-cover test); `cmake --build build` succeeds; offscreen load clean; and a screenshot shows the Layout-B window with the real library + album-art thumbnails + a working now-playing/queue panel + transport. Then review → merge. **Follow-ons:** album-grid-by-album view, real audio position + interactive seek (engine work, ex-1B.1), 1C-mpris (media keys), 1C-lyrics, Phase 1D (Flatpak).
