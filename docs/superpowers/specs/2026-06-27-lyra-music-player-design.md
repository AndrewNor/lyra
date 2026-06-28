# Lyra — Design Spec

- **Date:** 2026-06-27
- **Status:** Approved (brainstorming complete; ready for implementation planning)
- **Author:** Andrew (with Claude)

---

## 1. Summary

Lyra is a Rust-built, KDE-native music player for Linux. It exists because every
existing Linux player was rejected on three counts: ugly/dated UI, poor library
management, and missing/poor audio features. Lyra targets exactly those three:

1. **Beautiful, native-feeling UI** that fits a KDE Plasma desktop.
2. **Excellent local-library management.**
3. **Great audio quality and features.**

It ships first as an excellent **local-library player**, then grows through a clean
plug-in architecture into podcasts, internet radio, self-hosted servers
(Navidrome/Subsonic), and eventually YouTube/YouTube Music.

The guiding architecture is **"Rust brain, Qt face"**: 100% of the logic lives in
Rust; the UI is a thin QML/Kirigami skin bridged via CXX-Qt, so Lyra *is* a Plasma
app and inherits the user's real Breeze theme.

## 2. Goals & priorities (in order)

1. **Beautiful + native** — must inherit the user's Breeze color scheme, accent,
   fonts, and dark/light mode; must not read as dated or as a stretched phone app.
2. **Library management** — fast scan/index, strong search, robust metadata
   read/write, album-art handling.
3. **Audio + features** — gapless, ReplayGain, EQ, a bit-perfect path, broad format
   coverage, a proper play queue, lyrics.

## 3. Non-goals / constraints

- **Free-to-use sources only.** No paid streaming APIs or DRM (no Spotify/Tidal
  playback).
- **Linux-first.** KDE Plasma 6 / Wayland is the only initial target. Architecture
  stays portable so Windows/macOS remains *possible* later, but no cross-platform
  compromises are made now.
- **No full-screen "Now Playing" view** — the collapsible side panel is the
  now-playing experience.
- **No GStreamer backend built** — the user's library needs only pure-Rust decoding
  (see §7). GStreamer remains a *pre-designed contingency* behind a trait, not built.

## 4. Decisions log

| Decision | Choice | Rationale |
|---|---|---|
| Language / philosophy | Rust core + thin QML/Kirigami UI via **CXX-Qt 0.8.1** | User loves Rust; #1 priority is a native KDE look. Only a Qt/Kirigami app inherits Breeze theming natively. |
| UI layout | **Layout B**: sidebar · browse · **collapsible** right Now-Playing/queue panel · bottom transport | User chose it for art- and queue-forward listening; collapsible for narrow windows. |
| Immersive now-playing | **No** full-window view | Side panel is enough; keeps scope tight. |
| Bit-perfect output | **Must-have**, first-class | User wants a fully bypassable DSP chain + PipeWire passthrough; future-proofs for lossless. |
| Platform scope | **Linux-first, keep portable** | Build for KDE now; don't paint into a non-portable corner. |
| YouTube/YT-Music | **On roadmap, last & opt-in** | Matches "anything free to use," but isolated due to legal/breakage risk. |
| Name | **Lyra** | Short, musical (lyre/constellation), easy to theme an icon around. |
| Audio decode/output | Pure-Rust (`symphonia` + `cpal`/PipeWire) | Covers 100% of the current library; gives the hands-on control the user wants. |
| Database | SQLite (`rusqlite`) + FTS5 | Best-fit embedded DB + full-text search for a desktop library app. |

## 5. Verified environment (mid-2026, this machine)

- KDE Plasma 6 on **Wayland**; audio via **PipeWire 1.4.2** (PulseAudio compat present).
- Qt6 runtime **6.8.2**, Kirigami **6.13.0**, `libphonon4qt6` 4.12, `libKExiv2Qt6` present.
- Toolchains: **Rust 1.93**, Node 20, Python 3.13, gcc 14, CMake 3.31, Flatpak 1.16.
  **Missing:** `qmake6`/Qt6 -dev packages (only Qt5 on default qmake path), Kirigami
  -dev, `meson`, `org.kde.Platform` Flatpak runtime, `yt-dlp` — all are Phase-0 installs.
- **Library audited:** ~689 tracks, 5.9 GB, ~98% MP3 (667 mp3, 21 m4a, 1 flac), **zero
  exotic formats** → pure-Rust decoding fully covers it; scan performance is a non-issue
  at this size.

## 6. Architecture — "Rust brain, Qt face"

**Three-layer thread model:**

1. **Real-time audio thread** — the `cpal` callback only pulls f32 frames from a
   lock-free ring buffer (`rtrb`/`ringbuf`). Zero allocation, zero locking, no I/O;
   any violation causes PipeWire xruns.
2. **Rust core (Tokio multi-threaded runtime)** — owns library scan, DB access,
   metadata, ReplayGain, the decode/feed loop, and (later) all network sources.
   Talks to the audio thread only via the ring buffer + atomics/command channel.
3. **UI thread (Qt event loop)** — CXX-Qt marshals between Tokio and Qt: long work
   runs on Tokio and posts results back as Qt signals, so QML never blocks. State
   flows UI→core via invokables/commands, core→UI via signals. QML never touches the
   audio thread.

**Cargo workspace** of small, single-purpose crates (each independently testable):

```
lyra/
  crates/
    core/        # app state, command bus, playback orchestration
    engine/      # decode → DSP → resample → ring buffer → cpal output
    decoder/     # Decoder trait (default: symphonia; GStreamer contingency stubbed)
    dsp/         # ReplayGain apply, parametric EQ, resample, bit-perfect bypass
    db/          # rusqlite + FTS5, schema, queries
    library/     # filesystem scan, change detection, art cache
    metadata/    # read (symphonia) + write (lofty)
    sources/     # MusicSource trait + SourceRegistry
      local/     # first MusicSource (feature-flagged)
      podcast/ radio/ subsonic/ youtube/   # later phases, feature-flagged
    mpris/       # MPRIS2 D-Bus, notifications
    ui/          # CXX-Qt bridge + QML/Kirigami
```

**Portability:** `core`, `engine`, `db`, `library`, `metadata`, `dsp` stay
platform-agnostic; only output backends and packaging are platform-specific.

## 7. Audio engine

- **Decode:** `symphonia 0.6` (MP3/FLAC/AAC-LC/ALAC/Vorbis/PCM) + `symphonia-adapter-libopus`
  for Opus (accepts one C dep; `libopus` present). Covers 100% of the current library.
- **Output:** `cpal =0.18.1` with the `pipewire` feature (pinned; this is the first
  cpal release with that feature — verify it builds in Phase 0).
- **Gapless:** prototype/early-ship with `rodio 0.22.2` (which does achieve gapless);
  graduate to a direct `cpal` pipeline only for **sample-accurate** encoder
  delay/padding trimming at track boundaries.
- **DSP chain — fully bypassable (bit-perfect is a must-have):**
  - **ReplayGain 2.0** track+album gain via `ebur128`, computed during scan, applied
    as a linear gain stage.
  - **Parametric EQ** via `biquad` (or graph-based `fundsp`).
  - **Resampling** via `rubato`, only when device rate ≠ file rate.
  - **Bit-perfect toggle:** bypass EQ/RG/resample and configure the PipeWire node for
    passthrough/exclusive output. (Most meaningful once lossless files are added.)
- **Contingency:** a `Decoder` trait abstracts decode+analysis so a `gstreamer-rs`
  backend *could* drop in for exotic formats/turnkey ReplayGain — **not built**, since
  the library doesn't need it.

## 8. UI / UX

- **Layout B:** left nav sidebar · main browse area · **collapsible** right
  Now-Playing+queue panel · bottom transport bar.
- **Sidebar:** Albums / Songs / Artists / Genres / Recently Added → Playlists → future
  Sources (greyed "SOON" so the shape already fits the roadmap).
- **Right panel:** large current artwork, title/artist, reorderable "Up Next" queue;
  toggles open/closed.
- **Bottom bar:** artwork + meta, shuffle/prev/play/next/repeat, seek with times, volume.
- **Native theming:** follows Breeze color scheme, accent, fonts, dark/light live.
- **Design discipline:** treat Kirigami as theming + primitives; compose
  **desktop-grade** QtQuick layouts. Avoid `GlobalDrawer`/`PageRow`/page-stack chrome
  so it never reads like a stretched phone app.
- Approved mockups persist at `.superpowers/brainstorm/` (layout options + the
  fleshed-out Layout B screenshot).

## 9. Library & data

- **SQLite via `rusqlite` (bundled)** + **FTS5** full-text search. Schema models
  Track / Album / Artist plus source-id columns the integrations layer needs.
- **Scan:** background Tokio job — walk → mtime change-detection → parallel tag-parse
  → ReplayGain analysis → upsert → emit progress to UI. Incremental + parallel by
  design (correct even though trivial at ~700 tracks).
- **Metadata:** read via `symphonia`; **write/edit** via `lofty` (round-trip editing).
- **Album art:** extract embedded art, cache resized thumbnails on disk keyed by
  content hash.

## 10. Desktop integration

- **MPRIS2** via `mpris-server` (zbus) → Plasma media controls, lock screen,
  Now-Playing applet, and **media keys** all work natively.
- **Notifications** via freedesktop `org.freedesktop.Notifications` / KNotifications.
- **Packaging:** Flatpak against `org.kde.Platform` 6.x (primary channel); AppImage /
  native packages possible later.

## 11. Source plug-in architecture

- One async **`MusicSource` trait**: `search`, `browse`, `resolve_stream(track_id) ->
  StreamHandle`, plus capability flags. Every source maps into the shared
  Track/Album/Artist model and a `SourceRegistry` the UI talks to generically.
- **One playback path:** all sources resolve to an HTTP byte stream and decode through
  the same `Decoder` via `stream-download` (handles unbounded live streams).
- Each source is a **feature-flagged crate**, so churn/breakage in any pre-1.0 source
  crate can never reach the local-library player. Pin all pre-1.0 deps.

## 12. Phased roadmap

### Phase 0 — Spike & skeleton (1–2 weeks), two go/no-go gates
De-risk the hardest unknowns before committing.
- **Modules:** CXX-Qt 0.8.1 bridge skeleton (CMake-drives-Cargo); Kirigami shell with
  one ListView bound to a Rust QObject model; `rodio` quick-play spike; `cpal =0.18.1`
  pipewire-feature build check; cargo workspace layout.
- **Setup:** install Qt6 + Kirigami -dev packages; add `org.kde.Platform` Flatpak runtime.
- **GATE 1 (technical):** a Kirigami window shows a Rust-driven list themed by Breeze,
  and one file plays.
- **GATE 2 (ergonomics):** after a day of the CMake-drives-Cargo loop, confirm it's
  tolerable for years. If not, **flip to Slint** (pure-Rust UI) — same audio/library
  core, idiomatic `cargo run`, at the cost of native Breeze theming. This is the one
  sanctioned pivot.

### Phase 1 — Excellent local library player (the product)
A truly excellent local-first player.
- **Modules:** decoder-trait (symphonia default); library-scan; db (rusqlite + FTS5);
  metadata read (symphonia) + write (lofty); album-art cache; engine (rodio → direct
  cpal for sample-accurate gapless); dsp (ebur128 ReplayGain + biquad/fundsp EQ +
  rubato resample + **bit-perfect passthrough**); play queue (queue/shuffle/repeat);
  Layout-B Kirigami UI; MPRIS + media keys + notifications; lyrics (embedded + local
  `.lrc`, synced); `source_local`; Flatpak packaging.

### Phase 2 — Podcasts (lowest-risk source)
Validates the `MusicSource` trait + streaming path end-to-end.
- **Modules:** formalized `MusicSource` trait + `SourceRegistry`; `source_podcast`
  (`feed-rs`, OPML import); subscription DB + conditional-GET polling; download manager
  + resume-position; `stream-download` integration.

### Phase 3 — Self-hosted (Navidrome/Subsonic) + Internet radio
Two clean, fully-free, zero-legal-risk network sources.
- **Modules:** `source_subsonic` (`opensubsonic`; stream/cover URLs; auth UX);
  `source_radio` (vendored Radio Browser client — the published crate is stale;
  `icy-metadata` for StreamTitle); reconnect/buffer handling for live streams.

### Phase 4 — YouTube / YT-Music (opt-in, isolated, last)
The riskiest, highest-maintenance source — built last, behind a feature flag, designed
to fail gracefully.
- **Modules:** `source_youtube` (`ytmapi-rs` for search/browse — pinned, volatile;
  spawn `yt-dlp -J` for stream URLs); `yt-dlp` auto-download/auto-update as a
  first-class feature; PO-token provider sidecar; graceful-degradation UI; documented
  opt-in legal-risk consent.

### Phase 5 (optional) — Cross-platform & polish
- Windows/macOS cpal backends (WASAPI/CoreAudio); evaluate Kirigami off-Linux vs a
  second skin; optional Jellyfin adapter when Rust tooling matures.

## 13. Testing strategy

- **TDD on core logic** (fast Rust unit tests): queue/shuffle/repeat, scan diffing, DB
  queries, ReplayGain math, metadata parse/write round-trips, search ranking.
- **Integration tests** on the engine with fixture audio files: decode → frames,
  gapless boundary correctness, format coverage.
- **Manual/visual verification** for the QML UI and real audio output (these resist
  automated assertion).

## 14. Risks & mitigations

| Risk | Mitigation |
|---|---|
| **Build ergonomics** — CMake-drives-Cargo, no clean `cargo run` | Surfaced explicitly at Phase 0 Gate 2; Slint is the sanctioned escape hatch. |
| **CXX-Qt maturity** — younger, narrower community | Phase 0 Gate 1 spike; Slint fallback. |
| **Kirigami mobile-aesthetic trap** | Use Kirigami as theming + primitives; compose desktop-grade QtQuick layouts. |
| **Real-time audio discipline** — alloc/lock/IO in callback = xruns | Strict lock-free ring-buffer contract; no I/O on the audio thread. |
| **Sample-accurate gapless is hand-rolled** (if graduating off rodio) | Stay on rodio until needed; GStreamer `playbin3` path is the contingency. |
| **YouTube fragility & legal exposure** | Isolated, opt-in, last; auto-update yt-dlp; documented consent. |
| **Pre-1.0 source-crate churn** | Feature-flag + pin each source; vendor the Radio Browser client. |
| **Flatpak packaging complexity** (Qt6/KF6 + subprocess + Wayland sandbox) | Treat KDE runtime + permissions as explicit Phase-1 packaging work. |

## 15. Deferred / open items

- Smart playlists, crossfade, and an audio visualizer are **not** in Phase 1 (YAGNI;
  revisit after the core ships).
- Cross-platform UI viability (Kirigami off-Linux vs a second skin) is decided in
  Phase 5, not now.
- Exact DB schema and the `MusicSource`/`Decoder` trait signatures are finalized during
  implementation planning.

## 16. Tech stack summary

| Layer | Choice | Pin / note |
|---|---|---|
| UI | QML + Kirigami via CXX-Qt | CXX-Qt `0.8.1`, Qt6 `6.8.2`, Kirigami `6.13.0`, ECM 6.0+, CMake ≥3.28 |
| Async core | Tokio (multi-thread) | core runtime |
| Decode | `symphonia` 0.6 + `symphonia-adapter-libopus` | covers full library |
| Output | `cpal` `=0.18.1` (`pipewire` feature) | first release with the feature — verify in Phase 0 |
| Engine (early) | `rodio` 0.22.2 | gapless ok; graduate to direct cpal for delay/padding |
| Loudness | `ebur128` | ReplayGain 2.0 at scan |
| EQ / DSP | `biquad` / `fundsp` | parametric EQ |
| Resample | `rubato` | only when rates differ |
| Ring buffer | `rtrb` / `ringbuf` | RT-safe |
| DB | `rusqlite` (bundled) + FTS5 | + full-text search |
| Tags (read) | `symphonia-metadata` | on the player path |
| Tags (write) | `lofty` | round-trip editing |
| MPRIS | `mpris-server` (zbus) | Plasma media controls + media keys |
| Streaming | `stream-download` | one playback path for all sources |
| Packaging | Flatpak (`org.kde.Platform`) | primary channel |
