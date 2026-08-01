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
pub mod dictionary;
pub mod language;
pub mod logging;
pub mod srs;
pub mod vocabulary;

// Re-export main types for convenience
pub use audio::{AudioManager, PronunciationScorer, SpeechRecognizer, TtsBackend};
pub use dictionary::{Dictionary, DictionaryManager, VocabEntry};
pub use language::{Language, LanguageManager, LanguageRegistry};
pub use logging::init_logging;
pub use srs::{SrsBackend, SrsCard, SrsManager};
pub use vocabulary::KanjiEntry;
