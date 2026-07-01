#!/usr/bin/env bash
# One-step installer for Lyra.
#
#   curl -fsSL https://raw.githubusercontent.com/AndrewNor/lyra/master/install.sh | bash
#
# Downloads the latest AppImage (works on any modern distro — it bundles Qt +
# Kirigami), installs it to ~/.local/bin/lyra, and adds a menu entry + icon.
# No root, no package manager, no dependency headaches.
set -euo pipefail

REPO="AndrewNor/lyra"
BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_DIR="$DATA_DIR/applications"
ICON_DIR="$DATA_DIR/icons/hicolor/scalable/apps"

echo "→ Finding the latest Lyra release…"
url=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep -oE 'https://[^"]*\.AppImage' | head -n1)
if [ -z "${url:-}" ]; then
    echo "✗ Couldn't find an AppImage in the latest release of $REPO." >&2
    exit 1
fi

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"

echo "→ Downloading $(basename "$url")…"
curl -fL# "$url" -o "$BIN_DIR/lyra"
chmod +x "$BIN_DIR/lyra"

echo "→ Installing icon and menu entry…"
curl -fsSL "https://raw.githubusercontent.com/$REPO/master/packaging/ai.drivee.lyra.svg" \
     -o "$ICON_DIR/ai.drivee.lyra.svg" 2>/dev/null || true

cat > "$APP_DIR/ai.drivee.lyra.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Lyra
GenericName=Music Player
Comment=Play your local music library
Exec=$BIN_DIR/lyra
Icon=ai.drivee.lyra
Terminal=false
Categories=AudioVideo;Audio;Player;Qt;KDE;
Keywords=music;audio;player;
EOF

update-desktop-database "$APP_DIR" 2>/dev/null || true

echo
echo "✓ Lyra installed to $BIN_DIR/lyra"
echo "  Launch it from your app menu (search \"Lyra\"), or run:"
case ":$PATH:" in
    *":$BIN_DIR:"*) echo "      lyra" ;;
    *)              echo "      $BIN_DIR/lyra"
                    echo "  (tip: add $BIN_DIR to your PATH to just type 'lyra')" ;;
esac
echo
echo "  If it doesn't start, your system may lack FUSE — either run:"
echo "      $BIN_DIR/lyra --appimage-extract-and-run"
echo "  or install FUSE:  sudo apt install libfuse2t64   (Debian/Ubuntu)"
