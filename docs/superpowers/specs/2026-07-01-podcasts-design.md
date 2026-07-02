# Lyra — Podcasts feature design

**Date:** 2026-07-01
**Status:** approved design, pre-implementation

## Goal

Add a podcast experience to Lyra comparable to Google Podcasts / Castbox: search
podcasts online, subscribe to shows, stream or download episodes, resume where
you left off, get new-episode updates, and control playback speed without pitch
change.

## Decisions (locked)

- **Scope:** full — online search, subscribe, stream, downloads, background
  auto-refresh, in-app "New Episodes" inbox, pitch-preserving playback speed.
- **Directory:** Apple/iTunes Search API — free, **no API key**, returns the
  RSS feed URL. (Podcastindex/others can be added later behind the same trait.)
- **Remote playback:** HTTP-backed `MediaSource` with HTTP range requests
  (stream instantly, seek anywhere; downloads reuse the same fetch path).
- **Speed:** pitch-preserving WSOLA time-stretch in the DSP crate.
- **Updates:** refresh on launch + every ~30 min; new episodes appear in an
  in-app "New Episodes" inbox with a sidebar badge (no OS notifications).

## Constraints (project rules)

- No `unwrap`/`expect` in non-test code.
- The real-time audio callback stays allocation/lock/log-free. All network I/O
  runs on the decode thread or background worker threads.
- `~/Music` is scanned read-only and never written. Podcast data is written only
  to XDG data/cache dirs.
- CXX-Qt: `Q_PROPERTY`s are snake_case in QML; invokables use
  `#[cxx_name = "camelCase"]`; cross-thread updates use the CXX-Qt thread handle.

## Architecture

New crate **`crates/podcast`** (pure Rust, no Qt), four modules:

- **`directory.rs`** — Apple iTunes Search client.
  `search(term) -> Vec<PodcastResult>` (title, author, artwork URL, feed URL,
  itunes_id). Endpoint:
  `GET https://itunes.apple.com/search?media=podcast&term=<q>&limit=25`.
  `lookup?id=<collectionId>` fills details for a not-yet-subscribed show.
- **`feed.rs`** — fetch a show's feed with `ureq`, parse with **`feed-rs`**
  (RSS + Atom + iTunes namespace). Per episode: title, description/summary,
  audio enclosure URL + type, `itunes:duration`, `pubDate`, `guid`, image.
  Show-level: author, image, description.
- **`store.rs`** — all DB reads/writes for subscriptions, episodes, play state,
  and downloads, via the existing `lyra-db` connection + a new migration.
- **`http_source.rs`** — `HttpMediaSource` (implements Symphonia's
  `MediaSource: Read + Seek`) plus a `download(url, dest)` helper sharing the
  fetch path.

**HTTP client:** `ureq` (blocking) — fits the engine's `std::thread` model, no
async runtime, native `Range` header support.

**Threading:** search, feed fetch, downloads, and periodic refresh run on
background worker threads (cap ~4 concurrent feed fetches). Results marshal back
to the UI thread via the CXX-Qt thread handle, which then updates properties /
emits signals.

**Engine touch-point:** add
`Engine::play_source(Box<dyn MediaSource + Send>, duration_hint: Option<f64>)`
alongside `play(path)`; refactor `play(path)` to open the file and funnel into a
shared internal. Everything downstream (ring buffer, position, EQ, time-stretch,
MPRIS, auto-advance) is unchanged.

## Data model

One append-only migration in `crates/db/src/schema.rs`. Two new tables in the
existing `library.db`.

**`podcasts`**
- `id` INTEGER PK
- `feed_url` TEXT UNIQUE NOT NULL
- `itunes_id` INTEGER
- `title` TEXT NOT NULL
- `author` TEXT
- `description` TEXT
- `artwork_url` TEXT
- `subscribed` INTEGER NOT NULL DEFAULT 1
- `last_refreshed_at` INTEGER
- `created_at` INTEGER NOT NULL

**`podcast_episodes`**
- `id` INTEGER PK
- `podcast_id` INTEGER NOT NULL REFERENCES podcasts(id) ON DELETE CASCADE
- `guid` TEXT NOT NULL
- `title` TEXT NOT NULL
- `description` TEXT
- `audio_url` TEXT NOT NULL
- `artwork_url` TEXT
- `duration_secs` INTEGER
- `published_at` INTEGER
- `is_new` INTEGER NOT NULL DEFAULT 1
- `position_secs` REAL NOT NULL DEFAULT 0
- `played` INTEGER NOT NULL DEFAULT 0
- `last_played_at` INTEGER
- `download_path` TEXT
- `download_status` TEXT NOT NULL DEFAULT 'none'  -- none|queued|downloading|complete|failed
- `created_at` INTEGER NOT NULL
- UNIQUE(`podcast_id`, `guid`)

Indexes: `podcast_id`, `is_new`, `published_at`.

**On disk:**
- Downloads → `$XDG_DATA_HOME/lyra/podcasts/<podcast_id>/<episode_id>.<ext>`
  (default `~/.local/share/lyra/podcasts/`).
- Artwork cache → `$XDG_CACHE_HOME/lyra/podcast-art/`.

## Streaming playback (`HttpMediaSource`)

- On open, a ranged probe learns `Content-Length` and whether the server sends
  `Accept-Ranges: bytes`.
- **Read** = sequential streaming GET with a rolling buffer.
- **Seek** = new `Range: bytes=<pos>-` request. If the server refuses ranges,
  fall back to read-and-discard forward; a backward seek without range support
  re-opens from 0. `is_seekable()` reflects range support; `byte_len()` returns
  `Content-Length` when known.
- Sets a `User-Agent`, sane timeouts, follows redirects (tracking-prefixed
  enclosures are common).
- Runs on the decode thread only. Network stalls → ring underrun → brief silence
  → auto-recovers (already handled by the output path).

**Resolving an episode:** if `download_status == 'complete'` → play the local
file; else → `HttpMediaSource(audio_url)`. Either way the show's episode list
becomes the play queue, so auto-advance / next / prev work unchanged. Stream
duration comes from the feed's `itunes:duration`; position from `frames_played`
(adjusted for speed — see below).

**Downloads:** a worker thread streams the same URL to the destination path,
moving `download_status` queued → downloading → complete with progress reported
to the UI. Cancel/delete stops the thread and removes the file.

## Pitch-preserving speed (time-stretch)

Add a **WSOLA** time-stretch stage to `crates/dsp`.

- **Pipeline order:** `decode → EQ → resample to device rate → time-stretch →
  ring buffer`. Bit-perfect mode bypasses EQ and time-stretch.
- **Control:** a shared atomic `speed` (f32 bits) + generation counter, mirroring
  the existing EQ mechanism, so speed changes mid-episode take effect within a
  chunk. Default `1.0`; presets 1× / 1.25× / 1.5× / 1.75× / 2×. Applies to all
  playback; surfaced mainly for podcasts.
- **Position accounting:** the progress bar shows **source time**, not output
  time. Reported position =
  `last_decoded_source_ts − (frames_still_buffered × speed ÷ device_rate)`,
  using Symphonia's per-packet timestamps. This stays correct at any speed and
  when speed changes. Seeking is in source time (unaffected by speed), so
  `seekToSecs` is unchanged.
- **Persistence:** a single global playback-speed setting saved with the session
  (per-show speed memory is a future add).
- Built test-first. If a well-maintained WSOLA crate fits cleanly it will be
  evaluated; otherwise a compact in-house implementation.

## UI & QML bridge

**Sidebar** gains a **Podcasts** group: **Discover** (search), **Subscriptions**,
**New Episodes** (unread-count badge), **Downloads**.

**Views:**
- **Discover:** debounced (~400 ms) search field → results (artwork, title,
  author) → tap opens the Show page (even if not subscribed) with a Subscribe
  button.
- **Show page:** header (artwork, title, author, description, Subscribe/
  Unsubscribe) + episode list. Episode row: title, date, duration, played/
  position indicator, actions — Play, Download/Cancel/Delete, Mark
  played/unplayed.
- **New Episodes:** newest-first list across subscriptions; playing or marking
  clears `is_new` and decrements the badge.
- **Downloads:** downloaded / in-progress episodes; play offline or delete.

**Transport & now-playing (reused):** a speed button cycling 1×–2×. When a
podcast plays, the now-playing panel's second tab shows episode notes in place of
Lyrics; artwork/title/show name fill the usual slots. Auto-advance, next/prev,
seek, MPRIS unchanged.

**Two QObjects, clean split:**
- **`Podcast`** (new) — data & network only. Properties (snake_case):
  `search_results`, `searching`, `current_show_json`, `subscriptions_json`,
  `new_episodes_json`, `downloads_json`, `new_count`. Invokables (camelCase):
  `search`, `openShow`, `subscribe`, `unsubscribe`, `refreshAll`, `refreshShow`,
  `downloadEpisode`, `cancelDownload`, `deleteDownload`, `markPlayed`. A
  `downloadProgress(episodeId, pct)` signal drives download UI.
- **`Player`** (existing) — audio. Gains
  `playPodcast(audioUrl, localPath, title, show, artwork, queueJson)` (builds an
  `HttpMediaSource` or file source, sets the show as the play queue) and
  `setSpeed(factor)` + a `playback_speed` property.

**QML orchestrates:** `Podcast` supplies metadata/JSON; QML hands an episode to
`Player.playPodcast(...)`. All audio ownership stays in `Player`; all network/DB
in `Podcast`; neither reaches into the other.

## Background refresh & New inbox

On launch (after session restore) and every ~30 min, `refreshAll` runs on worker
threads (≤4 concurrent feeds). Each feed is fetched, parsed, and upserted by
`(podcast_id, guid)`; a genuinely new guid gets `is_new = 1`. The sidebar badge
binds to `new_count = COUNT(is_new AND NOT played)`. Playing or marking an
episode clears its `is_new`. Per-feed errors are isolated (logged, skipped) and
never abort the batch. A manual Refresh button triggers the same path.

## Error handling

- No `unwrap`/`expect` in non-test code. Search/feed/download failures surface as
  friendly UI states (directory unreachable, per-show retry, download → `failed`
  with retry) and never panic, even on malformed feeds (missing fields default).
- Range-unsupported servers: `HttpMediaSource` falls back to forward-read /
  re-open; playback works, seeking best-effort.
- RT audio callback stays alloc/lock/log-free; all network I/O off the RT path.
- All writes to XDG data/cache dirs; `~/Music` never touched.

## Testing

- `crates/podcast` unit tests: directory JSON parsing (fixture), feed parsing
  against RSS fixtures (iTunes namespace, missing fields, redirect enclosures),
  `store` CRUD/upsert on in-memory SQLite, download-path building.
- `HttpMediaSource`: `Read`/`Seek` against a tiny in-test HTTP server — range
  requests, no-range fallback, `byte_len`.
- `dsp` time-stretch (test-first): output length ≈ input ÷ speed, no NaNs,
  continuity, and a sine-wave check that dominant pitch is unchanged.
- Engine `play_source`: integration with a mock in-memory source.
- UI: offscreen boot + property/JSON self-tests.
- New deps (`ureq`, `feed-rs`) verified to build in the release CI containers
  (Debian trixie, KDE Sdk, Flatpak).

## New dependencies

- `ureq` — blocking HTTP client (search, feeds, streaming source, downloads).
- `feed-rs` — RSS/Atom/iTunes feed parsing.
- `serde` / `serde_json` — already present (directory JSON + QML JSON payloads).

## Out of scope (future)

- Podcastindex or other directories behind the `directory` trait.
- Per-show playback speed memory.
- OS desktop notifications for new episodes.
- OPML import/export.
