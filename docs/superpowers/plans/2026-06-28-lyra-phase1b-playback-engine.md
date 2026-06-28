# Lyra Phase 1B — Playback Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]` checkboxes.

**Goal:** The pure-Rust playback core: a streaming decoder, a play-queue model, a DSP stage (ReplayGain loudness analysis, parametric EQ, resampling, bit-perfect bypass), and a cpal/PipeWire output engine that plays a file end-to-end with gapless track transitions.

**Architecture:** Three new crates + an extension to `lyra-core`. `lyra-decoder` (a `Decoder` trait + `SymphoniaDecoder` yielding interleaved f32 packets). `lyra-core::queue` (pure `PlayQueue` over track ids, building on the existing `next_index`). `lyra-dsp` (loudness/EQ/resample, all pure functions over sample buffers). `lyra-engine` (a real-time cpal output stream fed from a decode thread via an `rtrb` lock-free ring buffer, with play/pause/seek/stop and gapless preload). The first three are unit-tested; the engine is built + smoke-tested by playing a file (a human confirms audio quality at the Phase-1B gate).

**Tech Stack:** symphonia 0.6, cpal =0.18.1 (pipewire), rtrb 0.3, ebur128 0.1, rubato 3.0, biquad 0.6.

## Global Constraints

- **Pure Rust, no Qt.** None of these crates depend on `lyra-ui`/cxx-qt/Qt. `lyra-decoder`, `lyra-core`, `lyra-dsp` test with plain `cargo test -p <crate>` (no audio device needed). `lyra-engine` needs an audio device only to *run*, not to *build/test* its pure parts.
- **Version pins** (add to root `[workspace.dependencies]`): `symphonia = { version = "0.6", features = ["mp3","isomp4","aac","flac","vorbis","wav","pcm"] }`, `ebur128 = "0.1"`, `rubato = "3"`, `biquad = "0.6"`, `rtrb = "0.3"`. (`cpal`/`rodio` already pinned.)
- **Real-time discipline (lyra-engine):** the cpal output callback may ONLY pop frames from the `rtrb::Consumer` and write them out — **no allocation, no locking, no I/O, no logging** in the callback. All decoding/DSP happens on a separate decode thread that fills the `rtrb::Producer`.
- **Sample format:** the internal pipeline is **interleaved `f32`**. The decoder converts symphonia's native format to f32; the engine resamples (via `lyra-dsp`) only when the file rate ≠ device rate.
- **Error handling:** each crate has a `thiserror` `Error` + `Result<T>`; no `unwrap`/`expect` in library code (tests excepted; the engine callback uses non-panicking fallbacks — write silence on underrun).
- **API-correction clause:** symphonia 0.6 / cpal 0.18.1 / rtrb 0.3 / ebur128 / rubato 3 / biquad 0.6 call details may differ from the snippets here; correct against the installed crates (docs.rs for the pinned version), preserving the public signatures and the tests' asserted behavior. Report BLOCKED only if a capability is genuinely missing.
- **Project root:** `/home/andrew/Documents/Personal Projects/lyra` (git, branch `phase-1b-engine`). Quote the path (it has a space).

## File Structure

```
crates/
  core/src/queue.rs          # PlayQueue (NEW; lib.rs gains `pub mod queue;`)
  decoder/                   # lyra-decoder
    src/lib.rs               # Error, AudioSpec, Decoder trait, DecodedChunk
    src/symphonia_backend.rs # SymphoniaDecoder
  dsp/                       # lyra-dsp
    src/lib.rs               # re-exports, Error
    src/loudness.rs          # ebur128 ReplayGain
    src/eq.rs                # biquad parametric EQ
    src/resample.rs          # rubato wrapper
  engine/                    # lyra-engine
    src/lib.rs               # Engine, PlaybackState, commands
    src/output.rs            # cpal stream + rtrb consumer
    src/decode_loop.rs       # decode thread: decoder -> dsp -> rtrb producer
```

---

### Task 0: Add `lyra-decoder` / `lyra-dsp` / `lyra-engine` crates + pin deps

**Files:** root `Cargo.toml`; stubs for the three crates.

- [ ] **Step 1:** Add `"crates/decoder"`, `"crates/dsp"`, `"crates/engine"` to `[workspace] members`; append the symphonia/ebur128/rubato/biquad/rtrb pins (above) to `[workspace.dependencies]`.
- [ ] **Step 2:** Create stubs:
  - `crates/decoder/Cargo.toml` — deps `symphonia = { workspace = true }`, `thiserror = { workspace = true }`; dev-dep `tempfile`.
  - `crates/dsp/Cargo.toml` — deps `ebur128`, `rubato`, `biquad`, `thiserror` (all `workspace = true`).
  - `crates/engine/Cargo.toml` — deps `lyra-decoder { path=../decoder }`, `lyra-dsp { path=../dsp }`, `lyra-core { path=../core }`, `cpal = { workspace = true }`, `rtrb = { workspace = true }`, `thiserror = { workspace = true }`.
  - Each `src/lib.rs`: a one-line doc comment.
- [ ] **Step 3:** `cargo metadata >/dev/null && echo OK`; `cargo build -p lyra-decoder -p lyra-dsp -p lyra-engine`. Commit: `chore: add decoder/dsp/engine crates + pin audio deps`.

---

### Task 1: `lyra-core::queue` — `PlayQueue` (TDD)

**Files:** create `crates/core/src/queue.rs`; add `pub mod queue;` to `crates/core/src/lib.rs`.

**Interfaces:** Produces `pub struct PlayQueue { items: Vec<i64>, pos: Option<usize>, repeat: RepeatMode, shuffle: bool }` with: `new()`, `set_items(Vec<i64>)`, `current() -> Option<i64>`, `next() -> Option<i64>` (advances `pos` using the existing `next_index` semantics with `repeat`; honors `shuffle` by walking a shuffled order), `prev() -> Option<i64>`, `jump_to(usize)`, `set_repeat(RepeatMode)`, `set_shuffle(bool)`, `append(i64)`, `clear()`. Track ids are `lyra-db` row ids (`i64`).

- [ ] **Step 1: failing tests** (in `queue.rs`): empty queue → `current`/`next` are `None`; `set_items([10,20,30])` → `current()==Some(10)`; `next()` walks 10→20→30; with `RepeatMode::Off` past the end → `None`; with `RepeatMode::All` wraps to 10; `prev()` goes back; `jump_to(2)` → `current()==Some(30)`; `append(40)` then advance reaches 40. (Shuffle: with `set_shuffle(true)` on `[10,20,30]`, the *set* of ids visited over 3 `next()` calls equals `{10,20,30}` with no repeats — assert the multiset, not order, to keep it deterministic-free; do NOT use rng seeded by time — derive a shuffle from a fixed permutation or a passed-in seed so the test is reproducible.)
- [ ] **Step 2:** run `cargo test -p lyra-core` → fail.
- [ ] **Step 3: implement.** Reuse `crate::next_index` for the linear advance. For shuffle, maintain an explicit `order: Vec<usize>` permutation of indices; `set_shuffle(true)` builds it (use a simple deterministic shuffle from a seed parameter or a Fisher–Yates over a caller-suppliable seed — **no `Math.random`/time-based rng**, so tests are reproducible; expose `set_shuffle_seeded(bool, u64)` if needed and have `set_shuffle(bool)` call it with a fixed default seed). `next()`/`prev()`/`current()` map through `order` when shuffled.
- [ ] **Step 4:** `cargo test -p lyra-core` → all green (existing `next_index` tests + new queue tests).
- [ ] **Step 5:** commit `feat(core): PlayQueue with repeat/shuffle over track ids`.

---

### Task 2: `lyra-decoder` — `Decoder` trait + `SymphoniaDecoder` (TDD)

**Files:** `crates/decoder/src/lib.rs`, `crates/decoder/src/symphonia_backend.rs`; `crates/decoder/tests/decode.rs`.

**Interfaces:**
- `pub struct AudioSpec { pub sample_rate: u32, pub channels: u16 }`.
- `pub trait Decoder { fn spec(&self) -> AudioSpec; fn next_chunk(&mut self) -> Result<Option<Vec<f32>>>; }` — `next_chunk` returns interleaved f32 frames (a packet's worth), `Ok(None)` at end of stream.
- `pub struct SymphoniaDecoder { ... }`; `SymphoniaDecoder::open(path: &Path) -> Result<Self>`.
- `pub enum Error` (`#[from]` symphonia error + io).

- [ ] **Step 1: failing integration test** `crates/decoder/tests/decode.rs`: generate a known WAV (copy `write_min_wav`, but make it longer — e.g. 4410 stereo frames of a 440 Hz sine at 44100 Hz so there's real audio), open with `SymphoniaDecoder`, assert `spec() == {44100, 2}`, then drain `next_chunk()` summing frame counts and assert the total interleaved sample count ≈ `4410 * 2` (allow the decoder's framing; assert within a small tolerance and that it terminates with `Ok(None)`).
- [ ] **Step 2:** run → fail.
- [ ] **Step 3: implement** `SymphoniaDecoder` using symphonia 0.6: probe the format, select the default audio track, create a decoder, loop `format.next_packet()` → `decoder.decode(&packet)` → convert the `AudioBufferRef` to interleaved f32 (use a `SampleBuffer::<f32>` and `copy_interleaved_ref`). `spec()` from the track's `codec_params` (sample_rate, channels count). (Correct the exact symphonia 0.6 type/method names against the crate.)
- [ ] **Step 4:** run → green.
- [ ] **Step 5:** commit `feat(decoder): Decoder trait + symphonia f32 backend`.

---

### Task 3: `lyra-dsp` — ReplayGain loudness analysis (TDD)

**Files:** `crates/dsp/src/loudness.rs`; wire into `lib.rs`.

**Interfaces:** `pub fn analyze_lufs(samples: &[f32], sample_rate: u32, channels: u16) -> Result<f64>` (integrated loudness, LUFS, via ebur128); `pub fn replaygain_gain_db(lufs: f64) -> f32` (ReplayGain 2.0 target −18 LUFS → `-18.0 - lufs`); `pub fn db_to_linear(db: f32) -> f32`.

- [ ] **Step 1: failing test:** feed 1 second of digital silence (`vec![0.0f32; 44100*2]`, stereo) → `analyze_lufs` returns a very low value (≤ −70 LUFS or the ebur128 "silent" sentinel — assert it's ≤ −60 or is the documented negative-infinity case). Feed a 1 s −6 dBFS 1 kHz sine (generate it) → `analyze_lufs` is in a sane loud range (roughly −12..−3 LUFS — assert a wide band, this is a sanity check not a precision test). `replaygain_gain_db(-18.0)` ≈ `0.0`; `replaygain_gain_db(-8.0)` ≈ `-10.0`. `db_to_linear(0.0)==1.0`, `db_to_linear(-6.0)` ≈ `0.501` (±0.01).
- [ ] **Step 2:** run → fail.
- [ ] **Step 3: implement** with `ebur128::EbuR128::new(channels, sample_rate, Mode::I)`, `add_frames_f32(&interleaved)`, `loudness_global()`. Map per the formulas above. (Handle the silent/`-inf` case so the test's silence assertion holds.)
- [ ] **Step 4:** green. **Step 5:** commit `feat(dsp): EBU R128 loudness + ReplayGain gain`.

---

### Task 4: `lyra-dsp` — parametric EQ + resampler (TDD)

**Files:** `crates/dsp/src/eq.rs`, `crates/dsp/src/resample.rs`; wire into `lib.rs`.

**Interfaces:**
- `pub struct Equalizer { ... }`; `Equalizer::new(sample_rate: u32, bands: &[EqBand]) -> Result<Self>` where `pub struct EqBand { pub freq_hz: f32, pub gain_db: f32, pub q: f32 }`; `pub fn process(&mut self, interleaved: &mut [f32], channels: u16)` (applies each band's biquad per channel, in place).
- `pub fn resample(input: &[f32], channels: u16, from_rate: u32, to_rate: u32) -> Result<Vec<f32>>` (rubato; returns interleaved f32 at `to_rate`; if `from_rate==to_rate` returns the input unchanged).

- [ ] **Step 1: failing tests:**
  - EQ identity: an `Equalizer` with a single band at 0 dB gain leaves a signal essentially unchanged (process a ramp/sine, assert output ≈ input within a small epsilon — a 0 dB peaking filter is ~unity).
  - EQ boost changes energy: a +12 dB band at 1 kHz applied to a 1 kHz sine increases its RMS vs the input (assert output RMS > input RMS).
  - Resample length: resampling 44100→48000 of `N` stereo frames yields ≈ `N * 48000/44100` frames (±a small tolerance); `from==to` returns identical length.
- [ ] **Step 2:** run → fail.
- [ ] **Step 3: implement** EQ with `biquad::Coefficients::<f32>::from_params(Type::PeakingEQ(gain_db), fs.hz(), f0.hz(), q)` + `DirectForm2Transposed` per channel (one filter instance per band per channel; deinterleave per channel or stride). Resample with `rubato::SincFixedIn`/`FftFixedIn` (deinterleave → resample per channel → reinterleave). (Correct rubato 3.0 / biquad 0.6 exact APIs.)
- [ ] **Step 4:** green. **Step 5:** commit `feat(dsp): parametric biquad EQ + rubato resampler`.

---

### Task 5: `lyra-engine` — cpal output + decode loop, play a file (build + smoke gate)

**Files:** `crates/engine/src/output.rs`, `crates/engine/src/decode_loop.rs`, `crates/engine/src/lib.rs`.

**Interfaces:**
- `pub enum PlaybackState { Stopped, Playing, Paused }`.
- `pub struct Engine { ... }`; `Engine::new() -> Result<Self>` (opens the default PipeWire output device via cpal, sets up the rtrb ring buffer + output stream); `Engine::play(&mut self, path: &Path) -> Result<()>` (spawns/feeds the decode loop for `path`); `pause`, `resume`, `stop`; `state() -> PlaybackState`. (Seek + gapless-preload may be stubbed with TODOs in this task and completed in a follow-up; the gate is "a file plays cleanly".)
- Internals: a decode thread runs `SymphoniaDecoder` → optional resample to device rate (`lyra-dsp::resample`) → push f32 frames into `rtrb::Producer`; the cpal output callback pops from `rtrb::Consumer` into the output buffer, writing silence if the buffer underruns. No alloc/lock/IO in the callback.

- [ ] **Step 1:** Implement `output.rs` (build cpal output stream on the device's default f32 config, callback pops from the consumer), `decode_loop.rs` (decode→resample→produce, on its own thread), `lib.rs` (`Engine` wiring + state). There is no unit test for live audio; instead add a `#[test]` that constructs an `Engine` and asserts `state()==Stopped` initially (build-level sanity), guarded so it doesn't fail in CI without a device (skip gracefully if `Engine::new()` errors with "no device").
- [ ] **Step 2: build** `cargo build -p lyra-engine` → Finished.
- [ ] **Step 3: smoke-test playback** behind a tiny example binary `crates/engine/examples/play.rs` (`Engine::new()?; engine.play(argv[1])?; sleep; `): run `timeout 10 cargo run -p lyra-engine --example play -- "$(find ~/Music -iname '*.mp3' | head -1)"`. SUCCESS = audio is produced and no panic before the timeout (exit 124 is a PASS, like Phase 0). Capture stdout/stderr.
- [ ] **Step 4:** commit `feat(engine): cpal/PipeWire output + decode loop (plays a file)`.

---

## Phase 1B Exit Criteria

`cargo test -p lyra-core -p lyra-decoder -p lyra-dsp` is all-green (queue, decode, loudness, EQ, resample), `lyra-engine` builds, and `cargo run -p lyra-engine --example play -- <file>` produces audible audio. **Human gate:** confirm playback sounds correct (no glitches/dropouts) on the real PipeWire device. Then whole-branch review → merge to `master`. Deferred to a 1B.1 follow-up (tracked, not in this plan): sample-accurate gapless preload, seek, and wiring ReplayGain/EQ/bit-perfect toggles through the engine from settings. **Next:** Phase 1C (the Layout-B Kirigami UI) binds `PlayQueue` + `Engine` + `lyra-db` search/list into the real player.
