# Lyra Phase 1C-mpris — MPRIS2 / media keys

> superpowers:subagent-driven-development. Verified over D-Bus (`qdbus6`/`busctl`) — no GUI clicking; full "media keys while playing" is the owner's gate.

**Goal:** Expose Lyra over **MPRIS2** D-Bus (`mpris-server` 0.10) so Plasma's media controls, the lock screen, and keyboard media keys (Play/Pause/Next/Previous/Stop) control playback, and the current track shows in Plasma's Now-Playing applet.

## Global Constraints
- **Threading:** the `Player` QObject + `Engine` live on the Qt thread (Engine is `!Send`). The MPRIS server runs on its OWN thread (zbus async executor — mpris-server 0.10 uses zbus; NO tokio needed). **Incoming** D-Bus controls marshal to the Qt thread via cxx-qt's `qt_thread()` → `CxxQtThread<Player>` (Send) → `.queue(|player| ...)` calling Player methods. **Outgoing** state changes: the Player (Qt thread) pushes updates to the MPRIS thread (a channel or a shared handle) which calls the server's property-changed emit.
- cxx-qt props snake_case in QML; invokables `#[cxx_name]`. UI crate `lyra_ui`. No `unwrap`/`expect` outside tests; never panic the UI or the D-Bus thread.
- Root `/home/andrew/Documents/Personal Projects/lyra` (branch `phase-1c-mpris`, quote it). Build `cmake --build build`.

---

### Task A: MPRIS server wired to Player

**Files:** `crates/ui/Cargo.toml` (+ `mpris-server = "0.10"`, and add it to root `[workspace.dependencies]`), `crates/ui/src/mpris.rs` (new), `crates/ui/src/player.rs`, `crates/ui/src/lib.rs`.

- [ ] **A1 — MPRIS module** (`mpris.rs`): implement `mpris-server` 0.10's `RootInterface` + `PlayerInterface` on a struct that holds a `CxxQtThread<Player>` (for incoming controls) and reads the latest playback state from a shared `Arc<Mutex<MprisState>>` (status, title, artist, album, art-url, length-µs, position-µs). Map:
  - `play_pause`/`play`/`pause`/`stop`/`next`/`previous` → `qt_thread.queue(|p| p.resume()/pause()/stop()/next()/prev())` (use play_pause→ if Playing pause else resume).
  - `playback_status()` → from shared state (Playing/Paused/Stopped).
  - `metadata()` → build `mpris_server::Metadata` from shared state (title/artist/album; `mpris:artUrl` = `file://`+cover_thumb if present; `mpris:length` from duration).
  - `position()` → shared state position µs. `can_go_next/can_go_previous/can_play/can_pause/can_control` → true; `can_seek` → false for now (engine seek exists but skip MPRIS SetPosition/Seek this phase, or wire if easy).
  - Root: `identity()`="Lyra", `can_quit`=false (or true→quit), `can_raise`=false, `supported_uri_schemes`/`mime_types`=empty.
  - Provide `pub fn start(qt: CxxQtThread<Player>) -> MprisHandle` that spawns a thread, runs the zbus server (block_on the server future on that thread), and returns a handle wrapping the `Arc<Mutex<MprisState>>` + a way to trigger `properties_changed`. (mpris-server 0.10: create `Server::new("Lyra", impl).await`, then `server.run().await`; to emit use the server's properties-changed API. Since the Server lives on the MPRIS thread, the handle pushes new state into the shared Mutex and signals the thread (channel/Notify) to call the emit; OR if mpris-server exposes a cloneable/Send emitter, store that. Correct against the real 0.10 API.)
- [ ] **A2 — Player wiring** (`player.rs`): the Player owns an `Option<MprisHandle>`. Add `#[cxx_name="initMpris"] fn init_mpris(self: Pin<&mut Self>)` — call once from QML `Component.onCompleted`; it does `mpris::start(self.qt_thread())` and stores the handle. (Requires `impl cxx_qt::Threading for Player` — add if not present.) After every state change (`play_from_list`/`next`/`prev`/`pause`/`resume`/`stop`/`refresh_position`), update the MPRIS handle's shared state (status, title, artist, album, cover, duration, position) and trigger properties_changed. Guard `None` handle (if MPRIS failed to start, the UI still works).
- [ ] **A3 — QML:** add `player.initMpris()` to `Component.onCompleted` in `Main.qml` (after the library load).
- [ ] **A4 — verify + commit.** `cargo build -p lyra_ui`; `cmake --build build`. Then VERIFY over D-Bus while the app runs (background-launch `./build/lyra`, then):
  - `busctl --user list | grep -i mpris` OR `qdbus6 | grep -i mpris` shows `org.mpris.MediaPlayer2.Lyra` (or similar).
  - `qdbus6 org.mpris.MediaPlayer2.Lyra /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player.PlaybackStatus` → "Stopped".
  - `qdbus6 ... org.mpris.MediaPlayer2.Identity` → "Lyra".
  - Calling `...Player.Next` / `.PlayPause` via qdbus6 returns without error (no crash; with nothing loaded these are safe no-ops).
  Capture this D-Bus output. Commit `feat(ui): MPRIS2 server (media keys / Plasma controls)`.

**If mpris-server 0.10 + the cxx-qt CxxQtThread integration proves too tangled to do cleanly (e.g. lifetime/Send issues with the server emitter across threads), STOP and report BLOCKED with the exact compiler/zbus errors** — do not ship a half-working or panicking D-Bus thread.

**API-correction clause:** mpris-server 0.10 trait shapes (`RootInterface`/`PlayerInterface`, `Metadata`, `Time`, `Property`, `Server::new`/`run`/emit) — verify against docs.rs/mpris-server/0.10 and the installed crate; correct freely, preserve the behavior. zbus's executor: use `async_io::block_on` or mpris-server's provided run helper on the dedicated thread.

## Exit Criteria
`org.mpris.MediaPlayer2.Lyra` appears on the session bus; PlaybackStatus/Identity/Metadata readable; control methods (PlayPause/Next/Previous/Stop) invoke the Player without crashing (D-Bus-verified). `lyra_ui` builds; offscreen-clean. Review → merge. **Owner gate:** play a track, then press the keyboard Play/Pause + Next media keys (or use Plasma's Now-Playing applet) and confirm they control Lyra. **Deferred:** MPRIS Seek/SetPosition, Raise/Quit.
