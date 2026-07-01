# Lyra

[![Latest release](https://img.shields.io/github/v/release/AndrewNor/lyra?sort=semver)](https://github.com/AndrewNor/lyra/releases/latest)
[![Release build](https://github.com/AndrewNor/lyra/actions/workflows/release.yml/badge.svg)](https://github.com/AndrewNor/lyra/actions/workflows/release.yml)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
[![Platform: Linux/KDE](https://img.shields.io/badge/platform-Linux%20%2F%20KDE%20Plasma-1d99f3.svg)](#requirements)

A beautiful, native music player for KDE Plasma — a fast Rust core wrapped in a
QML/Kirigami interface that borrows its palette from the album art you're
listening to.

Lyra scans your local music library, plays it gaplessly through PipeWire, and
integrates with the desktop (media keys, MPRIS) the way a first-class Plasma app
should.

> **Status:** actively developed. Linux/Wayland + Plasma is the primary target.

## Screenshots

<!-- Drop PNGs into docs/screenshots/ and uncomment:
![Lyra — now playing](docs/screenshots/now-playing.png)
![Lyra — library](docs/screenshots/library.png)
-->

_Screenshots coming soon._

---

## Features

- **Local library** — scans `~/Music`, organised by Songs, Albums, Artists,
  Genres and Recently Added.
- **Adaptive color** — the accent and background gradient are sampled from the
  current track's cover art (colorless tracks fall back to a calm slate).
- **Full transport** — play/pause, seek, next/previous, shuffle and repeat, with
  a shuffle-aware Up Next queue.
- **10-band equalizer** — biquad EQ that rebuilds on the decode thread, plus
  bit-perfect and crossfade options.
- **Desktop integration** — MPRIS so media keys and the Plasma media applet work.
- **Session restore** — reopens on the track you left off, paused where you were.
- **Lyrics** — shows synced/plain lyrics when available.

## Requirements

Lyra is built with Qt 6, KDE Kirigami and a Rust workspace driven by CMake.

| Dependency        | Minimum         |
| ----------------- | --------------- |
| CMake             | 3.28            |
| Qt                | 6.8             |
| KDE Kirigami      | (Qt 6 build)    |
| Rust (with cargo) | stable          |
| C++ compiler      | C++17           |
| PipeWire + ALSA   | dev headers     |

### Debian / Ubuntu

```bash
sudo apt install \
    build-essential cmake ninja-build git pkg-config \
    qt6-base-dev qt6-declarative-dev \
    qml6-module-org-kde-kirigami qml6-module-qtquick-controls \
    qml6-module-qtquick-layouts qml6-module-qtquick-window \
    qml6-module-qtqml-workerscript \
    libpipewire-0.3-dev libasound2-dev
# plus Rust: https://rustup.rs
```

### Arch / Fedora

- **Arch:** `cmake ninja qt6-base qt6-declarative kirigami rust pipewire alsa-lib`
- **Fedora:** `cmake ninja-build qt6-qtbase-devel qt6-qtdeclarative-devel kf6-kirigami2-devel pipewire-devel alsa-lib-devel rust cargo`

## Build from source

```bash
git clone https://github.com/AndrewNor/lyra.git
cd lyra
cmake -B build -G Ninja
cmake --build build
```

The first configure fetches [Corrosion](https://github.com/corrosion-rs/corrosion)
and [cxx-qt-cmake](https://github.com/KDAB/cxx-qt-cmake), so it needs network
access once.

### Run

```bash
./build/lyra
```

Put some music in `~/Music`, click **Scan**, and press play.

## Keyboard shortcuts

| Shortcut            | Action              |
| ------------------- | ------------------- |
| <kbd>Space</kbd>    | Play / Pause        |
| <kbd>Ctrl</kbd>+<kbd>→</kbd> / <kbd>←</kbd> | Next / Previous track |
| <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>→</kbd> / <kbd>←</kbd> | Seek ±10s |
| <kbd>Ctrl</kbd>+<kbd>↑</kbd> / <kbd>↓</kbd> | Volume up / down |
| <kbd>Ctrl</kbd>+<kbd>S</kbd> | Toggle shuffle |
| <kbd>Ctrl</kbd>+<kbd>R</kbd> | Cycle repeat        |
| <kbd>Ctrl</kbd>+<kbd>M</kbd> | Mute / unmute       |

Shortcuts pause while you're typing in a text field. System media keys
(Play/Pause, Next, Previous) work too, via MPRIS.

## Install (prebuilt)

Prebuilt packages are attached to each [release](https://github.com/AndrewNor/lyra/releases):

- **Flatpak** — `flatpak install lyra.flatpak`
- **AppImage** — `chmod +x Lyra-*.AppImage && ./Lyra-*.AppImage`
- **Debian/Ubuntu** — `sudo apt install ./lyra_*.deb`

## Architecture

Lyra is a Cargo workspace bridged to Qt via [CXX-Qt](https://github.com/KDAB/cxx-qt):

| Crate              | Responsibility                                        |
| ------------------ | ----------------------------------------------------- |
| `crates/core`      | Play queue, shuffle/repeat logic (pure Rust)          |
| `crates/engine`    | Audio output (cpal → PipeWire), ring buffer, position |
| `crates/decoder`   | Symphonia-backed decoding                             |
| `crates/dsp`       | Equalizer / biquad filters, ReplayGain (EBU R128)     |
| `crates/db`        | SQLite library storage                                |
| `crates/metadata`  | Tag reading (lofty)                                   |
| `crates/library`   | Filesystem scan + indexing                            |
| `crates/ui`        | CXX-Qt `QObject`s + QML/Kirigami frontend             |
| `crates/cli`       | Headless CLI                                          |

The C++ side is a thin shim (`crates/ui/cpp/main.cpp`) that starts the Qt event
loop and loads the Rust-registered QML module.

## License

[GPL-3.0](LICENSE) © Andrew Nor
