# Phase 0 — Results (Gates 1 & 2)

- **Branch:** `phase-0-spike`
- **Date:** 2026-06-28
- **Outcome:** Phase 0 complete. The Rust + CXX-Qt/Kirigami stack is proven end-to-end. Proceed to Phase 1 on this stack.

## Build evidence
- `cargo test -p lyra-core` → 5 passed, no Qt required.
- rodio spike plays an MP3; cpal/PipeWire spike enumerates the output at F32 / 48 kHz.
- `lyra_ui` CXX-Qt 0.8.1 bridge compiles; `cmake --build build` links `./build/lyra` (61 MB).
- Headless `QT_QPA_PLATFORM=offscreen` smoke test: QML scene loads with no import/type errors.

## Gate 1 — Technical verification: **PASS**
Verified by screenshotting the running `./build/lyra` on the live Plasma 6 / Wayland session:
- Window opens, titled "Lyra (Phase 0 spike)".
- Renders in **native Breeze Dark** with Kirigami chrome (titlebar, header, toolbar) — not the offscreen "Fusion" fallback. This satisfies priority #1 (native KDE look).
- Rust → QML data path works: the `greeting` property, the `nextIndex()` invokable (delegates to `lyra_core`, returns `(0+1)%3 = 1`), and the 3 seeded tracks all render from Rust.
- "Simulate scan" action is present and wired (background `qt_thread` → append). The live click was not scripted; tapping it to see the 0.5 s append is a trivial remaining manual check.
- Audio output proven by the Task 2 / Task 3 spikes.

(Screenshot retained in the session scratchpad: `lyra-window.png`.)

## Gate 2 — Build ergonomics: **KEEP Kirigami** (provisional; owner to confirm)
Measured inner-loop times on the dev box:

| Loop | Command | Time |
|---|---|---|
| Logic edit (most of the codebase) | `cargo test -p lyra-core` | ~1 s (no Qt, no CMake) |
| UI-seam edit | `cmake --build build` | ~25 s (cxx-qt regen of `lyra_ui` + relink of the 61 MB exe) |

`cargo run` does **not** run the windowed app under CXX-Qt 0.8.x; the canonical UI loop is `cmake --build build && ./build/lyra` (alias `lyra-run`).

**Decision: KEEP Kirigami.** Gate 1 confirmed the native-Breeze look that motivated choosing CXX-Qt over Slint in the first place; the ~25 s cost is confined to UI-seam edits, while the majority logic loop (audio, library, db, metadata, dsp, sources) stays ~1 s; and the build complexity, though real, is now solved and documented. **Slint remains the sanctioned escape hatch** (the audio/core/source architecture carries over unchanged) if the UI loop proves intolerable in sustained use — this is the owner's call to confirm after living with it. Potential UI-loop speedup later: install a fast linker (`mold`/`lld`) to cut the relink portion.

## As-built API corrections (vs the plan's example code)
The plan's CXX-Qt snippets were version-volatile guesses; the shipped code was corrected against the installed cxx-qt 0.8.1 and the pinned `v0.8.1` example:
- Crate renamed `lyra-ui` → **`lyra_ui`** (underscore) to work around a cxx-qt-build 0.8.1 env-var-name bug (`CXX_QT_EXPORT_CRATE_lyra_ui` vs `lyra-ui`) that broke linking.
- `build.rs` uses the builder API: `CxxQtBuilder::new_qml_module(QmlModule::new("ai.drivee.lyra").qml_files(["qml/Main.qml"])).file("src/bridge.rs")`.
- CMake uses the **two-macro** form: `cxx_qt_import_crate(... CRATES lyra_ui ...)` + `cxx_qt_import_qml_module(lyra_ui_qml_module URI "ai.drivee.lyra" SOURCE_CRATE lyra_ui)`, linking target `lyra_ui_qml_module`.
- Bridge details: bare `Pin` (not `core::pin::Pin`) in the macro; `QStringList::append` takes the value by move; `use cxx_qt::Threading;` is required for `qt_thread()`.

## Next
Author the **Phase 1 plan** (the local library player) against the Kirigami stack: the full Layout-B UI, parallel scanning, FTS5 search, tag read/write, album art, gapless, ReplayGain, EQ, bit-perfect, the play queue, MPRIS, and lyrics.
