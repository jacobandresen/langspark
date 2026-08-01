## Context

This is a new greenfield Rust application. The project has no existing codebase beyond OpenSpec configuration. See proposal.md for motivation. The application targets Linux desktop with GTK4/libadwaita for native UI.

Current state:
- No existing vocabulary or SRS system
- No Japanese dictionary integration
- No audio/TTS capability
- No pronunciation practice features

Constraints:
- Must use Rust programming language
- Must use GTK4 + libadwaita for UI (inspired by breadbin project structure)
- Must support offline usage after initial setup
- Must be open source compatible (no proprietary dependencies)

## Goals / Non-Goals

**Goals:**
- Create a maintainable Rust workspace with clear separation between core logic and UI
- Achieve native look and feel on Linux with GTK4/libadwaita
- Support offline-first usage with local dictionary data
- Provide responsive UI with smooth animations matching breadbin's kiosk experience
- Implement pronunciation practice with TTS and speech recognition
- Support multiple languages (Japanese and Spanish initially) with clean language switching
- Make language support extensible for future languages

**Non-Goals:**
- Cross-platform support beyond Linux (GTK4 provides this, but not a focus)
- Mobile or web versions
- Cloud synchronization or multi-user features
- Professional audio editing capabilities
- Real-time language switching during a study session

## Decisions

### Architecture: Rust Workspace with Core/GUI Separation

**Decision:** Use a Rust workspace with two crates: `langspark-core` (business logic, data models, SRS, audio, dictionary) and `langspark-gui` (GTK4 UI, widgets, user interaction).

**Rationale:** 
- Clear separation of concerns: core logic independent of UI framework
- Allows testing core logic without GTK dependencies
- Matches breadbin's architecture (breadbin-core + breadbin-gui)
- Enables potential future UI implementations (TUI, web) without rewriting business logic

**Alternatives considered:**
- Single crate: Simpler but mixes concerns, harder to test
- Three crates (core, audio, gui): Over-engineered for initial scope
- Shared library approach: Similar but workspace is more idiomatic Rust

### UI Framework: GTK4 + libadwaita

**Decision:** Use GTK4 with libadwaita (adw) for the UI, following breadbin's pattern.

**Rationale:**
- Native Linux look and feel
- Well-documented and maintained
- Breadbin provides excellent reference implementation
- libadwaita provides modern UI components (ViewStack, ViewSwitcher, etc.)
- Rust bindings are mature (gtk-rs project)

**Alternatives considered:**
- Egui: Not native, limited widgets, different aesthetic
- Druid: Less mature, fewer examples
- Tauri: Web UI, adds complexity of web + Rust bridge
- Relm: Separate project, less integration with GTK ecosystem

### Dictionary Source: JMdict-simplified + Kanjidic JSON

**Decision:** Use `scriptin/jmdict-simplified` GitHub repository as the primary dictionary source, providing pre-converted JSON files for JMdict, JMnedict, and Kanjidic.

**Rationale:**
- Already in JSON format, no conversion needed
- Comprehensive and well-maintained
- Freely available and open source
- Can be bundled with application or downloaded on first run

**Alternatives considered:**
- Direct EDICT files: Requires XML parsing and conversion
- KanjiAPI: API-based, requires network connectivity
- Jisho.org: API-based, rate limits, network required
- SQLite database: More complex to update, but better query performance

### Spaced Repetition: SM-2 as Primary, FSRS as Optional

**Decision:** Implement SM-2 algorithm as the primary SRS, with infrastructure to support FSRS as an optional alternative.

**Rationale:**
- SM-2 is simple, well-understood, and proven (Anki's default)
- Implementation is ~50 lines of Rust
- FSRS provides better personalization but is more complex
- Supporting both allows users to choose based on preference

**Alternatives considered:**
- FSRS only: Better but more complex, no fallback
- Anki compatibility: Would require matching Anki's exact implementation, limiting flexibility
- Custom algorithm: More work, less proven

**Implementation approach:**
- Create `SrsBackend` trait with `next_interval`, `update_card`, `score_response` methods
- Implement `SM2Backend` struct
- Implement `FSRSBackend` struct (using py-fsrs via FFI or Rust port)
- Default to SM-2, allow switching in preferences

### Text-to-Speech: VOICEVOX

**Decision:** Use VOICEVOX via `voicevox_core` Rust crate for TTS.

**Rationale:**
- Native Rust API
- High-quality neural voices specifically for Japanese
- Open source with active development
- Supports multiple speakers/voices
- Can generate audio on-demand or cache for offline use

**Alternatives considered:**
- Piper: Good multi-language support, but Japanese quality not as good
- OpenJTalk: Statistical, lower quality, requires HTS engine
- System TTS (espeak): Poor Japanese quality
- Pre-recorded audio: No flexibility, large storage, maintenance burden

### Speech Recognition: qwen3_asr_rs

**Decision:** Use `qwen3_asr_rs` pure Rust crate for speech recognition.

**Rationale:**
- Pure Rust implementation
- Supports Japanese
- Can run offline after model download
- Good accuracy for general speech recognition
- No external dependencies beyond the model files

**Alternatives considered:**
- speech-to-text-rust (Whisper): Also good, but larger model
- Vosk-api: C++ with Rust bindings, more complex setup
- Commercial APIs: Not offline, privacy concerns

### Audio Capture: CPAL

**Decision:** Use `cpal` crate for microphone audio capture.

**Rationale:**
- Low-level, cross-platform audio I/O
- Mature and widely used in Rust audio ecosystem
- Good documentation and examples
- Direct access to audio devices

**Alternatives considered:**
- libmic-rs: Higher-level, simpler API but less control
- Rodio: Primarily for playback, limited capture support
- Platform-specific APIs: Not cross-platform

### Pronunciation Scoring: Tiered Approach

**Decision:** Implement a tiered scoring system with three levels:
1. **Text Matching** (minimum): Compare recognized text to expected reading
2. **Phoneme-Level** (recommended): Break into morae, compare individually
3. **Acoustic Analysis** (advanced): DTW + MFCC comparison

**Rationale:**
- Allows incremental implementation
- Text matching is good enough for basic feedback
- Phoneme-level provides actionable feedback
- Acoustic analysis for advanced users but complex to implement
- Users can benefit from any level that's implemented

**Implementation phases:**
- Phase 1: Text matching with Levenshtein distance
- Phase 2: Morae-level segmentation and comparison
- Phase 3: DTW with MFCC features using `dtw_rs` crate

### Database: SQLite via rusqlite

**Decision:** Use SQLite with `rusqlite` crate for all persistent data (user vocabulary, SRS state, preferences).

**Rationale:**
- Single-file database, easy to manage
- No server required
- Mature Rust bindings
- Good performance for this use case
- Easy to backup and migrate

**Schema:**
- `vocabulary`: User-created entries (separate from dictionary)
- `kanji`: User-created kanji notes
- `cards`: SRS cards referencing vocabulary/kanji
- `decks`: User-created decks
- `reviews`: Review history for statistics
- `settings`: User preferences
- `languages`: Language configuration and installation status

**Alternatives considered:**
- JSON files: Simpler but no querying, less efficient for large data
- Sled: Embedded database, but less mature for complex queries
- Diesel + PostgreSQL: Overkill for single-user desktop app

### Multi-Language Architecture

**Decision:** Implement a language manager that coordinates all language-specific components. The system uses a pluggable architecture where language-specific features (dictionary, TTS, ASR) are loaded based on the active language.

**Rationale:**
- Clean separation between language-agnostic and language-specific code
- Easy to add new languages without modifying core logic
- Users can install only the languages they need
- Allows for language-specific optimizations

**Architecture:**
```
LanguageManager
├── Active Language State
├── Language Registry (available languages)
├── Dictionary Loader (language-specific)
├── TTS Manager (language-specific)
├── ASR Manager (language-specific)
└── Proficiency Level Mapper
```

**Language Abstraction:**
- Create `Language` enum or struct with language ID, display name, capabilities
- Create `LanguageTrait` for language-specific implementations
- Each language implements: dictionary loading, TTS, ASR configuration
- Core features (SRS, UI) work with trait objects, not concrete types

**TTS Strategy:**
- Use `voicevox_core` for Japanese
- Use `piper` for Spanish (and most other languages)
- Abstract both behind a `TtsBackend` trait
- Language manager selects appropriate backend based on active language

**ASR Strategy:**
- qwen3_asr_rs supports both Japanese and Spanish (30+ languages)
- Configure with language parameter when initializing
- No need for multiple ASR backends

**Database Schema Updates:**
- Add `language` field to all entries (vocabulary, kanji, cards, etc.)
- Add `languages` table for language metadata and installation status
- Filter all queries by active language by default
- Allow cross-language queries in advanced mode

**Alternatives considered:**
- Single TTS/ASR for all: Not possible, different quality per language
- Per-language binaries: More modular but complex distribution
- Runtime plugin loading: Overkill for this use case

### Workspace Structure: Inspired by breadbin

**Decision:** Follow breadbin's workspace and crate structure.

```
jv/
├── Cargo.toml                    # Workspace
├── crates/
│   ├── langspark-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs          # Public API
│   │       ├── model.rs         # Data structures
│   │       ├── dictionary.rs    # Dictionary loading
│   │       ├── srs.rs            # SRS algorithms
│   │       ├── audio.rs         # TTS, recording, recognition
│   │       ├── pronunciation.rs  # Pronunciation scoring
│   │       └── database.rs      # SQLite operations
│   │
│   └── jv-gui/
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs          # Entry point
│       │   ├── app.rs           # Main window, toolbar
│       │   ├── config.rs        # Settings
│       │   ├── vocabulary.rs     # Vocabulary tab
│       │   ├── kanji.rs          # Kanji tab
│       │   ├── review.rs         # Review tab
│       │   ├── pronunciation.rs   # Pronunciation tab
│       │   ├── stats.rs          # Statistics tab
│       │   ├── widgets/          # Reusable UI components
│       │   │   ├── card.rs       # Vocabulary/kanji card
│       │   │   ├── waveform.rs   # Audio waveform display
│       │   │   └── ...
│       │   └── task.rs          # Async task helpers
│       └── data/
│           └── style.css        # Custom theming
└── target/
```

**Rationale:**
- Proven structure from breadbin
- Clear separation of concerns
- Easy to navigate and extend
- Matches Rust idioms

## Risks / Trade-offs

### VOICEVOX + Piper Model Sizes
[Risk] VOICEVOX and Piper model files are large (~500MB-2GB each) → Provide option to use smaller models or only download selected voices. Allow pre-recorded audio as fallback. Allow installation of only one language's models at a time.

### Multiple Language Support Complexity
[Risk] Supporting multiple languages adds architectural complexity → Use trait-based abstraction to isolate language-specific code. Keep core logic language-agnostic. Start with Japanese and Spanish as proof of concept before adding more languages.

### Speech Recognition Accuracy
[Risk] qwen3_asr_rs accuracy for Japanese pronunciation may be lower than for English → Implement tiered scoring so basic text matching works even if recognition is imperfect. Provide clear feedback when recognition fails.

### Audio Latency
[Risk] Audio recording/playback latency could affect user experience → Use CPAL's low-latency configuration. Provide visual feedback (VU meter, waveform) during recording.

### GTK4 Learning Curve
[Risk] GTK4 + libadwaita has a learning curve for Rust developers → Follow breadbin patterns closely. Create reusable widget components. Document common UI patterns.

### Offline Model Download
[Risk] First run requires downloading large models (VOICEVOX, qwen3) → Provide progress indicators. Allow continuation without models (limited functionality). Cache models for future use.

### Memory Usage
[Risk] Loading full JMdict + Kanjidic + audio models could exceed memory limits → Implement lazy loading. Cache dictionary data efficiently. Provide memory usage monitoring.

### Platform Dependencies
[Risk] Audio dependencies (ALSA, PulseAudio) may not be present on all systems → Provide clear error messages with installation instructions. Check dependencies on startup.

## Migration Plan

### Initial Setup
1. User runs application for first time
2. Application checks for dictionary files in `~/.local/share/jv/`
3. If missing, prompt to download (or use bundled minimal set)
4. Check for VOICEVOX models, prompt to download
5. Check for qwen3 ASR model, prompt to download
6. Initialize SQLite database with default schema

### Data Locations
- Dictionary files: `~/.local/share/jv/dictionaries/`
- VOICEVOX models: `~/.local/share/jv/voicevox/`
- qwen3 models: `~/.local/share/jv/asr/`
- SQLite database: `~/.local/share/jv/jv.db`
- User audio cache: `~/.cache/jv/audio/`
- Application config: `~/.config/jv/config.toml`

### Rollback Strategy
- Dictionary files: Keep previous version, allow rollback via preferences
- Database: SQLite provides atomic transactions, but implement backup before schema migrations
- Models: Keep old models until new ones verified working

## Open Questions

1. **Model distribution:** Should VOICEVOX, Piper, and qwen3 models be bundled with the application, downloaded on first run, or optional add-ons?
   - Current plan: Download on first run with progress, allow skipping (reduced functionality). Download per-language models only when language is selected.

2. **Audio caching:** Should generated TTS audio be cached permanently, temporarily, or not at all?
   - Current plan: Cache indefinitely, provide cache cleanup in preferences. Cache per-language.

3. **Dictionary updates:** How often should dictionary updates be checked?
   - Current plan: Manual check via preferences, no automatic updates

4. **Pronunciation scoring level:** Which tier(s) to implement in v1?
   - Current plan: Tier 1 (text matching) for sure, Tier 2 (phoneme) if time permits

5. **Minimum Rust version:** What MSRV to target?
   - Current plan: Rust 2021 edition, latest stable supported by all dependencies

6. **Spanish dictionary source:** Which freely available Spanish-English dictionary to use?
   - Options: SpanDict JSON, Wiktionary dumps, or create our own from open sources
   - Current plan: Research and select best available option during implementation

7. **Piper Spanish model:** Which Piper model to use for Spanish?
   - Current plan: Use the official Piper Spanish model from the Piper repository
