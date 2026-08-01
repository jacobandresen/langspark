//! LangSpark Core Library
//!
//! This crate contains the business logic for LangSpark, including:
//! - Language management (Japanese, Spanish)
//! - Dictionary integration (JMdict, Kanjidic, Spanish dictionaries)
//! - Vocabulary management
//! - Spaced Repetition System (SM-2 algorithm)
//! - Audio processing (recording, playback, TTS, ASR)
//! - Pronunciation scoring
//!
//! All data structures and core functionality are language-aware and designed
//! to support multiple languages simultaneously.

pub mod audio;
pub mod database;
pub mod dictionary;
pub mod language;
pub mod logging;
pub mod repositories;
pub mod srs;

// Re-export main types for convenience
pub use audio::{AudioManager, PronunciationScorer, SpeechRecognizer, TtsBackend};
pub use database::{Database, Repository, initialize_schema, Migration};
pub use dictionary::{Dictionary, DictionaryManager};
pub use language::{Language, LanguageManager, LanguageRegistry};
pub use logging::init_logging;
pub use repositories::{CardState, KanjiEntry, SrsCard, SrsRating, SqliteKanjiRepository, SqliteSrsRepository, SqliteVocabularyRepository, VocabularyEntry};
pub use srs::{SrsBackend, SrsManager};
