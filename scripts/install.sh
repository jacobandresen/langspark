#!/usr/bin/env bash
# Build LangSpark in release mode and install it (binary, .desktop entry,
# AppStream metadata) under $PREFIX (defaults to /usr/local).
#
# Usage: PREFIX=/usr/local ./scripts/install.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
METAINFO_DIR="$PREFIX/share/metainfo"

echo "Building langspark-gui in release mode..."
cargo build --release -p langspark-gui

echo "Installing to $PREFIX ..."
install -Dm755 target/release/langspark-gui "$BIN_DIR/langspark-gui"
install -Dm644 langspark-gui/data/org.langspark.LangSpark.desktop "$APP_DIR/org.langspark.LangSpark.desktop"
install -Dm644 langspark-gui/data/org.langspark.LangSpark.metainfo.xml "$METAINFO_DIR/org.langspark.LangSpark.metainfo.xml"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" || true
fi

echo "Installed. Launch with: langspark-gui (or from your application menu)"
