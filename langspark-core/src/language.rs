//! Language management module
//!
//! Handles language configuration, switching, and resource management.

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
pub enum Language {
    /// Japanese
    Japanese,
    /// Spanish
    Spanish,
}

/// Registry of available languages and their metadata
pub struct LanguageRegistry;

/// Manages the active language and coordinates language-specific features
pub struct LanguageManager;

impl Language {
    /// Get the language code (e.g., "ja" for Japanese, "es" for Spanish)
    pub fn code(&self) -> &'static str {
        match self {
            Language::Japanese => "ja",
            Language::Spanish => "es",
        }
    }
    
    /// Get the display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Japanese => "Japanese",
            Language::Spanish => "Spanish",
        }
    }
}
