//! Vocabulary and Kanji management module
//!
//! Handles vocabulary entries and kanji data.

/// A vocabulary entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VocabEntry;

/// A kanji entry with readings, meanings, and metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KanjiEntry;
