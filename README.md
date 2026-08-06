# LangSpark

A native, offline-first vocabulary application for mastering languages through spaced repetition and pronunciation practice. Built in Rust with GTK4, LangSpark combines the effectiveness of scientific memory techniques with real-time speech feedback to help you learn Japanese, Spanish, and more.

## Overview

LangSpark is designed for language learners who want a focused, distraction-free environment to build their vocabulary. Unlike web-based solutions, LangSpark runs locally on your machine, ensuring your data stays private and responsive. Whether you're studying kanji, mastering Spanish verbs, or expanding your vocabulary in any supported language, LangSpark adapts to your learning style.

## Features

### Spaced Repetition System
Harness the proven SM-2 algorithm to optimize your review schedule. Cards appear precisely when you're most likely to forget them, maximizing retention with minimal effort. Track your progress with detailed statistics, daily streaks, and retention rates that show your improvement over time.

### Pronunciation Practice
Speak with confidence. LangSpark integrates language-specific text-to-speech engines and speech recognition to provide real-time feedback on your pronunciation. Record yourself, compare against native pronunciation, and receive instant scoring with actionable feedback.

### Comprehensive Dictionary Integration
Access rich dictionary data for each language. For Japanese, explore JMdict and Kanjidic with detailed readings, meanings, stroke counts, and example sentences. Spanish support includes comprehensive word definitions and usage examples. Each entry is just a click away from being added to your study deck.

### Language Switching
Seamlessly switch between installed languages. LangSpark supports Japanese and Spanish out of the box, with architecture designed to accommodate additional languages. Each language maintains its own dictionary data, TTS models, and ASR configurations, all accessible through a unified interface.

### Native GTK4 Interface
Inspired by clean, functional design principles, the LangSpark interface organizes your study into intuitive tabs: Vocabulary browser, Kanji lookup (Japanese), Review queue, Pronunciation practice, and Statistics dashboard. Keyboard shortcuts and smooth animations make navigation effortless.

## Architecture

LangSpark is built as a Rust workspace with two main crates:

- **`langspark-core`**: The business logic engine handling SRS calculations, dictionary queries, audio processing, and language management
- **`langspark-gui`**: The GTK4/libadwaita user interface providing the kiosk-style layout and responsive interactions

Data persistence uses SQLite for reliability, with language-specific assets (dictionary JSON files, TTS models) stored in a dedicated directory. Audio capture and playback leverage CPAL for cross-platform microphone support.

## Supported Languages

| Language | Dictionary | TTS Engine | Speech Recognition |
|----------|------------|------------|-------------------|
| Japanese | JMdict, Kanjidic (`scriptin/jmdict-simplified` JSON format) | [VOICEVOX Engine](https://voicevox.hiroshiba.jp/) (spoken to over its local HTTP API) | qwen3_asr_rs (optional, see below) |
| Spanish | Custom minimal JSON schema — no maintained JSON dictionary export exists for Spanish; see `langspark_core::dictionary::spanish` | Piper (`piper-rs`, offline ONNX model) | qwen3_asr_rs (optional, see below) |

Additional languages can be added by implementing a dictionary loader and wiring up TTS/ASR configuration; see `langspark_core::dictionary` and `langspark_core::tts`.

**Speech recognition** (`qwen3_asr_rs`) is behind the `asr` Cargo feature because its only backends are `tch` (needs a system-wide libtorch install) and `mlx` (Apple Silicon only) — the default build doesn't require either, and `SpeechRecognizer::transcribe` reports a clear "unavailable" error without the feature enabled. Build with `cargo build --features langspark-core/asr` once libtorch is installed to enable it.

## Getting Started

### Prerequisites

- Rust (edition 2021) with Cargo
- GTK4 (4.10+) and libadwaita (1.4+) development libraries
- An audio backend (ALSA/PulseAudio on Linux, CPAL's default host elsewhere)
- To use pronunciation practice: a running [VOICEVOX Engine](https://voicevox.hiroshiba.jp/) for Japanese TTS, and a downloaded Piper voice model (`.onnx` + `.onnx.json`) for Spanish TTS

### Running from source

```bash
cargo build --workspace          # builds langspark-core and langspark-gui
cargo run -p langspark-gui       # launches the app
```

On first run, LangSpark creates its SQLite database at the XDG data directory
(`~/.local/share/langspark/langspark.db` on Linux) and looks for dictionary
JSON files under `~/.local/share/langspark/dictionaries/<code>.json` — a
missing dictionary shows as a dismissible toast rather than a hard failure
(the app itself still opens).

### Testing

```bash
cargo test --workspace
```

`langspark-gui`'s tests construct real GTK widgets, which needs a display
connection. If none is available (headless CI), run under Xvfb:

```bash
xvfb-run -a cargo test --workspace
```

### Building a release binary

```bash
cargo build --release -p langspark-gui
```

The optimized binary is at `target/release/langspark-gui`. To install it
system-wide (binary, `.desktop` entry, AppStream metadata) on Linux:

```bash
PREFIX=/usr/local ./scripts/install.sh
```

### Platform Notes

**Linux**: Install GTK4 and libadwaita development packages via your distribution's package manager.

**macOS**: Install GTK4 via Homebrew: `brew install gtk4 libadwaita`

**Windows**: Install GTK4 via MSYS2 or the official GTK Windows installer.

## Usage

### Adding Vocabulary
Browse words by level, topic, or frequency. Click any entry to view details, listen to pronunciation, and add it to your study deck. Kanji entries include readings, meanings, stroke order, and radicals.

### Review Sessions
Your daily review queue automatically populates with cards due for review. Rate each card with Again, Hard, Good, or Easy to update the SRS schedule. The system tracks your progress and adjusts intervals based on your performance.

### Pronunciation Practice
Select a word, click Play to hear the native pronunciation, then record yourself. The system transcribes your attempt, compares it to the expected pronunciation, and provides a score with visual feedback. Try again until you nail it.

## Technical Stack

- **Language**: Rust
- **UI Framework**: GTK4 with libadwaita
- **Audio**: CPAL for capture, language-specific TTS engines for synthesis
- **Speech Recognition**: qwen3_asr_rs (optional `asr` feature; supports Japanese and Spanish among 30+ languages)
- **Database**: SQLite via rusqlite
- **Async Runtime**: Tokio
- **Serialization**: Serde

## Project Structure

```
LangSpark/
├── langspark-core/     # Core logic: SRS, dictionaries, audio, TTS/ASR, pronunciation scoring
│   └── src/
├── langspark-gui/      # GTK4/libadwaita UI: tabs, dialogs, widgets, app state
│   ├── src/
│   └── data/           # style.css, .desktop file, AppStream metadata
├── scripts/
│   └── install.sh      # Release build + system install
└── openspec/           # OpenSpec change documentation
```

At runtime, LangSpark stores its data under the platform's standard XDG-style
directories (see `langspark-gui::config::AppDirs`):

- Database: `~/.local/share/langspark/langspark.db`
- Dictionaries: `~/.local/share/langspark/dictionaries/`
- Audio cache: `~/.cache/langspark/audio/`
- Config: `~/.config/langspark/config.toml`

## Contributing

LangSpark is built with OpenSpec methodology. The complete specification is available in the `openspec/changes/langspark/` directory, including:

- `proposal.md`: The vision and capabilities
- `design.md`: Architecture and technical decisions
- `tasks.md`: Implementation roadmap
- `specs/`: Detailed specifications for each capability

## Contact

For questions, feedback, or contributions, please contact:

**Jacob Andresen**  
jacob.andresen@gmail.com

## License

LangSpark is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.

You should have received a copy of the GNU General Public License along with this program. If not, see [https://www.gnu.org/licenses/](https://www.gnu.org/licenses/).
