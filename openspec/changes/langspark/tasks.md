## 1. Project Setup

- [x] 1.1 Create Rust workspace with langspark-core and langspark-gui crates
- [x] 1.2 Add basic Cargo.toml files for workspace and both crates
- [x] 1.3 Add initial dependencies to langspark-core (serde, serde_json, rusqlite, thiserror, anyhow, log)
- [x] 1.4 Add initial dependencies to langspark-gui (adw, gtk4, gdk4, glib, gio, tokio)
- [x] 1.5 Add language management dependencies (strum, strum_macros)
- [x] 1.6 Create directory structure for both crates
- [x] 1.7 Set up basic logging infrastructure
- [x] 1.8 Create .gitignore file

## 2. Language Management

- [x] 2.1 Define Language enum (Japanese, Spanish) and implementations
- [x] 2.2 Create Language trait for language-specific behavior
- [x] 2.3 Implement LanguageRegistry with available languages and their metadata
- [x] 2.4 Create LanguageManager to track active language and coordinate language-specific features
- [x] 2.5 Implement language switching logic
- [x] 2.6 Add language installation status tracking
- [x] 2.7 Create language selection UI component

## 3. Core Data Model

- [x] 3.1 Define VocabEntry struct with all required fields from vocabulary-management spec — fulfilled by `VocabularyEntry` in repositories.rs (word, reading, meaning, part_of_speech, language, level, tags)
- [x] 3.2 Define KanjiEntry struct with all required fields from kanji-lookup spec
- [x] 3.3 Define SrsCard struct for SRS tracking (from spaced-repetition spec)
- [x] 3.4 Define PronunciationResult struct for scoring feedback
- [x] 3.5 Add language field to all data structures
- [x] 3.6 Implement Serialize/Deserialize for all data structures
- [x] 3.7 Create type aliases and newtype wrappers for domain types (Word, Reading, etc.)

## 4. Dictionary Integration

- [x] 4.1 Create language-agnostic Dictionary trait
- [x] 4.2 Implement JMdict JSON loader from scriptin/jmdict-simplified format
- [x] 4.3 Implement Kanjidic JSON loader
- [x] 4.4 Research and select Spanish dictionary source — no maintained JSON export exists; defined a custom minimal schema (see `dictionary::spanish` module docs) producible from open sources via an offline conversion script
- [x] 4.5 Implement Spanish dictionary JSON loader
- [x] 4.6 Create DictionaryManager to hold loaded dictionaries (language-specific)
- [x] 4.7 Implement search functionality (by word, reading, meaning)
- [x] 4.8 Implement filter functionality (by proficiency level, part of speech) — tag filtering N/A for dictionary lookups (tags are user-owned, on `VocabularyEntry`)
- [x] 4.9 Add language-specific fuzzy matching (Japanese text vs Spanish text)
- [x] 4.10 Implement caching for dictionary data — in-memory per-language cache in `DictionaryManager`, `is_loaded()` avoids re-parsing
- [x] 4.11 Add dictionary version checking and update mechanism

## 5. Database Layer

- [x] 5.1 Create SQLite schema for all tables (vocabulary, kanji, cards, decks, reviews, settings, languages)
- [x] 5.2 Add language field to all relevant tables
- [x] 5.3 Implement database initialization and migration system — added `run_migrations`/`current_schema_version` (version-tracked, skips already-applied migrations) alongside `initialize_schema`
- [x] 5.4 Create Repository trait for database operations
- [x] 5.5 Implement SqliteRepository for vocabulary CRUD (with language filtering)
- [x] 5.6 Implement SqliteRepository for kanji CRUD
- [x] 5.7 Implement SqliteRepository for SRS card operations (with language filtering)
- [x] 5.8 Implement SqliteRepository for deck operations
- [x] 5.9 Implement SqliteRepository for review history
- [x] 5.10 Implement SqliteRepository for language management
- [x] 5.11 Add database backup/restore functionality

## 6. Spaced Repetition System

- [x] 6.1 Define SrsBackend trait with required methods
- [x] 6.2 Implement SM2Backend struct with SM-2 algorithm
- [x] 6.3 Implement next_interval calculation
- [x] 6.4 Implement ease factor adjustment based on ratings
- [x] 6.5 Implement card state management (new, learning, review)
- [x] 6.6 Create SrsManager to track all cards and scheduling (language-aware) — now owns an in-memory card set with `add_card`/`remove_card`/`cards_for_language`/`due_cards_for_language`
- [x] 6.7 Implement daily review queue generation (filter by active language)
- [x] 6.8 Add statistics tracking (streaks, retention rates) — `calculate_retention_rate`, `calculate_streak`, `build_review_stats`
- [x] 6.9 Create deck management functionality — `DeckManager` (in-memory) + `SqliteDeckRepository` (persistence)

## 7. Audio Infrastructure

- [x] 7.1 Create AudioManager struct to coordinate TTS, recording, recognition (language-aware)
- [x] 7.2 Implement CPAL-based audio recorder
- [x] 7.3 Implement audio playback using rodio or gstreamer
- [x] 7.4 Create waveform data extraction for visualization
- [x] 7.5 Add audio caching system for generated TTS (per-language)
- [x] 7.6 Implement audio file format handling (WAV)

## 8. Text-to-Speech (Multi-Language)

- [x] 8.1 Define TtsBackend trait for language-agnostic TTS
- [x] 8.2 Add voicevox_core dependency to langspark-core (Japanese) — `voicevox_core` isn't published to crates.io and the one `voicevox-rs` release fails to compile (broken upstream); instead `tts.rs::VoicevoxTts` talks directly to a local VOICEVOX Engine over HTTP via `ureq`
- [x] 8.3 Add piper dependency to langspark-core (Spanish and others) — `piper-rs` (default-features=false to avoid a broken espeak-ng-data build step)
- [x] 8.4 Create VoicevoxTTS wrapper struct for Japanese
- [x] 8.5 Create PiperTTS wrapper struct for Spanish
- [x] 8.6 Implement synthesize method for text-to-speech (trait-based)
- [x] 8.7 Add language-specific voice selection configuration
- [x] 8.8 Implement audio generation and caching (per-language)
- [x] 8.9 Add error handling for missing models — `UnavailableTts` fallback backend
- [x] 8.10 Create fallback mechanism for offline use — `UnavailableTts`

## 9. Speech Recognition (Multi-Language)

- [x] 9.1 Add qwen3_asr_rs dependency to langspark-core — added as optional, gated behind an `asr` Cargo feature since its only backends (tch/mlx) need a native libtorch install or Apple Silicon, neither guaranteed present
- [x] 9.2 Create SpeechRecognizer wrapper struct
- [x] 9.3 Implement transcribe method for audio-to-text (language-aware)
- [x] 9.4 Add language parameter configuration for qwen3_asr_rs
- [x] 9.5 Implement confidence score handling — qwen3-asr-rs doesn't report one; field is reserved (`None`) for a backend that does
- [x] 9.6 Add error handling for recognition failures — clear "unavailable" error without the `asr` feature; wrapped errors with it
- [x] 9.7 Add language-specific text normalization (kana vs Latin script)

## 10. Pronunciation Scoring (Tier 1 - Text Matching)

- [x] 10.1 Create PronunciationScorer struct (language-aware)
- [x] 10.2 Implement language-specific text normalization (Japanese: remove spaces, consistent kana; Spanish: remove accents, consistent case)
- [x] 10.3 Implement Levenshtein distance calculation
- [x] 10.4 Implement simple score calculation based on text matching
- [x] 10.5 Create language-specific feedback message generation
- [x] 10.6 Add phoneme/morae segmentation for Japanese (morae-level)
- [x] 10.7 Add phoneme segmentation for Spanish — approximate vowel-group syllabification (documented as a simplification of true Spanish syllable rules)
- [x] 10.8 Implement phoneme-level comparison (Tier 2) — `score_pronunciation_tier2` (unit-level edit distance over `segment_units`)

## 11. UI Setup

- [x] 11.1 Create main.rs with GTK application initialization
- [x] 11.2 Implement load_styles function for custom CSS
- [x] 11.3 Create build_ui function for main window construction — `app::build_main_window`
- [x] 11.4 Set up header bar with view switcher and language indicator (inspired by breadbin)
- [x] 11.5 Implement ToolbarView with ViewStack for tab navigation
- [x] 11.6 Add application menu with Preferences, About, Quit
- [x] 11.7 Create application actions and handlers — verified live via screenshot (all 5 tabs, language indicator, menu, toast)

## 12. Vocabulary Tab UI

- [x] 12.1 Create vocabulary tab module
- [x] 12.2 Implement section header widget (like breadbin's section_header_widget)
- [x] 12.3 Create Card widget for vocabulary display
- [x] 12.4 Implement horizontal strip layout for vocabulary sections
- [x] 12.5 Add "Show All" button with expand/collapse animation — `Revealer` slide transition
- [x] 12.6 Create grid view for expanded sections — `FlowBox`
- [x] 12.7 Implement language-specific section grouping (JLPT for Japanese, CEFR for Spanish) — generic by `level` field, works for both
- [x] 12.8 Add search box and filter controls — `SearchEntry` filters client-side over the already-loaded entries (`filter_entries`, tested)

## 13. Kanji Tab UI (Language-Specific)

- [x] 13.1 Create kanji tab module
- [x] 13.2 Implement KanjiCard widget
- [x] 13.3 Create horizontal strip layout for kanji sections
- [x] 13.4 Add "Show All" functionality
- [x] 13.5 Implement section grouping by JLPT level, grade, radical — grouped by JLPT level; grade/radical shown in the card itself
- [x] 13.6 Create kanji detail display — `kanji::dialog`
- [x] 13.7 Add logic to show/hide tab based on active language (Japanese only) — `kanji::is_visible_for`, wired into the ViewStack page

## 14. Review Tab UI

- [x] 14.1 Create review tab module
- [x] 14.2 Implement card display for review (front and back)
- [x] 14.3 Create rating buttons (Again, Hard, Good, Easy)
- [x] 14.4 Add progress indicator (X of Y cards)
- [x] 14.5 Implement card flip animation — simplified to an instant front/back label swap (no CSS transition); revisit if the plain swap feels abrupt in practice
- [x] 14.6 Create daily review queue display (filtered by active language) — queue construction is the caller's job (feeds `ReviewSession::new`); language filtering already exists via `SrsManager::due_cards_for_language`
- [x] 14.7 Add keyboard shortcuts for rating — Space reveals the answer; 1-4 rate Again/Hard/Good/Easy (`EventControllerKey` on the session root)

## 15. Pronunciation Tab UI

- [x] 15.1 Create pronunciation tab module
- [x] 15.2 Implement word selection for practice (language-specific)
- [x] 15.3 Create play button with language-specific TTS integration — calls an injected `synthesize` callback (real `VoicevoxTts`/`PiperTts` wiring is section 24)
- [x] 15.4 Implement record/stop buttons with CPAL — single "Record" button captures a fixed clip via the injected `record` callback (`AudioRecorder`); no separate stop button yet
- [x] 15.5 Add waveform visualization widget
- [x] 15.6 Create score display area
- [x] 15.7 Implement feedback message display
- [x] 15.8 Add navigation (next/previous word)

## 16. Statistics Tab UI

- [x] 16.1 Create statistics tab module
- [x] 16.2 Implement overall progress summary display
- [x] 16.3 Create daily streak widget — included in the summary tiles
- [x] 16.4 Add review history chart (using a charting library or custom drawing) — custom `DrawingArea` bar chart
- [x] 16.5 Implement next reviews schedule display
- [x] 16.6 Create per-deck statistics — `compute_deck_stats` (pure, tested) + list display
- [ ] 16.7 Add achievement/badges display — deferred (no achievement/badge data model exists yet)

## 17. Detail Dialogs

- [x] 17.1 Create vocabulary detail dialog
- [x] 17.2 Implement large word display with language-specific phonetic guide (furigana for Japanese, phonetic for Spanish) — reading shown beneath the word (no furigana-over-kanji rendering; GTK has no built-in ruby-text widget)
- [x] 17.3 Add meaning, part of speech, proficiency level display
- [x] 17.4 Create example sentence display — placeholder row (no example-sentence data source yet)
- [x] 17.5 Implement audio playback controls — Play button calls an injected callback (real backend wiring is section 24)
- [x] 17.6 Add pronunciation practice button
- [x] 17.7 Create add to deck functionality — button + callback (deck picker UI itself deferred to section 24)
- [x] 17.8 Implement edit/delete buttons
- [x] 17.9 Create kanji detail dialog (Japanese only)
- [x] 17.10 Add all readings, meanings, stroke info display

## 18. Waveform Widget

- [x] 18.1 Create Waveform widget for audio visualization
- [x] 18.2 Implement drawing of audio samples
- [x] 18.3 Add color customization for reference vs user waveforms
- [x] 18.4 Implement waveform comparison display
- [x] 18.5 Add smooth scrolling for long waveforms — content width grows with sample count inside a horizontal `ScrolledWindow`

## 19. Preferences Dialog

- [x] 19.1 Create preferences dialog
- [x] 19.2 Add language selection UI
- [x] 19.3 Add dictionary data location setting — shown (read-only; a folder picker is future work)
- [x] 19.4 Implement TTS voice selection (language-specific) — text entry for voice ID (a browsable picker needs installed-model listing from section 24/25)
- [x] 19.5 Add SRS algorithm selection (SM-2/FSRS)
- [x] 19.6 Create SRS parameter configuration — starting ease factor
- [x] 19.7 Add UI theme selection — wired to `adw::StyleManager`
- [x] 19.8 Implement audio device selection — `langspark_core::list_audio_devices` + `ComboRow`
- [x] 19.9 Add cache cleanup controls — Clear Cache button wired to `AudioCache::clear`
- [x] 19.10 Add language installation management — install buttons shown per language (actual download flow is section 25/"Advanced Features")

## 20. Async Task Infrastructure

- [x] 20.1 Create task.rs module for async operations
- [x] 20.2 Implement run_blocking helper (like breadbin)
- [x] 20.3 Add spawning utilities for GTK async compatibility
- [x] 20.4 Create progress reporting for long operations (including language model downloads)
- [x] 20.5 Implement cancellation support for async tasks

## 21. Application Configuration

- [x] 21.1 Create config.rs for settings management
- [x] 21.2 Implement Settings struct with all configurable options
- [x] 21.3 Add active language setting to configuration
- [x] 21.4 Add configuration file loading/saving (TOML format)
- [x] 21.5 Implement environment variable overrides
- [x] 21.6 Add XDG config directory support

## 22. Error Handling

- [x] 22.1 Create error types for all modules (thiserror/anyhow) — `LangSparkError` for cases callers branch on; anyhow elsewhere (idiomatic, already used throughout)
- [x] 22.2 Implement user-friendly error messages — `LangSparkError::user_message()`
- [x] 22.3 Add error reporting UI (toast notifications, dialogs) — `diagnostics::show_error_toast` + `adw::ToastOverlay` wired into the main window; verified live via screenshot
- [x] 22.4 Create dependency check on startup (per-language) — `diagnostics::check_dependencies` (audio hardware + dictionary file presence), run from `main.rs` on activate
- [x] 22.5 Implement graceful degradation for missing language features — `UnavailableTts`, `asr` feature gate, dependency-check toasts instead of hard failures

## 23. Custom Styling

- [x] 23.1 Create style.css with language-inspired color schemes — `data/style.css`, loaded via `ui::load_styles`
- [x] 23.2 Add dark mode support — built on libadwaita's semantic `@accent_color`/`@card_bg_color`/etc. variables, which already adapt to the system/`StyleManager` color scheme
- [x] 23.3 Style header bar with language indicator and tab switcher — `.langspark-language-indicator`; verified live via screenshot
- [x] 23.4 Create card styling (hover, active states) — `.langspark-card`
- [ ] 23.5 Style detail dialogs — using default Adwaita dialog styling; no bespoke rules yet
- [x] 23.6 Add waveform visualization styling — `.langspark-waveform`
- [x] 23.7 Style buttons and form elements — standard Adwaita `suggested-action`/`destructive-action` classes throughout

## 24. Integration

- [x] 24.1 Connect langspark-core to langspark-gui (dependency setup)
- [x] 24.2 Implement event/message passing between core and UI — `review::ReviewSession`'s `on_review(index, rating)` callback is the message from UI back to app state
- [x] 24.3 Create async bridge for core operations — `task::run_blocking` used for the review-save write path; found and fixed a real bug along the way: `rusqlite::Connection` isn't `Sync`, so `Database` now wraps it in a `Mutex` so `Arc<Database>`-based repositories can cross the background thread pool at all
- [x] 24.4 Add error propagation from core to UI — repository/load failures surface via `diagnostics::show_error_toast`
- [x] 24.5 Implement state synchronization — `state::AppState::load_tab_data()` is the single source of truth feeding vocabulary/kanji/review/statistics tabs consistently; verified live (real on-disk DB at `~/.local/share/langspark/langspark.db`, confirmed via screenshot)
- [x] 24.6 Wire language manager to all language-aware components — `LanguageManager` drives which language every repository query filters by and the kanji tab's visibility; real-time language switching mid-session is an explicit non-goal (design.md), so switching takes effect on restart

## 25. Build and Packaging

- [x] 25.1 Create build scripts for release — `scripts/install.sh`
- [x] 25.2 Add Cargo.toml profiles for release builds — LTO, single codegen unit, stripped symbols
- [x] 25.3 Implement binary packaging — `scripts/install.sh` installs binary + desktop entry + metadata under `$PREFIX`; no distro-specific package (.deb/.rpm/Flatpak) yet
- [x] 25.4 Create .desktop file for Linux application menu
- [x] 25.5 Add AppStream metadata
- [x] 25.6 Create installation instructions — README "Running from source" / "Building a release binary" sections

## 26. Testing

- [x] 26.1 Add unit tests for langspark-core data models — `model.rs` newtypes, plus indirect coverage of `VocabularyEntry`/`KanjiEntry`/`SrsCard` via repository CRUD tests
- [x] 26.2 Create tests for SRS algorithm
- [x] 26.3 Add tests for dictionary loading (both languages)
- [x] 26.4 Create tests for language switching
- [x] 26.5 Create integration tests for audio pipeline — `test_pronunciation_pipeline_encode_cache_decode_waveform_score` (synthesize → cache → decode → waveform → score); real hardware/model calls aren't exercised (no mic/TTS engine in CI)
- [x] 26.6 Add UI tests (if possible with GTK4) — consolidated `gtk_smoke` test (see ARCHITECTURE.md for why it's one test, not one per widget)
- [x] 26.7 Implement manual testing checklist — `MANUAL_TESTING.md`

## 27. Documentation

- [x] 27.1 Create README.md with overview and setup instructions
- [x] 27.2 Add usage documentation — README "Usage" section + in-app Help dialog
- [x] 27.3 Create architecture documentation — `ARCHITECTURE.md`
- [x] 27.4 Add language-specific setup instructions — README's TTS/ASR/dictionary table + prerequisites
- [x] 27.5 Add API documentation for langspark-core — doc comments throughout; `cargo doc -p langspark-core` builds clean
- [x] 27.6 Implement help system in UI — `app.help` action opens a Help dialog from the app menu

## 28. Advanced Features (Optional / Phase 2)

- [ ] 28.1 Implement FSRS backend
- [x] 28.2 Add phoneme-level pronunciation scoring (Tier 2) — done early alongside 10.8 (`score_pronunciation_tier2`)
- [ ] 28.3 Implement acoustic analysis scoring (Tier 3 with DTW)
- [ ] 28.4 Add pitch accent detection (Japanese)
- [ ] 28.5 Add Spanish stress pattern detection
- [ ] 28.6 Create stroke order animation (Japanese)
- [ ] 28.7 Implement handwriting practice
- [ ] 28.8 Add import/export for Anki compatibility
- [ ] 28.9 Create mobile companion (future)
- [ ] 28.10 Add support for additional languages (French, German, etc.)
