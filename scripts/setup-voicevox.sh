#!/usr/bin/env bash
# Set up a local VOICEVOX Engine for Japanese TTS (see langspark-core/src/tts.rs
# `VoicevoxTts`, which talks to it over HTTP at http://127.0.0.1:50021 by
# default). Requires Docker with a running daemon; this script does not
# install Docker itself since that's platform-specific.
#
# On Linux x86_64/aarch64, LangSpark can install a native build instead (no
# Docker) from Preferences -> Study -> Language Installation, or via
# `langspark_core::install_voicevox_engine` — prefer that unless you're on
# another platform/architecture or specifically want the engine in Docker.
#
# Idempotent: safe to re-run. Starts a stopped container, or reports that it's
# already running.
#
# Usage: ./scripts/setup-voicevox.sh
#   VOICEVOX_IMAGE=voicevox/voicevox_engine:cpu-latest   # or an nvidia-*/gpu tag
#   VOICEVOX_PORT=50021
#   VOICEVOX_CONTAINER=voicevox_engine
set -euo pipefail

IMAGE="${VOICEVOX_IMAGE:-voicevox/voicevox_engine:cpu-latest}"
PORT="${VOICEVOX_PORT:-50021}"
CONTAINER="${VOICEVOX_CONTAINER:-voicevox_engine}"

if ! command -v docker >/dev/null 2>&1; then
    echo "docker is not installed. Install it via your package manager, e.g.:" >&2
    echo "  sudo pacman -S docker && sudo systemctl enable --now docker" >&2
    echo "  (then add yourself to the docker group: sudo usermod -aG docker \$USER, then re-login)" >&2
    exit 1
fi

# Prefer plain `docker`, but fall back to `sudo docker`: if this script (or
# install.sh) just added the user to the docker group, that membership isn't
# active in the current shell session yet (needs a fresh login, or `newgrp
# docker`) even though the daemon itself is up.
DOCKER=(docker)
if ! docker info >/dev/null 2>&1; then
    if sudo docker info >/dev/null 2>&1; then
        echo "Note: using 'sudo docker' — you're not in the docker group in this shell session yet"
        echo "(re-login, or run 'newgrp docker', to use plain 'docker' from now on)."
        DOCKER=(sudo docker)
    else
        echo "Can't reach the Docker daemon. Is it running?" >&2
        echo "  sudo systemctl enable --now docker" >&2
        echo "  sudo usermod -aG docker \$USER   # then re-login, or run: newgrp docker" >&2
        exit 1
    fi
fi

if "${DOCKER[@]}" ps --filter "name=^${CONTAINER}\$" --filter "status=running" --format '{{.Names}}' | grep -q "^${CONTAINER}\$"; then
    echo "VOICEVOX Engine is already running as '${CONTAINER}'."
elif "${DOCKER[@]}" ps -a --filter "name=^${CONTAINER}\$" --format '{{.Names}}' | grep -q "^${CONTAINER}\$"; then
    echo "Starting existing '${CONTAINER}' container..."
    "${DOCKER[@]}" start "${CONTAINER}" >/dev/null
else
    echo "Pulling ${IMAGE}..."
    "${DOCKER[@]}" pull "${IMAGE}"
    echo "Starting '${CONTAINER}' on port ${PORT}..."
    "${DOCKER[@]}" run -d \
        --name "${CONTAINER}" \
        --restart unless-stopped \
        -p "${PORT}:50021" \
        "${IMAGE}" >/dev/null
fi

echo -n "Waiting for VOICEVOX Engine to respond on port ${PORT}"
for _ in $(seq 1 30); do
    if curl -fsS --max-time 1 "http://127.0.0.1:${PORT}/version" >/dev/null 2>&1; then
        echo
        echo "VOICEVOX Engine is up: $(curl -fsS "http://127.0.0.1:${PORT}/version")"
        echo "LangSpark's Pronunciation tab (Japanese) should now be able to synthesize speech."
        exit 0
    fi
    echo -n "."
    sleep 1
done

echo
echo "VOICEVOX Engine didn't respond after 30s. Check its logs with:" >&2
echo "  docker logs ${CONTAINER}" >&2
exit 1
