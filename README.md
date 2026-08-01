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

- **`jv-core`**: The business logic engine handling SRS calculations, dictionary queries, audio processing, and language management
- **`jv-gui`**: The GTK4/libadwaita user interface providing the kiosk-style layout and responsive interactions

Data persistence uses SQLite for reliability, with language-specific assets (dictionary JSON files, TTS models) stored in a dedicated directory. Audio capture and playback leverage CPAL for cross-platform microphone support.

## Supported Languages

| Language | Dictionary | TTS Engine | Speech Recognition |
|----------|------------|------------|-------------------|
| Japanese | JMdict, Kanjidic | VOICEVOX | qwen3_asr_rs |
| Spanish | SpanDict | Piper | qwen3_asr_rs |

Additional languages can be added by configuring dictionary datasets and TTS/ASR models.

## Getting Started

### Prerequisites

- Rust 1.70+ with Cargo
- GTK4 and libadwaita development libraries
- SQLite
- Audio backend: ALSA/PulseAudio (Linux), Core Audio (macOS), WASAPI (Windows)

### Installation

1. Clone the repository
2. Install language data: `cargo run -- install-language --lang ja` or `--lang es`
3. Launch the application: `cargo run`

### Building

LangSpark builds on Linux, macOS, and Windows.

```bash
cargo build --release
```

The release binary will be available in `target/release/` (or `target\release\` on Windows).

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
- **Speech Recognition**: qwen3_asr_rs (supports Japanese and Spanish)
- **Database**: SQLite via rusqlite
- **Async Runtime**: Tokio
- **Serialization**: Serde

## Project Structure

```
LangSpark/
├── jv-core/           # Core logic: SRS, dictionaries, audio processing
├── jv-gui/           # GTK4 user interface
├── data/             # Language-specific assets and SQLite database
│   ├── ja/           # Japanese dictionaries and models
│   ├── es/           # Spanish dictionaries and models
│   └── langspark.db  # User data and SRS state
└── openspec/         # OpenSpec change documentation
```

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
