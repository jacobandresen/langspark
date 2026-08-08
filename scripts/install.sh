#!/usr/bin/env bash
# One-command full setup: system build dependencies, libtorch + the Qwen3-ASR
# model (speech recognition), a local VOICEVOX Engine (Japanese TTS, via
# Docker), the release binary itself, and the Japanese dictionary — installed
# under $PREFIX (defaults to /usr/local) as binary + .desktop entry +
# AppStream metadata.
#
# Usage: PREFIX=/usr/local ./scripts/install.sh
#   SKIP_DEPS=1       skip installing system packages (e.g. already installed,
#                     or you'd rather review install_dependencies() below and
#                     install them yourself)
#   SKIP_ASR=1        skip libtorch + the ASR model (scripts/setup-asr.sh) —
#                     the app still builds since ASR degrades to a clear
#                     "unavailable" error, but you'll need
#                     `--no-default-features` yourself if libtorch truly isn't
#                     available, since ASR is a default Cargo feature
#   SKIP_VOICEVOX=1   skip setting up VOICEVOX Engine (scripts/setup-voicevox.sh)
#   SKIP_DICTIONARY=1 skip downloading the Japanese dictionary
#
# Requires Rust/Cargo to already be installed (see https://rustup.rs) —
# distro package managers often ship an outdated Rust, so this script
# doesn't try to install it for you.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

PREFIX="${PREFIX:-/usr/local}"
BIN_DIR="$PREFIX/bin"
APP_DIR="$PREFIX/share/applications"
METAINFO_DIR="$PREFIX/share/metainfo"
ICON_DIR="$PREFIX/share/icons/hicolor/scalable/apps"

# System packages needed to build (and run) langspark-gui with the default
# feature set. Covers: GTK4 + libadwaita (UI), ALSA (cpal/rodio audio
# capture+playback), a C toolchain + cmake + pkg-config (needed to build
# qwen3-asr-rs's native tch/libtorch bindings), and Docker (VOICEVOX Engine,
# see setup-voicevox.sh). rusqlite is built with the "bundled" feature, so
# no system SQLite package is needed. libtorch itself (the ASR feature's
# native dependency) is handled separately by scripts/setup-asr.sh, below —
# it's project-local, not a system package.
install_dependencies() {
    if [[ "${SKIP_DEPS:-0}" == "1" ]]; then
        echo "SKIP_DEPS=1 set, not installing system packages."
        return
    fi

    if [[ "$(uname -s)" != "Linux" ]]; then
        echo "Not on Linux — skipping automatic dependency install."
        echo "See README.md's \"Platform Notes\" for macOS/Windows instructions."
        return
    fi

    local docker_pkg=()
    if [[ "${SKIP_VOICEVOX:-0}" != "1" ]]; then
        docker_pkg=(docker)
    fi

    echo "Installing system build dependencies..."
    if command -v pacman >/dev/null 2>&1; then
        sudo pacman -S --needed --noconfirm base-devel pkgconf cmake gtk4 libadwaita alsa-lib "${docker_pkg[@]}"
    elif command -v apt-get >/dev/null 2>&1; then
        sudo apt-get update
        [[ ${#docker_pkg[@]} -gt 0 ]] && docker_pkg=(docker.io)
        sudo apt-get install -y build-essential pkg-config cmake libgtk-4-dev libadwaita-1-dev libasound2-dev "${docker_pkg[@]}"
    elif command -v dnf >/dev/null 2>&1; then
        [[ ${#docker_pkg[@]} -gt 0 ]] && docker_pkg=(moby-engine)
        sudo dnf install -y gcc gcc-c++ make cmake pkgconf-pkg-config gtk4-devel libadwaita-devel alsa-lib-devel "${docker_pkg[@]}"
    elif command -v zypper >/dev/null 2>&1; then
        sudo zypper install -y -t pattern devel_basis
        sudo zypper install -y cmake pkgconf-pkg-config gtk4-devel libadwaita-devel alsa-devel "${docker_pkg[@]}"
    else
        echo "Unrecognized package manager — couldn't install dependencies automatically." >&2
        echo "Install manually: a C toolchain, cmake, pkg-config, GTK4 (4.10+) and" >&2
        echo "libadwaita (1.4+) development packages, and ALSA development headers" >&2
        echo "(plus Docker, unless you pass SKIP_VOICEVOX=1)." >&2
        echo "Then re-run with SKIP_DEPS=1." >&2
        exit 1
    fi

    if [[ ${#docker_pkg[@]} -gt 0 ]]; then
        if command -v systemctl >/dev/null 2>&1; then
            sudo systemctl enable --now docker || true
        fi
        if ! groups "$USER" | grep -qw docker; then
            sudo usermod -aG docker "$USER" || true
            echo "Added $USER to the docker group (setup-voicevox.sh below falls back to 'sudo docker'"
            echo "for this run — re-login, or run 'newgrp docker', to use plain 'docker' afterward)."
        fi
    fi
}

install_dependencies

LIBTORCH_DIR=""
if [[ "${SKIP_ASR:-0}" == "1" ]]; then
    echo "SKIP_ASR=1 set, not installing libtorch or the ASR model."
else
    ./scripts/setup-asr.sh
    LIBTORCH_DIR="$HOME/.local/share/langspark/libtorch-${LIBTORCH_VERSION:-2.7.0}"
fi

if [[ "${SKIP_VOICEVOX:-0}" == "1" ]]; then
    echo "SKIP_VOICEVOX=1 set, not setting up VOICEVOX Engine."
else
    ./scripts/setup-voicevox.sh
fi

CARGO_FEATURE_FLAGS=()
if [[ -n "$LIBTORCH_DIR" ]]; then
    # Bake libtorch's directory into the binary's RPATH so the *installed*
    # binary finds it at runtime without needing LD_LIBRARY_PATH set (unlike
    # a plain `cargo build` during development — see README.md).
    export LIBTORCH="$LIBTORCH_DIR"
    export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-Wl,-rpath,${LIBTORCH_DIR}/lib"
else
    CARGO_FEATURE_FLAGS=(--no-default-features)
fi

echo "Building langspark-gui in release mode..."
cargo build --release -p langspark-gui "${CARGO_FEATURE_FLAGS[@]}"

echo "Installing to $PREFIX ..."
install -Dm755 target/release/langspark-gui "$BIN_DIR/langspark-gui"
install -Dm644 langspark-gui/data/org.langspark.LangSpark.desktop "$APP_DIR/org.langspark.LangSpark.desktop"
install -Dm644 langspark-gui/data/org.langspark.LangSpark.metainfo.xml "$METAINFO_DIR/org.langspark.LangSpark.metainfo.xml"
install -Dm644 langspark-gui/data/icons/org.langspark.LangSpark.svg "$ICON_DIR/org.langspark.LangSpark.svg"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" || true
fi

if [[ "${SKIP_DICTIONARY:-0}" == "1" ]]; then
    echo "SKIP_DICTIONARY=1 set, not downloading the Japanese dictionary."
else
    echo "Downloading the Japanese dictionary..."
    cargo run -p langspark-core --example install_dictionary "${CARGO_FEATURE_FLAGS[@]}"
fi

echo "Installed. Launch with: langspark-gui (or from your application menu)"
