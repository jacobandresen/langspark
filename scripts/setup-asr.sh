#!/usr/bin/env bash
# Set up local Japanese speech recognition (see langspark-core/src/asr.rs
# `SpeechRecognizer`, backed by the `qwen3-asr-rs` crate via libtorch).
# Downloads a standalone libtorch C++ distribution (version-pinned to what
# `tch`/`qwen3-asr-rs` expects — a pip-installed `torch` is usually a newer
# version and gets rejected at build time), the Qwen3-ASR-0.6B model weights,
# and generates the tokenizer.json the model repo doesn't ship (needs a throwaway
# Python venv with `transformers`, removed afterward).
#
# Idempotent: safe to re-run — skips any step whose output already exists.
#
# Usage: ./scripts/setup-asr.sh
#   LIBTORCH_VERSION=2.7.0
#   ASR_MODEL=Qwen3-ASR-0.6B        # or Qwen3-ASR-1.7B (larger, sharded weights)
#   ASR_LANGUAGE=ja                 # subdirectory under the ASR model dir
#
# After this completes, build and run with:
#   export LIBTORCH="$HOME/.local/share/langspark/libtorch-<version>"
#   export LD_LIBRARY_PATH="$LIBTORCH/lib:$LD_LIBRARY_PATH"
#   cargo build -p langspark-gui --features asr
#   LD_LIBRARY_PATH="$LIBTORCH/lib:$LD_LIBRARY_PATH" ./target/debug/langspark-gui
set -euo pipefail

LIBTORCH_VERSION="${LIBTORCH_VERSION:-2.7.0}"
ASR_MODEL="${ASR_MODEL:-Qwen3-ASR-0.6B}"
ASR_LANGUAGE="${ASR_LANGUAGE:-ja}"

DATA_DIR="$HOME/.local/share/langspark"
LIBTORCH_DIR="$DATA_DIR/libtorch-${LIBTORCH_VERSION}"
MODEL_DIR="$DATA_DIR/asr/${ASR_LANGUAGE}"

for cmd in curl unzip python3; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Missing required command: $cmd" >&2
        exit 1
    fi
done

# --- 1. libtorch (standalone C++ distribution, not pip's torch) -----------
if [[ -f "$LIBTORCH_DIR/lib/libtorch.so" ]]; then
    echo "libtorch ${LIBTORCH_VERSION} already present at $LIBTORCH_DIR"
else
    echo "Downloading libtorch ${LIBTORCH_VERSION} (CPU)..."
    tmp_zip="$(mktemp --suffix=.zip)"
    trap 'rm -f "$tmp_zip"' EXIT
    curl -fSL -o "$tmp_zip" \
        "https://download.pytorch.org/libtorch/cpu/libtorch-cxx11-abi-shared-with-deps-${LIBTORCH_VERSION}%2Bcpu.zip"
    tmp_extract="$(mktemp -d)"
    unzip -q "$tmp_zip" -d "$tmp_extract"
    mkdir -p "$LIBTORCH_DIR"
    mv "$tmp_extract"/libtorch/* "$LIBTORCH_DIR"/
    rm -rf "$tmp_extract" "$tmp_zip"
    trap - EXIT
    echo "libtorch installed to $LIBTORCH_DIR"
fi

# --- 2. Qwen3-ASR model weights + config (from Hugging Face) --------------
mkdir -p "$MODEL_DIR"
BASE_URL="https://huggingface.co/Qwen/${ASR_MODEL}/resolve/main"
FILES=(config.json model.safetensors vocab.json merges.txt tokenizer_config.json generation_config.json preprocessor_config.json chat_template.json)

echo "Fetching ${ASR_MODEL} into $MODEL_DIR ..."
for f in "${FILES[@]}"; do
    if [[ -f "$MODEL_DIR/$f" ]]; then
        echo "  $f already present, skipping"
    else
        echo "  downloading $f..."
        curl -fSL -C - -o "$MODEL_DIR/$f" "$BASE_URL/$f"
    fi
done

# --- 3. tokenizer.json (not shipped by the model repo; generated from
#        vocab.json + merges.txt via a throwaway venv) --------------------
if [[ -f "$MODEL_DIR/tokenizer.json" ]]; then
    echo "tokenizer.json already present."
else
    echo "Generating tokenizer.json (needs a throwaway Python venv with 'transformers')..."
    venv_dir="$(mktemp -d)"
    python3 -m venv "$venv_dir"
    "$venv_dir/bin/pip" install --upgrade pip -q
    "$venv_dir/bin/pip" install transformers -q
    "$venv_dir/bin/python" -c "
from transformers import AutoTokenizer
tok = AutoTokenizer.from_pretrained('$MODEL_DIR', trust_remote_code=True)
tok.backend_tokenizer.save('$MODEL_DIR/tokenizer.json')
"
    rm -rf "$venv_dir"
    echo "tokenizer.json generated."
fi

echo
echo "ASR setup complete."
echo "Build and run with:"
echo "  export LIBTORCH=\"$LIBTORCH_DIR\""
echo "  export LD_LIBRARY_PATH=\"\$LIBTORCH/lib:\$LD_LIBRARY_PATH\""
echo "  cargo build -p langspark-gui --features asr"
echo "  LD_LIBRARY_PATH=\"\$LIBTORCH/lib:\$LD_LIBRARY_PATH\" ./target/debug/langspark-gui"
