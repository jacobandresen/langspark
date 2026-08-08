//! Language management module
//!
//! Handles language configuration, switching, and resource management.

use anyhow;

/// Supported languages (Japanese only, for now).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
pub enum Language {
    /// Japanese
    Japanese,
}

/// Language-specific behavior shared by every supported language.
///
/// Kept as a trait (rather than only inherent methods) so language-agnostic
/// code can be written against `dyn LanguageInfo` / generic bounds instead of
/// matching on the `Language` enum directly.
pub trait LanguageInfo {
    /// Get the language code (e.g., "ja" for Japanese)
    fn code(&self) -> &'static str;
    /// Get the display name
    fn display_name(&self) -> &'static str;
}

impl LanguageInfo for Language {
    fn code(&self) -> &'static str {
        Language::code(self)
    }

    fn display_name(&self) -> &'static str {
        Language::display_name(self)
    }
}

/// Installation status of a language's optional resources (dictionary, TTS/ASR models).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationStatus {
    /// Nothing has been downloaded yet
    NotInstalled,
    /// Download/setup is in progress
    Installing,
    /// All required resources are present and usable
    Installed,
    /// Installation was attempted but failed
    Failed,
}

/// Metadata for a language including its resources and capabilities
#[derive(Debug, Clone)]
pub struct LanguageMetadata {
    /// Language code (e.g., "ja")
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
    installation_status: std::collections::HashMap<Language, InstallationStatus>,
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
            installation_status: std::collections::HashMap::new(),
        }
    }

    /// Get the installation status of a language (defaults to `NotInstalled`)
    pub fn get_installation_status(&self, language: Language) -> InstallationStatus {
        self.installation_status
            .get(&language)
            .copied()
            .unwrap_or(InstallationStatus::NotInstalled)
    }

    /// Record the installation status of a language
    pub fn set_installation_status(&mut self, language: Language, status: InstallationStatus) {
        self.installation_status.insert(language, status);
    }

    /// Whether the active language's resources are fully installed
    pub fn is_active_language_installed(&self) -> bool {
        self.get_installation_status(self.active_language) == InstallationStatus::Installed
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
    }

    #[test]
    fn test_language_parse_display() {
        assert_eq!(Language::Japanese.to_string(), "Japanese");
    }

    #[test]
    fn test_language_parse_from_string() {
        assert_eq!("Japanese".parse::<Language>().unwrap(), Language::Japanese);
        assert!("Spanish".parse::<Language>().is_err());
    }

    #[test]
    fn test_language_registry_creation() {
        let registry = LanguageRegistry::new();
        let available = registry.get_available_languages();

        assert_eq!(available, vec![Language::Japanese]);
    }

    #[test]
    fn test_language_registry_metadata() {
        let registry = LanguageRegistry::new();

        let ja_metadata = registry.get_metadata(Language::Japanese).unwrap();
        assert_eq!(ja_metadata.code, "ja");
        assert_eq!(ja_metadata.display_name, "Japanese");
        assert_eq!(ja_metadata.flag_emoji, "🇯🇵");
        assert!(ja_metadata.supports_kanji);
    }

    #[test]
    fn test_language_registry_by_code() {
        let registry = LanguageRegistry::new();

        assert_eq!(registry.get_by_code("ja"), Some(Language::Japanese));
        assert_eq!(registry.get_by_code("fr"), None);
    }

    #[test]
    fn test_language_registry_supports_kanji() {
        let registry = LanguageRegistry::new();
        assert!(registry.supports_kanji(Language::Japanese));
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
    fn test_language_info_trait() {
        fn code_via_trait(l: &dyn LanguageInfo) -> &'static str {
            l.code()
        }
        assert_eq!(code_via_trait(&Language::Japanese), "ja");
    }

    #[test]
    fn test_installation_status_tracking() {
        let mut manager = LanguageManager::new(Language::Japanese);
        assert_eq!(
            manager.get_installation_status(Language::Japanese),
            InstallationStatus::NotInstalled
        );
        assert!(!manager.is_active_language_installed());

        manager.set_installation_status(Language::Japanese, InstallationStatus::Installed);
        assert_eq!(
            manager.get_installation_status(Language::Japanese),
            InstallationStatus::Installed
        );
        assert!(manager.is_active_language_installed());
    }

    #[test]
    fn test_language_manager_resources() {
        let manager = LanguageManager::new(Language::Japanese);
        assert!(manager.get_tts_voice().contains("voicevox"));
        assert!(manager.get_asr_model().contains("qwen3_asr_rs:ja"));
    }
}

impl Language {
    /// Get the language code (e.g., "ja" for Japanese)
    pub fn code(&self) -> &'static str {
        match self {
            Language::Japanese => "ja",
        }
    }

    /// Get the display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::Japanese => "Japanese",
        }
    }
}
