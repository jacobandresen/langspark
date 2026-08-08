# LangSpark

A native, offline-first vocabulary application for mastering Japanese through spaced repetition and pronunciation practice. Built in Rust with GTK4, LangSpark combines the effectiveness of scientific memory techniques with real-time speech feedback.

## Overview

LangSpark is designed for language learners who want a focused, distraction-free environment to build their vocabulary. Unlike web-based solutions, LangSpark runs locally on your machine, ensuring your data stays private and responsive.

## Features

### Spaced Repetition System
Harness the proven SM-2 algorithm (or FSRS, selectable in Preferences) to optimize your review schedule. Cards appear precisely when you're most likely to forget them, maximizing retention with minimal effort.

### Pronunciation Practice
Speak with confidence. LangSpark integrates VOICEVOX (text-to-speech) and Qwen3-ASR (speech recognition) to provide real-time feedback on your pronunciation. Record yourself, compare against native pronunciation, and receive instant scoring with a per-character diff of what was and wasn't heard.

### Comprehensive Dictionary Integration
Explore JMdict and Kanjidic with detailed readings, meanings, and example sentences (supplemented by the Tatoeba corpus for words JMdict's own examples don't cover). Each entry is just a click away from being added to your study deck.

### Native GTK4 Interface
Inspired by clean, functional design principles, the LangSpark interface organizes your study into intuitive tabs: Vocabulary browser and Review queue, plus Pronunciation practice once a speech recognition model is installed (see "Speech recognition" below). Keyboard shortcuts make navigation effortless.

## Architecture

LangSpark is built as a Rust workspace with two main crates:

- **`langspark-core`**: The business logic engine handling SRS calculations, dictionary queries, audio processing, and language management
- **`langspark-gui`**: The GTK4/libadwaita user interface providing the kiosk-style layout and responsive interactions

Data persistence uses SQLite for reliability, with language-specific assets (dictionary JSON files, TTS models) stored in a dedicated directory. Audio capture and playback leverage CPAL for cross-platform microphone support.

## Supported Languages

Japanese only, for now.

| Language | Dictionary | TTS Engine | Speech Recognition |
|----------|------------|------------|-------------------|
| Japanese | JMdict, Kanjidic (`scriptin/jmdict-simplified` JSON format) | [VOICEVOX Engine](https://voicevox.hiroshiba.jp/) (spoken to over its local HTTP API) | qwen3_asr_rs |

**Speech recognition** (`qwen3_asr_rs`) is a *default* Cargo feature (`asr`), but its only backends are `tch` (needs a system-wide libtorch install) and `mlx` (Apple Silicon only) — so building it requires libtorch to actually be present. `./scripts/install.sh` sets this up automatically as part of a full install (see below); for day-to-day `cargo build`/`cargo run` during development, run `./scripts/setup-asr.sh` once, then export the `LIBTORCH`/`LD_LIBRARY_PATH` it prints in every shell you build or run from (libtorch is dynamically linked, so both build time *and* runtime need it). Without libtorch available, build with `--no-default-features` — `SpeechRecognizer::transcribe` then reports a clear "unavailable" error instead of failing to compile. libtorch is only needed to *build*, though — once a binary has the `asr` feature compiled in, the model itself (weights + tokenizer) can be fetched from Preferences → Study → Language Installation instead of re-running the script (see `langspark_core::install_asr_model`); the Pronunciation tab only appears once that model is actually present.

## Getting Started

### Prerequisites

- Rust (edition 2021) with Cargo
- GTK4 (4.10+) and libadwaita (1.4+) development libraries
- An audio backend (ALSA/PulseAudio on Linux, CPAL's default host elsewhere)
- libtorch, for speech recognition — a *default* Cargo feature (see "Speech recognition" above), so needed to build at all unless you pass `--no-default-features`
- To use pronunciation practice: a running [VOICEVOX Engine](https://voicevox.hiroshiba.jp/) for Japanese TTS — installable from within the app (Preferences → Study → Language Installation) on Linux x86_64/aarch64 or Windows x86_64, no Docker needed; macOS and other architectures need Docker (`./scripts/setup-voicevox.sh`)

`./scripts/install.sh` sets up everything above (except Rust/Cargo itself) automatically — see "Building a release binary" below. The rest of this section covers a manual dev-workflow setup instead.

### Running from source

```bash
./scripts/setup-asr.sh                                    # once: libtorch + the ASR model
export LIBTORCH="$HOME/.local/share/langspark/libtorch-2.7.0"
export LD_LIBRARY_PATH="$LIBTORCH/lib:$LD_LIBRARY_PATH"    # every shell you build/run from
cargo build --workspace          # builds langspark-core and langspark-gui
cargo run -p langspark-gui       # launches the app
```

Skip the `setup-asr.sh`/env var steps and pass `--no-default-features` to `cargo build`/`cargo run` instead if you don't want speech recognition (see "Speech recognition" above).

On first run, LangSpark creates its SQLite database at the XDG data directory
(`~/.local/share/langspark/langspark.db` on Linux) and looks for dictionary
JSON files under `~/.local/share/langspark/dictionaries/<code>.json` — a
missing dictionary shows as a dismissible toast rather than a hard failure
(the app itself still opens). Everything else the app needs at runtime — the
dictionary, supplemental example sentences, the VOICEVOX Engine (Linux
x86_64/aarch64), and the speech recognition model — installs from
Preferences → Study → Language Installation; Preferences → Data Sources
lists what each one is, where it comes from, and its license.
`cargo run -p langspark-core --example install_dictionary` downloads just
the dictionary from the command line instead, if you'd rather.

### Testing

```bash
cargo test --workspace       # needs LIBTORCH set — see "Running from source" — or add --no-default-features
```

`langspark-gui`'s tests construct real GTK widgets, which needs a display
connection. If none is available (headless CI), run under Xvfb:

```bash
xvfb-run -a cargo test --workspace
```

### Building a release binary

The one-command path — `./scripts/install.sh` — does everything: installs
system build dependencies (detecting `pacman`/`apt`/`dnf`/`zypper`, asking
for `sudo`), sets up libtorch + the ASR model (`setup-asr.sh`) and a local
VOICEVOX Engine via Docker (`setup-voicevox.sh`), builds the release binary
(baking libtorch's path into the binary itself via `RUSTFLAGS`/`rpath`, so
the *installed* binary doesn't need `LD_LIBRARY_PATH` set at launch — unlike
a dev-workflow `cargo build`), installs it system-wide (binary, `.desktop`
entry, AppStream metadata), and downloads the Japanese dictionary:

```bash
PREFIX=/usr/local ./scripts/install.sh
```

Each step can be skipped independently if you'd rather manage it yourself:
`SKIP_DEPS=1`, `SKIP_ASR=1`, `SKIP_VOICEVOX=1`, `SKIP_DICTIONARY=1`. With
`SKIP_ASR=1`, the release build itself falls back to `--no-default-features`
(no speech recognition) rather than failing on a missing libtorch.

To build the release binary yourself without the rest of `install.sh`:

```bash
cargo build --release -p langspark-gui   # needs LIBTORCH set — see "Running from source"
```

The optimized binary is at `target/release/langspark-gui`.

### Platform Notes

**Linux**: Install GTK4 and libadwaita development packages via your distribution's package manager.

**macOS**: Install GTK4 via Homebrew: `brew install gtk4 libadwaita`

**Windows**: Install GTK4 via MSYS2 or the official GTK Windows installer.

## Usage

### Adding Vocabulary
Browse words by JLPT level. Click any entry to view details, listen to pronunciation, and add it to your study deck.

### Review Sessions
Your daily review queue automatically populates with cards due for review. Rate each card with Again, Hard, Good, or Easy to update the SRS schedule. The system tracks your progress and adjusts intervals based on your performance.

### Pronunciation Practice
Select a word, click Play to hear the native pronunciation, then record yourself. The system transcribes your attempt, compares it to the expected pronunciation, and provides a score with visual feedback. Try again until you nail it.

## Technical Stack

- **Language**: Rust
- **UI Framework**: GTK4 with libadwaita
- **Audio**: CPAL for capture, language-specific TTS engines for synthesis
- **Speech Recognition**: qwen3_asr_rs (default `asr` feature, needs libtorch)
- **Database**: SQLite via rusqlite
- **Async Runtime**: Tokio
- **Serialization**: Serde

## Project Structure

```
LangSpark/
├── langspark-core/     # Core logic: SRS, dictionaries, audio, TTS/ASR, pronunciation scoring
│   ├── src/
│   └── examples/
│       └── install_dictionary.rs   # downloads the Japanese dictionary (used by install.sh)
├── langspark-gui/      # GTK4/libadwaita UI: tabs, dialogs, widgets, app state
│   ├── src/
│   └── data/           # style.css, .desktop file, AppStream metadata, app icon
└── scripts/
    ├── install.sh          # Full setup: deps, libtorch+ASR model, VOICEVOX, release build, install, dictionary
    ├── setup-asr.sh        # libtorch + Qwen3-ASR model (speech recognition), standalone
    └── setup-voicevox.sh   # Local VOICEVOX Engine via Docker (Japanese TTS), standalone
```

At runtime, LangSpark stores its data under the platform's standard XDG-style
directories (see `langspark-gui::config::AppDirs`):

- Database: `~/.local/share/langspark/langspark.db`
- Dictionaries: `~/.local/share/langspark/dictionaries/`
- Audio cache: `~/.cache/langspark/audio/`
- Config: `~/.config/langspark/config.toml`

## Contributing

See `ARCHITECTURE.md` for the module layout and design decisions.

## Contact

For questions, feedback, or contributions, please contact:

**Jacob Andresen**  
jacob.andresen@gmail.com

## License

LangSpark is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with this program. If not, see [https://www.gnu.org/licenses/](https://www.gnu.org/licenses/).
