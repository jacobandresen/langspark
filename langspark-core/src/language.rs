//! Language management module
//!
//! Handles language configuration, switching, and resource management.

use anyhow;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
pub enum Language {
    /// Japanese
    Japanese,
    /// Spanish
    Spanish,
}

/// Metadata for a language including its resources and capabilities
#[derive(Debug, Clone)]
pub struct LanguageMetadata {
    /// Language code (e.g., "ja", "es")
    pub code: &'static str,
    /// Display name
    pub display_name: &'static str,
    /// Flag emoji
    pub flag_emoji: &'static str,
    /// Whether this language supports kanji
    pub supports_kanji: bool,
    /// Default TTS voice for this language
    pub default_tts_voice: &'static str,
    /// Default ASR model for this language
    pub default_asr_model: &'static str,
}

/// Registry of available languages and their metadata
pub struct LanguageRegistry {
    available_languages: Vec<(Language, LanguageMetadata)>,
}

impl LanguageRegistry {
    /// Create a new language registry with all supported languages
    pub fn new() -> Self {
        let mut registry = Self {
            available_languages: Vec::new(),
        };
        
        // Register Japanese
        registry.register_language(
            Language::Japanese,
            LanguageMetadata {
                code: "ja",
                display_name: "Japanese",
                flag_emoji: "🇯🇵",
                supports_kanji: true,
                default_tts_voice: "voicevox:zundamon",
                default_asr_model: "qwen3_asr_rs:ja",
            }
        );
        
        // Register Spanish
        registry.register_language(
            Language::Spanish,
            LanguageMetadata {
                code: "es",
                display_name: "Spanish", 
                flag_emoji: "🇪🇸",
                supports_kanji: false,
                default_tts_voice: "piper:es_es",
                default_asr_model: "qwen3_asr_rs:es",
            }
        );
        
        registry
    }
    
    /// Register a new language
    pub fn register_language(&mut self, language: Language, metadata: LanguageMetadata) {
        self.available_languages.push((language, metadata));
    }
    
    /// Get all available languages
    pub fn get_available_languages(&self) -> Vec<Language> {
        self.available_languages.iter().map(|(lang, _)| *lang).collect()
    }
    
    /// Get metadata for a specific language
    pub fn get_metadata(&self, language: Language) -> Option<&LanguageMetadata> {
        self.available_languages
            .iter()
            .find(|(lang, _)| *lang == language)
            .map(|(_, metadata)| metadata)
    }
    
    /// Get language by code
    pub fn get_by_code(&self, code: &str) -> Option<Language> {
        self.available_languages
            .iter()
            .find(|(_, metadata)| metadata.code == code)
            .map(|(lang, _)| *lang)
    }
    
    /// Check if a language supports kanji
    pub fn supports_kanji(&self, language: Language) -> bool {
        self.get_metadata(language)
            .map(|m| m.supports_kanji)
            .unwrap_or(false)
    }
}

/// Manages the active language and coordinates language-specific features
pub struct LanguageManager {
    registry: LanguageRegistry,
    active_language: Language,
}

impl Default for LanguageManager {
    fn default() -> Self {
        Self::new(Language::Japanese)
    }
}

impl LanguageManager {
    /// Create a new language manager with the specified active language
    pub fn new(active_language: Language) -> Self {
        Self {
            registry: LanguageRegistry::new(),
            active_language,
        }
    }
    
    /// Get the currently active language
    pub fn get_active_language(&self) -> Language {
        self.active_language
    }
    
    /// Set the active language
    pub fn set_active_language(&mut self, language: Language) -> anyhow::Result<()> {
        // Validate that the language is available
        if self.registry.get_metadata(language).is_none() {
            anyhow::bail!("Language {} is not registered", language);
        }
        self.active_language = language;
        Ok(())
    }
    
    /// Get the language registry
    pub fn registry(&self) -> &LanguageRegistry {
        &self.registry
    }
    
    /// Get metadata for the active language
    pub fn get_active_metadata(&self) -> Option<&LanguageMetadata> {
        self.registry.get_metadata(self.active_language)
    }
    
    /// Get the language code for the active language
    pub fn get_active_code(&self) -> &'static str {
        self.active_language.code()
    }
    
    /// Get the display name for the active language
    pub fn get_active_display_name(&self) -> &'static str {
        self.active_language.display_name()
    }
    
    /// Check if the current language supports kanji
    pub fn supports_kanji(&self) -> bool {
        self.registry.supports_kanji(self.active_language)
    }
    
    /// Get appropriate resources for the active language
    pub fn get_tts_voice(&self) -> &'static str {
        self.get_active_metadata()
            .map(|m| m.default_tts_voice)
            .unwrap_or("default")
    }
    
    pub fn get_asr_model(&self) -> &'static str {
        self.get_active_metadata()
            .map(|m| m.default_asr_model)
            .unwrap_or("default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_code_and_display_name() {
        assert_eq!(Language::Japanese.code(), "ja");
        assert_eq!(Language::Japanese.display_name(), "Japanese");
        
        assert_eq!(Language::Spanish.code(), "es");
        assert_eq!(Language::Spanish.display_name(), "Spanish");
    }

    #[test]
    fn test_language_parse_display() {
        use strum::Display;
        assert_eq!(Language::Japanese.to_string(), "Japanese");
        assert_eq!(Language::Spanish.to_string(), "Spanish");
    }

    #[test]
    fn test_language_parse_from_string() {
        use strum::EnumString;
        assert_eq!("Japanese".parse::<Language>().unwrap(), Language::Japanese);
        assert_eq!("Japanese".parse::<Language>().unwrap(), Language::Japanese);
        assert_eq!("Spanish".parse::<Language>().unwrap(), Language::Spanish);
    }

    #[test]
    fn test_language_registry_creation() {
        let registry = LanguageRegistry::new();
        let available = registry.get_available_languages();
        
        assert!(available.contains(&Language::Japanese));
        assert!(available.contains(&Language::Spanish));
        assert_eq!(available.len(), 2);
    }

    #[test]
    fn test_language_registry_metadata() {
        let registry = LanguageRegistry::new();
        
        let ja_metadata = registry.get_metadata(Language::Japanese).unwrap();
        assert_eq!(ja_metadata.code, "ja");
        assert_eq!(ja_metadata.display_name, "Japanese");
        assert_eq!(ja_metadata.flag_emoji, "🇯🇵");
        assert!(ja_metadata.supports_kanji);
        
        let es_metadata = registry.get_metadata(Language::Spanish).unwrap();
        assert_eq!(es_metadata.code, "es");
        assert_eq!(es_metadata.display_name, "Spanish");
        assert_eq!(es_metadata.flag_emoji, "🇪🇸");
        assert!(!es_metadata.supports_kanji);
    }

    #[test]
    fn test_language_registry_by_code() {
        let registry = LanguageRegistry::new();
        
        assert_eq!(registry.get_by_code("ja"), Some(Language::Japanese));
        assert_eq!(registry.get_by_code("es"), Some(Language::Spanish));
        assert_eq!(registry.get_by_code("fr"), None);
    }

    #[test]
    fn test_language_registry_supports_kanji() {
        let registry = LanguageRegistry::new();
        
        assert!(registry.supports_kanji(Language::Japanese));
        assert!(!registry.supports_kanji(Language::Spanish));
    }

    #[test]
    fn test_language_manager_creation() {
        let manager = LanguageManager::new(Language::Japanese);
        assert_eq!(manager.get_active_language(), Language::Japanese);
        assert_eq!(manager.get_active_code(), "ja");
        assert_eq!(manager.get_active_display_name(), "Japanese");
        assert!(manager.supports_kanji());
    }

    #[test]
    fn test_language_manager_switching() {
        let mut manager = LanguageManager::new(Language::Japanese);
        assert_eq!(manager.get_active_language(), Language::Japanese);
        
        manager.set_active_language(Language::Spanish).unwrap();
        assert_eq!(manager.get_active_language(), Language::Spanish);
        assert_eq!(manager.get_active_code(), "es");
        assert!(!manager.supports_kanji());
    }

    #[test]
    fn test_language_manager_resources() {
        let manager = LanguageManager::new(Language::Japanese);
        assert!(manager.get_tts_voice().contains("voicevox"));
        assert!(manager.get_asr_model().contains("qwen3_asr_rs:ja"));
        
        let manager_es = LanguageManager::new(Language::Spanish);
        assert!(manager_es.get_tts_voice().contains("piper"));
        assert!(manager_es.get_asr_model().contains("qwen3_asr_rs:es"));
    }
}

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
