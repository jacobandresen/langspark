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

pub mod asr;
pub mod audio;
pub mod database;
pub mod dictionary;
pub mod error;
pub mod installer;
pub mod language;
pub mod logging;
pub mod model;
pub mod pronunciation;
pub mod repositories;
pub mod seed;
pub mod srs;
pub mod tts;

// Re-export main types for convenience
pub use asr::{SpeechRecognizer, TranscriptionResult};
pub use audio::{audio_devices_available, list_audio_devices, AudioCache, AudioManager, AudioPlayer, AudioRecorder};
pub use error::LangSparkError;
pub use database::{Database, Repository, default_migrations, initialize_schema, run_migrations, Migration};
pub use dictionary::{Dictionary, DictionaryManager, ExampleSentence, TatoebaExamples, VocabEntry, VocabFilter};
pub use installer::{
    install_asr_model, install_jmdict, install_kanjidic, install_tatoeba_examples, install_voicevox_engine,
    voicevox_run_executable_name,
};
pub use language::{InstallationStatus, Language, LanguageInfo, LanguageManager, LanguageRegistry};
pub use logging::init_logging;
pub use model::{Meaning, Reading, Word};
pub use seed::{ja_school_vocabulary_len, seed_ja_school_vocabulary};
pub use pronunciation::{
    diff_chars, levenshtein_distance, score_pronunciation, score_pronunciation_tier2, segment_units, DiffOp,
    PronunciationResult, PronunciationScorer,
};
pub use repositories::{
    Deck, KanjiEntry, LanguageRecord, ReviewRecord, SqliteDeckRepository, SqliteKanjiRepository,
    SqliteLanguageRepository, SqliteReviewRepository, SqliteSrsRepository, SqliteVocabularyRepository,
    VocabularyEntry,
};
pub use srs::{
    build_review_stats, calculate_retention_rate, calculate_streak, CardState, DeckManager, FSRSBackend, ReviewStats,
    SrsBackend, SrsCard, SrsManager, SM2Backend, RATING_AGAIN, RATING_EASY, RATING_GOOD, RATING_HARD,
};
pub use tts::{TtsBackend, UnavailableTts, VoicevoxTts};

/// Rating for SRS (1=Again, 2=Hard, 3=Good, 4=Easy)
pub type SrsRating = u32;
