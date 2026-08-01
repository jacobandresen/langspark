//! Dictionary integration module
//!
//! Loads and queries language-specific dictionaries (JMdict, Kanjidic, Spanish).

/// Trait for dictionary operations
pub trait Dictionary {
    /// Look up a word
    fn lookup(&self, query: &str) -> Result<Vec<VocabEntry>, anyhow::Error>;
}

/// Manages all loaded dictionaries
pub struct DictionaryManager;

/// A vocabulary entry from the dictionary
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VocabEntry;
