# LangSpark Implementation Roadmap

This roadmap maps implementation tasks directly to OpenSpec capability specifications. Each task references its corresponding spec file and requirement, ensuring traceability from specification to implementation.

All spec files are located in `openspec/changes/langspark/specs/`.

## Phase 1: Project Foundation

*Spec: Project infrastructure (not in capability specs)*

### Core Infrastructure
- [x] Set up Rust workspace with `langspark-core` and `langspark-gui` crates
- [x] Configure Cargo.toml dependencies for both crates
- [ ] Set up basic CI/CD pipeline (cargo build, test, clippy, fmt)
- [x] Create project directory structure

### Data Layer Foundation
- [ ] Define core data structures (Language, VocabularyEntry, Kanji, SrsCard, etc.)
- [ ] Set up SQLite connection with rusqlite
- [ ] Create database schema migration system
- [ ] Implement basic database operations (open, close, migrate)

### Testing Framework
- [ ] Set up test project structure
- [ ] Create mock database for unit tests
- [ ] Implement test utilities for common scenarios
- [ ] Add basic test coverage reporting

### Logging
- [x] Add logging module to langspark-core
- [x] Add init_logging function
- [x] Add logging re-export in lib.rs
- [x] Add debug check utility

## Phase 2: Language Management

*Spec: `language-management/spec.md`*

### Requirement: Support multiple languages
- [x] Implement Language enum with Japanese and Spanish support (Scenario: User selects Japanese/Spanish)
- [x] Add Language trait for language-specific behavior
- [ ] Configure VOICEVOX TTS for Japanese
- [ ] Configure Piper TTS with Spanish model for Spanish
- [ ] Configure qwen3_asr_rs for both languages

### Requirement: Language switching
- [ ] Build language preference persistence (SQLite)
- [ ] Implement active language getter/setter (Scenario: User switches from Japanese to Spanish)
- [ ] Add language-specific resource path resolution
- [ ] Persist selection across sessions

### Language Module
- [x] Create LanguageRegistry with available languages and their metadata
- [x] Create LanguageManager to track active language and coordinate language-specific features

### Requirement: Language installation
- [ ] Create download manager for JMdict JSON
- [ ] Create download manager for Kanjidic JSON
- [ ] Create download manager for Spanish dictionary JSON
- [ ] Create download manager for VOICEVOX models
- [ ] Create download manager for Piper Spanish model
- [ ] Implement progress reporting and file validation (Scenario: User installs Japanese/Spanish)

### Tests
- [ ] Unit tests for Language enum and configuration
- [ ] Unit tests for language switching logic
- [ ] Integration test for language persistence
- [ ] Integration test for download and installation

## Phase 3: Dictionary Integration

*Spec: `dictionary-integration/spec.md`*

### Dictionary Loading
- [ ] Implement dictionary loader for JMdict JSON
- [ ] Implement dictionary loader for Kanjidic JSON
- [ ] Implement dictionary loader for Spanish dictionary JSON
- [ ] Create in-memory dictionary cache

### Dictionary Querying
- [ ] Build search index for fast lookups
- [ ] Add basic query API (lookup by word, kanji, reading)
- [ ] Implement fuzzy search capability
- [ ] Add filtering by level/topic/frequency

### Data Setup
- [ ] Download scripts for all dictionary files
- [ ] Data directory structure setup with license documentation

### Tests
- [ ] Unit tests for dictionary parsing
- [ ] Unit tests for search functionality
- [ ] Unit tests for filtering
- [ ] Integration test for dictionary loading performance

## Phase 4: Vocabulary Management

*Spec: `vocabulary-management/spec.md`*

### Data Structures
- [x] Define VocabEntry struct with all required fields from vocabulary-management spec
- [x] Define KanjiEntry struct with all required fields from kanji-lookup spec
- [x] Define SrsCard struct for SRS tracking (from spaced-repetition spec)
- [x] Define PronunciationResult struct for scoring feedback
- [x] Add language field to all data structures
- [ ] Implement Serialize/Deserialize for all data structures
- [ ] Create type aliases and newtype wrappers for domain types (Word, Reading, etc.)

### Vocabulary CRUD
- [ ] Implement VocabularyEntry create operation
- [ ] Implement VocabularyEntry read operation
- [ ] Implement VocabularyEntry update operation
- [ ] Implement VocabularyEntry delete operation
- [ ] Implement bulk import/export operations

### Vocabulary Organization
- [ ] Create vocabulary grouping by level
- [ ] Create vocabulary grouping by topic
- [ ] Create vocabulary grouping by frequency
- [ ] Add vocabulary tagging system

### Vocabulary Search
- [ ] Implement search by word/reading/meaning
- [ ] Implement filter by language
- [ ] Implement filter by level
- [ ] Implement filter by tags

### Tests
- [ ] Unit tests for each CRUD operation
- [ ] Unit tests for search and filtering
- [ ] Integration test for vocabulary persistence

## Phase 5: SRS Engine Module

*Spec: `spaced-repetition/spec.md`*

### SRS Data Structures
- [x] Define SrsBackend trait with required methods
- [x] Implement SM2Backend struct with SM-2 algorithm
- [x] Define SrsCard struct for SRS tracking
- [x] Create SrsManager to track all cards and scheduling (language-aware)

## Phase 5.5: Kanji Lookup

*Spec: `kanji-lookup/spec.md`*

### Kanji Data Loading
- [ ] Load Kanjidic JSON data
- [ ] Create kanji in-memory cache
- [ ] Build kanji search index

### Kanji Querying
- [ ] Implement lookup by kanji character
- [ ] Implement lookup by reading (on/kun)
- [ ] Implement lookup by meaning
- [ ] Implement lookup by radical
- [ ] Implement filtering by JLPT level
- [ ] Implement filtering by grade level
- [ ] Implement filtering by stroke count

### Tests
- [ ] Unit tests for kanji parsing
- [ ] Unit tests for search by character
- [ ] Unit tests for search by reading
- [ ] Unit tests for filtering
- [ ] Integration test for kanji lookup performance

## Phase 6: Audio Module

*Spec: `pronunciation-practice/spec.md`*

### Audio Module
- [x] Create AudioManager struct to coordinate TTS, recording, recognition (language-aware)
- [x] Implement CPAL-based audio recorder placeholder
- [x] Implement audio playback placeholder
- [x] Create waveform data extraction for visualization placeholder
- [x] Build audio caching system for TTS output placeholder
- [x] Implement audio file format handling (WAV) placeholder

## Phase 6.5: Spaced Repetition Engine

*Spec: `spaced-repetition/spec.md`*

### Requirement: Implement SM-2 algorithm
- [ ] Implement SM-2 algorithm core logic (Scenario: User reviews new card first time)
- [ ] Implement interval calculation for first review
- [ ] Implement interval calculation for subsequent reviews (Scenario: User reviews card second time)
- [ ] Implement ease factor adjustment (Scenario: User reviews card with ease factor adjustment)
- [ ] Implement "Again" rating logic (Scenario: User fails to recall card)

### Requirement: Manage card states
- [ ] Create new SRS card in "New" state (Scenario: User adds new word to study)
- [ ] Implement state transition: New -> Learning
- [ ] Implement state transition: Learning -> Review (Scenario: Card progresses through learning stages)
- [ ] Manage card associations with vocabulary entries

### Requirement: Generate daily review queue
- [ ] Query all cards due today
- [ ] Sort by next review date (oldest first)
- [ ] Limit to daily review limit
- [ ] Return cards with state (Scenario: System generates daily review queue)

### Requirement: Track statistics
- [ ] Implement daily streak tracking
- [ ] Implement session duration recording
- [ ] Implement cards reviewed counting
- [ ] Implement retention rate calculation (Scenario: User completes daily review)

### Requirement: Performance
- [ ] Optimize scheduling for 10,000 cards under 100ms
- [ ] Optimize loading daily queue under 200ms
- [ ] Verify SRS state memory under 10MB (Scenario: System calculates schedules for many cards)

### Tests
- [ ] Unit tests for SM-2 calculation logic
- [ ] Unit tests for each rating (Again, Hard, Good, Easy)
- [ ] Unit tests for state transitions
- [ ] Integration test for queue generation with 100+ cards
- [ ] Performance test: scheduling for 10,000 cards

## Phase 7: Audio Infrastructure

*Spec: `pronunciation-practice/spec.md` (Partial)*

### Audio Capture and Playback
- [ ] Set up CPAL for audio capture
- [ ] Implement audio recording with start/stop/control
- [ ] Add audio playback functionality
- [ ] Create waveform data extraction for visualization
- [ ] Build audio caching system for TTS output

### Tests
- [ ] Unit tests for audio configuration
- [ ] Integration test for recording/playback pipeline

## Phase 8: TTS Integration

*Spec: `pronunciation-practice/spec.md` - Requirement: Language-aware TTS*

### VOICEVOX Integration (Japanese)
- [ ] Integrate VOICEVOX TTS engine
- [ ] Implement Japanese text-to-speech synthesis (Scenario: User plays Japanese word pronunciation)
- [ ] Add audio caching for repeated playbacks

### Piper Integration (Spanish)
- [ ] Integrate Piper TTS engine with Spanish model
- [ ] Implement Spanish text-to-speech synthesis (Scenario: User plays Spanish word pronunciation)
- [ ] Add audio caching for repeated playbacks

### Language-Aware Abstraction
- [ ] Create TTS trait/interface
- [ ] Implement language-specific TTS selection
- [ ] Add volume control
- [ ] Add playback management

### Tests
- [ ] Integration test for Japanese TTS
- [ ] Integration test for Spanish TTS
- [ ] Performance test: TTS generation under 1 second

## Phase 9: Speech Recognition

*Spec: `pronunciation-practice/spec.md` - Requirement: Speech recognition*

### qwen3_asr_rs Integration
- [ ] Set up qwen3_asr_rs for Japanese
- [ ] Set up qwen3_asr_rs for Spanish
- [ ] Implement speech recognition abstraction
- [ ] Add transcription result processing

### Language-Specific Recognition
- [ ] Implement Japanese speech recognition (Scenario: System recognizes Japanese speech)
- [ ] Implement Spanish speech recognition (Scenario: System recognizes Spanish speech)
- [ ] Return confidence scores

### Tests
- [ ] Integration test for Japanese recognition
- [ ] Integration test for Spanish recognition
- [ ] Performance test: recognition under 2 seconds

## Phase 10: Pronunciation Scoring

*Spec: `pronunciation-practice/spec.md` - Requirements: Pronunciation scoring, Display feedback*

### Scoring Engine
- [ ] Implement text normalization (remove punctuation, normalize case)
- [ ] Implement Levenshtein distance calculation
- [ ] Implement score calculation (0-100%)
- [ ] Generate human-readable feedback

### User Experience
- [ ] Display overall score (Scenario: System scores Japanese/Spanish pronunciation)
- [ ] Display recognized vs expected text
- [ ] Display waveform comparison
- [ ] Provide option to try again (Scenario: User sees pronunciation feedback)

### Tests
- [ ] Unit tests for text normalization
- [ ] Unit tests for Levenshtein distance
- [ ] Unit tests for scoring logic
- [ ] Integration test for end-to-end scoring

## Phase 11: Audio Recording for Practice

*Spec: `pronunciation-practice/spec.md` - Requirement: Audio recording*

### Recording Implementation
- [ ] Capture audio from microphone at 44.1kHz (Scenario: User records pronunciation attempt)
- [ ] Display waveform in real-time
- [ ] Allow stopping recording
- [ ] Save recording for scoring

### Tests
- [ ] Integration test for recording workflow
- [ ] Performance test: recording latency under 100ms

## Phase 12: UI Foundation

*Spec: `ui-kiosk/spec.md` - Requirements: Application window, Tab navigation*

### Application Window
- [ ] Create main window 800x600+ (Scenario: User launches application)
- [ ] Apply custom styling
- [ ] Display header bar with title and language indicator
- [ ] Initialize all tabs

### Tab Navigation
- [ ] Set up ViewStack for tab switching
- [ ] Set up ViewSwitcher for tab transitions
- [ ] Implement tab switching in under 100ms (Scenario: User switches between tabs)
- [ ] Load content on demand

### Tests
- [ ] UI test for window creation
- [ ] UI test for tab switching performance

## Phase 13: Vocabulary UI

*Spec: `ui-kiosk/spec.md` - Requirement: Tab navigation (Vocabulary tab)*

### Vocabulary Tab
- [ ] Display vocabulary grouped by language-specific levels (Scenario: User views Vocabulary tab)
- [ ] Show cards horizontally (max 8 per row)
- [ ] Provide "Show All" for large groups
- [ ] Open detail dialog on click

### Vocabulary Detail Dialog
- [ ] Display large word with phonetic guide (Scenario: User opens vocabulary detail dialog)
- [ ] Show meaning/part_of_speech/level
- [ ] Show example sentence
- [ ] Add audio controls (play, stop)
- [ ] Add pronunciation practice button
- [ ] Add add-to-deck option

### Tests
- [ ] UI test for vocabulary card rendering
- [ ] UI test for detail dialog opening

## Phase 14: Kanji UI (Japanese only)

*Spec: `ui-kiosk/spec.md` - Requirement: Tab navigation (Kanji tab)*

### Kanji Tab
- [ ] Display kanji grouped by JLPT/grade/radius (Scenario: User views Kanji tab)
- [ ] Show cards horizontally
- [ ] Provide "Show All" for large groups
- [ ] Open detail dialog on click
- [ ] Hide tab when Spanish is active

### Kanji Detail Dialog
- [ ] Display large kanji (Scenario: User opens kanji detail dialog)
- [ ] Show all readings (on/kun)
- [ ] Show all meanings
- [ ] Show stroke count
- [ ] Show radical
- [ ] Add audio for each reading

### Tests
- [ ] UI test for kanji card rendering
- [ ] UI test for kanji detail dialog

## Phase 15: Review UI

*Spec: `ui-kiosk/spec.md` - Requirement: Tab navigation (Review tab)*

### Review Tab
- [ ] Display all cards due today (Scenario: User views Review tab)
- [ ] Show card front by default
- [ ] Provide rating buttons (Again, Hard, Good, Easy)
- [ ] Show progress (X of Y)
- [ ] Flip card on click

### Integration
- [ ] Connect to langspark-core for daily queue
- [ ] Update SRS state after rating
- [ ] Save review session statistics

### Tests
- [ ] UI test for review card display
- [ ] UI test for rating workflow
- [ ] Integration test for end-to-end review session

## Phase 16: Pronunciation UI

*Spec: `ui-kiosk/spec.md` - Requirement: Tab navigation (Pronunciation tab)*

### Pronunciation Tab
- [ ] Display word selection (Scenario: User views Pronunciation tab)
- [ ] Provide Play/Record/Stop buttons
- [ ] Display waveform
- [ ] Show score and feedback after recording

### Integration
- [ ] Connect Play to TTS
- [ ] Connect Record to audio capture
- [ ] Connect scoring display
- [ ] Connect feedback display

### Tests
- [ ] UI test for pronunciation workflow
- [ ] Integration test for end-to-end pronunciation practice

## Phase 17: Statistics UI

*Spec: `ui-kiosk/spec.md` - Requirement: Tab navigation (Statistics tab)*

### Statistics Tab
- [ ] Display overall progress (Scenario: User views Statistics tab)
- [ ] Display daily streak
- [ ] Display review history chart
- [ ] Display next reviews schedule
- [ ] Display per-deck statistics

### Tests
- [ ] UI test for statistics rendering

## Phase 18: Header and Navigation

*Spec: `ui-kiosk/spec.md` - Requirement: Header bar with language indicator*

### Header Bar
- [ ] Display current language name (Scenario: User sees language indicator)
- [ ] Display flag emoji
- [ ] Display visual indicator of active language

### Tests
- [ ] UI test for header bar display

## Phase 19: Keyboard Shortcuts

*Spec: `ui-kiosk/spec.md` - Requirement: Keyboard shortcuts*

### Shortcut Implementation
- [ ] Space reveals answer during review (Scenario: User uses keyboard for review)
- [ ] Keys 1-4 rate card (Again, Hard, Good, Easy)
- [ ] Escape closes dialogs
- [ ] Ctrl+P plays audio
- [ ] Ctrl+R records audio
- [ ] Ctrl+S stops recording

### Tests
- [ ] UI test for keyboard shortcuts

## Phase 20: Performance and Polish

*Spec: Various performance requirements across all specs*

### Core Performance
- [ ] Verify UI remains responsive at 60fps (ui-kiosk: Performance)
- [ ] Verify tab switching under 100ms (ui-kiosk: Performance)
- [ ] Verify card loading is smooth
- [ ] Verify UI memory under 150MB

### Code Quality
- [ ] Comprehensive error handling
- [ ] Logging system
- [ ] Configuration management
- [ ] Responsive design for different screen sizes

### Tests
- [ ] Full regression test suite
- [ ] Performance benchmarking
- [ ] UI responsiveness testing

## Delivery Checklist

- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] Code passes clippy checks
- [ ] Code is formatted with rustfmt
- [ ] Documentation is complete
- [ ] All data licenses are documented
- [ ] Build succeeds on target platforms
- [ ] All spec scenarios are implemented and tested

## Spec to Phase Mapping

| Spec File | Phases |
|-----------|--------|
| language-management | 2 |
| dictionary-integration | 3 |
| vocabulary-management | 4 |
| kanji-lookup | 5, 14 |
| spaced-repetition | 6 |
| pronunciation-practice | 7-11 |
| ui-kiosk | 12-19 |
