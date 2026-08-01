## Why

Learning vocabulary effectively requires a combination of spaced repetition for memory retention and pronunciation practice for speaking confidence. Existing solutions are either web-based (not native), lack integrated pronunciation feedback, or are language-specific. This change creates a native, offline-first **multi-language** vocabulary application with built-in pronunciation practice using modern Rust technologies, supporting Japanese, Spanish, and other languages (one at a time).

## What Changes

- Build a new Rust application with GTK4/libadwaita UI (inspired by breadbin's kiosk layout)
- Integrate **language-specific dictionaries** (JMdict/Kanjidic for Japanese, SpanDict/other for Spanish)
- Implement SM-2 and FSRS spaced repetition algorithms for optimized review scheduling
- Add **language-aware** text-to-speech (VOICEVOX for Japanese, Piper for Spanish)
- Add **language-aware** speech recognition (qwen3_asr_rs supports both Japanese and Spanish)
- Create SQLite database for user progress, SRS state, and custom decks
- Implement language selection UI and persistence
- Implement audio recording/playback with CPAL for pronunciation practice

## Capabilities

### New Capabilities
- `language-management`: Language selection, switching, and configuration for Japanese, Spanish, and future languages
- `vocabulary-management`: CRUD operations for vocabulary entries, organizing by level, topic, frequency (language-agnostic)
- `kanji-lookup`: Kanji dictionary access with readings, meanings, stroke counts, examples (Japanese-specific)
- `spaced-repetition`: SRS scheduling using SM-2 algorithm with user-configurable parameters
- `pronunciation-practice`: TTS playback, audio recording, speech recognition, and pronunciation scoring (language-aware)
- `ui-kiosk`: GTK4/libadwaita tabbed interface with vocabulary browser, review queue, pronunciation practice, and statistics
- `dictionary-integration`: Loading and querying language-specific dictionary datasets

### Modified Capabilities

<!-- No existing capabilities to modify - this is a new application -->

## Impact

- New Rust workspace with `langspark-core` (business logic) and `langspark-gui` (GTK4 UI) crates
- New dependencies: voicevox_core, piper, cpal, qwen3_asr_rs, tokio, serde, rusqlite, gtk4, libadwaita
- New data directory for language-specific dictionary JSON files and SQLite database
- New audio resources for pronunciation (generated on-demand via language-specific TTS)
- Language configuration stored in database
