# Lyra Phase 1C-transport — real playback position + seek + polish

> **For agentic workers:** superpowers:subagent-driven-development. Engine bits verified by build + a play smoke that prints advancing position; QML by offscreen + screenshot.

**Goal:** Make the transport bar real: the engine reports live playback **position**, the UI shows elapsed/remaining time on a **progressing seek bar**, and (best-effort) the bar is **draggable to seek**. Plus two polish fixes: de-dupe the header track count, and "(untitled)" fallback for blank-tag rows.

## Global Constraints
- **RT-safe:** position tracking in the cpal callback may only be an atomic store/add (`AtomicU64`) — no alloc/lock/IO. Reset the counter on `play()`/`seek()`.
- **cxx-qt:** qproperties are snake_case in QML; invokables `#[cxx_name]`. UI crate is `lyra_ui`. No `unwrap`/`expect` in non-test code; never panic the UI.
- **Honesty:** if interactive seek (decode-thread re-seek + ring-buffer flush) proves too risky to do cleanly, SHIP the live position display (progressing bar + time labels) and make the bar non-draggable with a clear note — do NOT fake seeking. Don't animate a position that isn't real.
- **Build:** `cmake --build build`; UI crate `lyra_ui`. Root `/home/andrew/Documents/Personal Projects/lyra` (branch `phase-1c-transport`, quote the path).

---

### Task A: Engine position (+ best-effort seek) + Player wiring

**Files:** `crates/engine/src/{output.rs,decode_loop.rs,lib.rs}`; `crates/ui/src/player.rs`.

- [ ] **A1 — Engine position.** Add a shared `Arc<AtomicU64>` "frames played" counter; the cpal output callback adds the number of frames it writes (only count frames actually pulled from the ring buffer, not silence-fill — or count all output frames and document the choice; pick the one that tracks audible position best). Reset to 0 on `play()`. Add `pub fn position_secs(&self) -> f64` = `frames_played / device_sample_rate`. Add `pub fn device_sample_rate(&self) -> u32`.
- [ ] **A2 — Engine seek (best-effort).** Attempt `pub fn seek(&mut self, secs: f64) -> Result<()>`: signal the decode thread (a command/`Arc<Mutex<Option<f64>>>` or a channel) to `format.seek(SeekMode::Coarse, SeekTo::Time { time, track_id })`, drain/reset the ring buffer so stale audio doesn't play, and set the frame counter to `secs * rate`. If symphonia seek on the current decode-loop structure is too invasive to do safely now, leave `seek` returning an `Err`/no-op with a `// TODO` and note it — the UI will then present a read-only bar. Verify either way by the smoke test.
- [ ] **A3 — Player wiring** (`player.rs`): add qproperties `#[qproperty(f64, position_secs)]` and `#[qproperty(f64, duration_secs)]`. On `play_from_list`/`next`/`prev`, set `duration_secs` from the row's `durationMs/1000.0`. Add `#[cxx_name="refreshPosition"] fn refresh_position(self)` — reads `engine.position_secs()` into `position_secs` (called by a QML Timer). Add `#[cxx_name="seek"] fn seek(self, fraction: f64)` → `engine.seek(fraction * duration_secs)` (best-effort; ignore Err gracefully).
- [ ] **A4 — verify + commit.** `cargo build -p lyra-engine -p lyra_ui`; existing engine test green; smoke: `timeout 10 cargo run -p lyra-engine --example play -- <mp3>` extended to print `position_secs()` once a second — confirm it ADVANCES (~1.0, 2.0, …). Commit `feat(engine): live playback position (+best-effort seek)` and `feat(ui): Player position/duration + refreshPosition/seek`.

### Task B: QML — wire the seek bar + polish

**Files:** `crates/ui/qml/Main.qml`, `crates/ui/qml/TrackDelegate.qml`.

- [ ] **B1 — Header de-dupe:** show `library.track_count + " tracks"` once; show `library.status_text` only when it's a transient message (e.g. while `library.scanning` or right after a scan) — not a second "N tracks".
- [ ] **B2 — Untitled fallback:** in `TrackDelegate.qml`, title = `modelData.title && modelData.title.trim().length ? modelData.title : "(untitled)"`.
- [ ] **B3 — Live transport:** a `Timer { interval: 250; running: player.state_text === "Playing"; repeat: true; onTriggered: player.refreshPosition() }`. The seek `Slider`/progress: `value: player.duration_secs > 0 ? player.position_secs / player.duration_secs : 0`; left label = `fmt(player.position_secs)`, right = `fmt(player.duration_secs)` (`m:ss`). If engine seek works, make it a `Slider` with `onMoved: player.seek(value)`; if seek is a no-op (Task A2 deferred), make it a non-interactive `ProgressBar` with a tooltip "drag-to-seek coming soon". (Check the Task A report for whether seek landed.)
- [ ] **B4 — build + verify:** `cmake --build build`; `QT_QPA_PLATFORM=offscreen timeout 15 ./build/lyra 2>&1 | tee /tmp/lyra-tr.log` clean (no QML errors, ~679 tracks). Commit `feat(ui): live seek bar + position/time labels; header + untitled polish`.

**Visual/audio gate (owner):** play a track → the bar progresses and time labels count up; (if seek landed) dragging jumps playback. Screenshot for the record.

## Exit Criteria
Engine reports advancing position (smoke-verified); Player exposes position/duration; the transport bar progresses with correct time labels; header de-duped; untitled fallback in place; offscreen-clean. Seek is interactive if it landed cleanly, else a clearly-labeled read-only bar (deferred). Review → merge. **Next:** MPRIS (media keys), album-grid view, lyrics, Flatpak.
