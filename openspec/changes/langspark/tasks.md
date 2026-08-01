## 1. Project Setup

- [ ] 1.1 Create Rust workspace with jv-core and jv-gui crates
- [ ] 1.2 Add basic Cargo.toml files for workspace and both crates
- [ ] 1.3 Add initial dependencies to jv-core (serde, serde_json, rusqlite, thiserror, anyhow, log)
- [ ] 1.4 Add initial dependencies to jv-gui (adw, gtk4, gdk4, glib, gio, tokio)
- [ ] 1.5 Add language management dependencies (strum, strum_macros)
- [ ] 1.6 Create directory structure for both crates
- [ ] 1.7 Set up basic logging infrastructure
- [ ] 1.8 Create .gitignore file

## 2. Language Management

- [ ] 2.1 Define Language enum (Japanese, Spanish) and implementations
- [ ] 2.2 Create Language trait for language-specific behavior
- [ ] 2.3 Implement LanguageRegistry with available languages and their metadata
- [ ] 2.4 Create LanguageManager to track active language and coordinate language-specific features
- [ ] 2.5 Implement language switching logic
- [ ] 2.6 Add language installation status tracking
- [ ] 2.7 Create language selection UI component

## 3. Core Data Model

- [ ] 3.1 Define VocabEntry struct with all required fields from vocabulary-management spec
- [ ] 3.2 Define KanjiEntry struct with all required fields from kanji-lookup spec
- [ ] 3.3 Define SrsCard struct for SRS tracking (from spaced-repetition spec)
- [ ] 3.4 Define PronunciationResult struct for scoring feedback
- [ ] 3.5 Add language field to all data structures
- [ ] 3.6 Implement Serialize/Deserialize for all data structures
- [ ] 3.7 Create type aliases and newtype wrappers for domain types (Word, Reading, etc.)

## 4. Dictionary Integration

- [ ] 4.1 Create language-agnostic Dictionary trait
- [ ] 4.2 Implement JMdict JSON loader from scriptin/jmdict-simplified format
- [ ] 4.3 Implement Kanjidic JSON loader
- [ ] 4.4 Research and select Spanish dictionary source
- [ ] 4.5 Implement Spanish dictionary JSON loader
- [ ] 4.6 Create DictionaryManager to hold loaded dictionaries (language-specific)
- [ ] 4.7 Implement search functionality (by word, reading, meaning)
- [ ] 4.8 Implement filter functionality (by proficiency level, part of speech, tags)
- [ ] 4.9 Add language-specific fuzzy matching (Japanese text vs Spanish text)
- [ ] 4.10 Implement caching for dictionary data
- [ ] 4.11 Add dictionary version checking and update mechanism

## 5. Database Layer

- [ ] 5.1 Create SQLite schema for all tables (vocabulary, kanji, cards, decks, reviews, settings, languages)
- [ ] 5.2 Add language field to all relevant tables
- [ ] 5.3 Implement database initialization and migration system
- [ ] 5.4 Create Repository trait for database operations
- [ ] 5.5 Implement SqliteRepository for vocabulary CRUD (with language filtering)
- [ ] 5.6 Implement SqliteRepository for kanji CRUD
- [ ] 5.7 Implement SqliteRepository for SRS card operations (with language filtering)
- [ ] 5.8 Implement SqliteRepository for deck operations
- [ ] 5.9 Implement SqliteRepository for review history
- [ ] 5.10 Implement SqliteRepository for language management
- [ ] 5.11 Add database backup/restore functionality

## 6. Spaced Repetition System

- [ ] 6.1 Define SrsBackend trait with required methods
- [ ] 6.2 Implement SM2Backend struct with SM-2 algorithm
- [ ] 6.3 Implement next_interval calculation
- [ ] 6.4 Implement ease factor adjustment based on ratings
- [ ] 6.5 Implement card state management (new, learning, review)
- [ ] 6.6 Create SrsManager to track all cards and scheduling (language-aware)
- [ ] 6.7 Implement daily review queue generation (filter by active language)
- [ ] 6.8 Add statistics tracking (streaks, retention rates)
- [ ] 6.9 Create deck management functionality

## 7. Audio Infrastructure

- [ ] 7.1 Create AudioManager struct to coordinate TTS, recording, recognition (language-aware)
- [ ] 7.2 Implement CPAL-based audio recorder
- [ ] 7.3 Implement audio playback using rodio or gstreamer
- [ ] 7.4 Create waveform data extraction for visualization
- [ ] 7.5 Add audio caching system for generated TTS (per-language)
- [ ] 7.6 Implement audio file format handling (WAV)

## 8. Text-to-Speech (Multi-Language)

- [ ] 8.1 Define TtsBackend trait for language-agnostic TTS
- [ ] 8.2 Add voicevox_core dependency to jv-core (Japanese)
- [ ] 8.3 Add piper dependency to jv-core (Spanish and others)
- [ ] 8.4 Create VoicevoxTTS wrapper struct for Japanese
- [ ] 8.5 Create PiperTTS wrapper struct for Spanish
- [ ] 8.6 Implement synthesize method for text-to-speech (trait-based)
- [ ] 8.7 Add language-specific voice selection configuration
- [ ] 8.8 Implement audio generation and caching (per-language)
- [ ] 8.9 Add error handling for missing models
- [ ] 8.10 Create fallback mechanism for offline use

## 9. Speech Recognition (Multi-Language)

- [ ] 9.1 Add qwen3_asr_rs dependency to jv-core
- [ ] 9.2 Create SpeechRecognizer wrapper struct
- [ ] 9.3 Implement transcribe method for audio-to-text (language-aware)
- [ ] 9.4 Add language parameter configuration for qwen3_asr_rs
- [ ] 9.5 Implement confidence score handling
- [ ] 9.6 Add error handling for recognition failures
- [ ] 9.7 Add language-specific text normalization (kana vs Latin script)

## 10. Pronunciation Scoring (Tier 1 - Text Matching)

- [ ] 10.1 Create PronunciationScorer struct (language-aware)
- [ ] 10.2 Implement language-specific text normalization (Japanese: remove spaces, consistent kana; Spanish: remove accents, consistent case)
- [ ] 10.3 Implement Levenshtein distance calculation
- [ ] 10.4 Implement simple score calculation based on text matching
- [ ] 10.5 Create language-specific feedback message generation
- [ ] 10.6 Add phoneme/morae segmentation for Japanese (morae-level)
- [ ] 10.7 Add phoneme segmentation for Spanish
- [ ] 10.8 Implement phoneme-level comparison (Tier 2)

## 11. UI Setup

- [ ] 11.1 Create main.rs with GTK application initialization
- [ ] 11.2 Implement load_styles function for custom CSS
- [ ] 11.3 Create build_ui function for main window construction
- [ ] 11.4 Set up header bar with view switcher and language indicator (inspired by breadbin)
- [ ] 11.5 Implement ToolbarView with ViewStack for tab navigation
- [ ] 11.6 Add application menu with Preferences, About, Quit
- [ ] 11.7 Create application actions and handlers

## 12. Vocabulary Tab UI

- [ ] 12.1 Create vocabulary tab module
- [ ] 12.2 Implement section header widget (like breadbin's section_header_widget)
- [ ] 12.3 Create Card widget for vocabulary display
- [ ] 12.4 Implement horizontal strip layout for vocabulary sections
- [ ] 12.5 Add "Show All" button with expand/collapse animation
- [ ] 12.6 Create grid view for expanded sections
- [ ] 12.7 Implement language-specific section grouping (JLPT for Japanese, CEFR for Spanish)
- [ ] 12.8 Add search box and filter controls

## 13. Kanji Tab UI (Language-Specific)

- [ ] 13.1 Create kanji tab module
- [ ] 13.2 Implement KanjiCard widget
- [ ] 13.3 Create horizontal strip layout for kanji sections
- [ ] 13.4 Add "Show All" functionality
- [ ] 13.5 Implement section grouping by JLPT level, grade, radical
- [ ] 13.6 Create kanji detail display
- [ ] 13.7 Add logic to show/hide tab based on active language (Japanese only)

## 14. Review Tab UI

- [ ] 14.1 Create review tab module
- [ ] 14.2 Implement card display for review (front and back)
- [ ] 14.3 Create rating buttons (Again, Hard, Good, Easy)
- [ ] 14.4 Add progress indicator (X of Y cards)
- [ ] 14.5 Implement card flip animation
- [ ] 14.6 Create daily review queue display (filtered by active language)
- [ ] 14.7 Add keyboard shortcuts for rating

## 15. Pronunciation Tab UI

- [ ] 15.1 Create pronunciation tab module
- [ ] 15.2 Implement word selection for practice (language-specific)
- [ ] 15.3 Create play button with language-specific TTS integration
- [ ] 15.4 Implement record/stop buttons with CPAL
- [ ] 15.5 Add waveform visualization widget
- [ ] 15.6 Create score display area
- [ ] 15.7 Implement feedback message display
- [ ] 15.8 Add navigation (next/previous word)

## 16. Statistics Tab UI

- [ ] 16.1 Create statistics tab module
- [ ] 16.2 Implement overall progress summary display
- [ ] 16.3 Create daily streak widget
- [ ] 16.4 Add review history chart (using a charting library or custom drawing)
- [ ] 16.5 Implement next reviews schedule display
- [ ] 16.6 Create per-deck statistics
- [ ] 16.7 Add achievement/badges display

## 17. Detail Dialogs

- [ ] 17.1 Create vocabulary detail dialog
- [ ] 17.2 Implement large word display with language-specific phonetic guide (furigana for Japanese, phonetic for Spanish)
- [ ] 17.3 Add meaning, part of speech, proficiency level display
- [ ] 17.4 Create example sentence display
- [ ] 17.5 Implement audio playback controls
- [ ] 17.6 Add pronunciation practice button
- [ ] 17.7 Create add to deck functionality
- [ ] 17.8 Implement edit/delete buttons
- [ ] 17.9 Create kanji detail dialog (Japanese only)
- [ ] 17.10 Add all readings, meanings, stroke info display

## 18. Waveform Widget

- [ ] 18.1 Create Waveform widget for audio visualization
- [ ] 18.2 Implement drawing of audio samples
- [ ] 18.3 Add color customization for reference vs user waveforms
- [ ] 18.4 Implement waveform comparison display
- [ ] 18.5 Add smooth scrolling for long waveforms

## 19. Preferences Dialog

- [ ] 19.1 Create preferences dialog
- [ ] 19.2 Add language selection UI
- [ ] 19.3 Add dictionary data location setting
- [ ] 19.4 Implement TTS voice selection (language-specific)
- [ ] 19.5 Add SRS algorithm selection (SM-2/FSRS)
- [ ] 19.6 Create SRS parameter configuration
- [ ] 19.7 Add UI theme selection
- [ ] 19.8 Implement audio device selection
- [ ] 19.9 Add cache cleanup controls
- [ ] 19.10 Add language installation management

## 20. Async Task Infrastructure

- [ ] 20.1 Create task.rs module for async operations
- [ ] 20.2 Implement run_blocking helper (like breadbin)
- [ ] 20.3 Add spawning utilities for GTK async compatibility
- [ ] 20.4 Create progress reporting for long operations (including language model downloads)
- [ ] 20.5 Implement cancellation support for async tasks

## 21. Application Configuration

- [ ] 21.1 Create config.rs for settings management
- [ ] 21.2 Implement Settings struct with all configurable options
- [ ] 21.3 Add active language setting to configuration
- [ ] 21.4 Add configuration file loading/saving (TOML format)
- [ ] 21.5 Implement environment variable overrides
- [ ] 21.6 Add XDG config directory support

## 22. Error Handling

- [ ] 22.1 Create error types for all modules (thiserror/anyhow)
- [ ] 22.2 Implement user-friendly error messages
- [ ] 22.3 Add error reporting UI (toast notifications, dialogs)
- [ ] 22.4 Create dependency check on startup (per-language)
- [ ] 22.5 Implement graceful degradation for missing language features

## 23. Custom Styling

- [ ] 23.1 Create style.css with language-inspired color schemes
- [ ] 23.2 Add dark mode support
- [ ] 23.3 Style header bar with language indicator and tab switcher
- [ ] 23.4 Create card styling (hover, active states)
- [ ] 23.5 Style detail dialogs
- [ ] 23.6 Add waveform visualization styling
- [ ] 23.7 Style buttons and form elements

## 24. Integration

- [ ] 24.1 Connect jv-core to jv-gui (dependency setup)
- [ ] 24.2 Implement event/message passing between core and UI
- [ ] 24.3 Create async bridge for core operations
- [ ] 24.4 Add error propagation from core to UI
- [ ] 24.5 Implement state synchronization
- [ ] 24.6 Wire language manager to all language-aware components

## 25. Build and Packaging

- [ ] 25.1 Create build scripts for release
- [ ] 25.2 Add Cargo.toml profiles for release builds
- [ ] 25.3 Implement binary packaging
- [ ] 25.4 Create .desktop file for Linux application menu
- [ ] 25.5 Add AppStream metadata
- [ ] 25.6 Create installation instructions

## 26. Testing

- [ ] 26.1 Add unit tests for jv-core data models
- [ ] 26.2 Create tests for SRS algorithm
- [ ] 26.3 Add tests for dictionary loading (both languages)
- [ ] 26.4 Create tests for language switching
- [ ] 26.5 Create integration tests for audio pipeline
- [ ] 26.6 Add UI tests (if possible with GTK4)
- [ ] 26.7 Implement manual testing checklist

## 27. Documentation

- [ ] 27.1 Create README.md with overview and setup instructions
- [ ] 27.2 Add usage documentation
- [ ] 27.3 Create architecture documentation
- [ ] 27.4 Add language-specific setup instructions
- [ ] 27.5 Add API documentation for jv-core
- [ ] 27.6 Implement help system in UI

## 28. Advanced Features (Optional / Phase 2)

- [ ] 28.1 Implement FSRS backend
- [ ] 28.2 Add phoneme-level pronunciation scoring (Tier 2)
- [ ] 28.3 Implement acoustic analysis scoring (Tier 3 with DTW)
- [ ] 28.4 Add pitch accent detection (Japanese)
- [ ] 28.5 Add Spanish stress pattern detection
- [ ] 28.6 Create stroke order animation (Japanese)
- [ ] 28.7 Implement handwriting practice
- [ ] 28.8 Add import/export for Anki compatibility
- [ ] 28.9 Create mobile companion (future)
- [ ] 28.10 Add support for additional languages (French, German, etc.)
