#!/usr/bin/env bash
# Build a self-contained Lyra AppImage.
#
# Expects a Linux host (or CI runner) with the build prerequisites installed
# (Qt 6, Kirigami, Rust, cmake, ninja). Bundles Qt + QML dependencies via
# linuxdeploy-plugin-qt. Run from anywhere; paths are resolved relative to the
# repo root.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$root"

builddir="build-appimage"
appdir="$root/AppDir"

# ── Build + stage install ─────────────────────────────────────────────────────
cmake -B "$builddir" -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build "$builddir"
rm -rf "$appdir"
DESTDIR="$appdir" cmake --install "$builddir" --prefix /usr

# ── Fetch linuxdeploy + Qt plugin ─────────────────────────────────────────────
tools="$root/.appimage-tools"
mkdir -p "$tools"
fetch() { [ -x "$tools/$1" ] || { wget -q -O "$tools/$1" "$2"; chmod +x "$tools/$1"; }; }
base="https://github.com/linuxdeploy"
fetch linuxdeploy          "$base/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
fetch linuxdeploy-plugin-qt "$base/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-x86_64.AppImage"

# CI runners usually lack FUSE; run the AppImage tools by extracting them.
export APPIMAGE_EXTRACT_AND_RUN=1
# Point the Qt plugin at our QML so it bundles the modules we import
# (QtQuick, Controls, Kirigami, …).
export QML_SOURCES_PATHS="$root/crates/ui/qml"
export OUTPUT="Lyra-x86_64.AppImage"

PATH="$tools:$PATH" linuxdeploy \
    --appdir "$appdir" \
    --plugin qt \
    -d "$appdir/usr/share/applications/ai.drivee.lyra.desktop" \
    -i "$appdir/usr/share/icons/hicolor/scalable/apps/ai.drivee.lyra.svg" \
    --output appimage

echo "Built: $root/$OUTPUT"
