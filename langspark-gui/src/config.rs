//! Application settings: persisted user preferences (TOML, XDG config dir),
//! with environment-variable overrides applied on load.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// All user-configurable application settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Active language code ("ja" — the only supported language for now)
    pub active_language: String,
    /// Override for where dictionary JSON files are read from
    pub dictionary_data_dir: Option<PathBuf>,
    /// Override for where the book catalog and cached book text are read from
    pub books_data_dir: Option<PathBuf>,
    /// VOICEVOX speaker/style ID string for Japanese TTS
    pub tts_voice_ja: String,
    /// Pronunciation (TTS) speaking speed, 1 (slowest) to 5 (normal engine
    /// speed, the default) — see `app.rs`'s `tts_speed_to_speed_scale` for
    /// how this maps onto VOICEVOX's own `speedScale` parameter.
    pub tts_speed: u8,
    /// "sm2" or "fsrs"
    pub srs_algorithm: String,
    /// Initial ease factor assigned to newly-created SRS cards (SM-2 range
    /// 1.3-3.0; ignored by FSRS, which derives its own initial difficulty).
    pub starting_ease_factor: f64,
    /// CPAL input device name, or None for the system default
    pub audio_input_device: Option<String>,
    /// CPAL/rodio output device name, or None for the system default
    pub audio_output_device: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            active_language: "ja".to_string(),
            dictionary_data_dir: None,
            books_data_dir: None,
            tts_voice_ja: "zundamon".to_string(),
            tts_speed: 5,
            srs_algorithm: "sm2".to_string(),
            starting_ease_factor: 2.5,
            audio_input_device: None,
            audio_output_device: None,
        }
    }
}

impl Settings {
    /// Load settings from `path`, falling back to defaults if the file
    /// doesn't exist, then apply any `LANGSPARK_*` environment overrides.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let mut settings = if path.exists() {
            let text = std::fs::read_to_string(path)?;
            toml::from_str(&text)?
        } else {
            Self::default()
        };
        settings.apply_env_overrides();
        Ok(settings)
    }

    /// Save settings to `path` as TOML, creating parent directories as needed.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Apply `LANGSPARK_*` environment variable overrides on top of loaded
    /// values, so e.g. `LANGSPARK_TTS_VOICE_JA=some-other-speaker` wins over the file.
    pub fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("LANGSPARK_ACTIVE_LANGUAGE") {
            self.active_language = v;
        }
        if let Ok(v) = std::env::var("LANGSPARK_DICTIONARY_DATA_DIR") {
            self.dictionary_data_dir = Some(PathBuf::from(v));
        }
        if let Ok(v) = std::env::var("LANGSPARK_BOOKS_DATA_DIR") {
            self.books_data_dir = Some(PathBuf::from(v));
        }
        if let Ok(v) = std::env::var("LANGSPARK_TTS_VOICE_JA") {
            self.tts_voice_ja = v;
        }
        if let Ok(v) = std::env::var("LANGSPARK_TTS_SPEED") {
            if let Ok(speed) = v.parse::<u8>() {
                self.tts_speed = speed.clamp(1, 5);
            }
        }
        if let Ok(v) = std::env::var("LANGSPARK_SRS_ALGORITHM") {
            self.srs_algorithm = v;
        }
    }
}

/// XDG-standard application directories (config, data, cache) — see
/// README.md's "Project Structure" section for the resulting paths.
pub struct AppDirs {
    dirs: ProjectDirs,
}

impl AppDirs {
    /// Resolve the standard LangSpark directories for this platform.
    /// Returns `None` if no valid home directory could be determined.
    pub fn new() -> Option<Self> {
        ProjectDirs::from("", "", "langspark").map(|dirs| Self { dirs })
    }

    pub fn config_file(&self) -> PathBuf {
        self.dirs.config_dir().join("config.toml")
    }

    pub fn database_file(&self) -> PathBuf {
        self.dirs.data_dir().join("langspark.db")
    }

    pub fn dictionaries_dir(&self) -> PathBuf {
        self.dirs.data_dir().join("dictionaries")
    }

    /// Where the Aozora Bunko book catalog (`catalog.json`) and each opened
    /// book's parsed/cached text (`<work id>.json`) live — see
    /// `langspark_core::install_aozora_catalog`/`fetch_book`.
    pub fn books_dir(&self) -> PathBuf {
        self.dirs.data_dir().join("books")
    }

    pub fn audio_cache_dir(&self) -> PathBuf {
        self.dirs.cache_dir().join("audio")
    }

    /// Where the Helsinki-NLP OPUS-MT ja-en translation model
    /// (`config.json`, `pytorch_model.bin`, `source.spm`, `target.spm`)
    /// lives — see `langspark_core::install_translation_model`.
    pub fn translation_model_dir(&self) -> PathBuf {
        self.dirs.data_dir().join("translation_model")
    }

    /// Where translated paragraphs are cached, keyed by a hash of the
    /// source text — see `langspark_core::TranslationCache`.
    pub fn translation_cache_dir(&self) -> PathBuf {
        self.dirs.cache_dir().join("translations")
    }

    /// Where a `qwen3` ASR model directory (`config.json`, `model.safetensors`,
    /// `tokenizer.json`) is expected for `language_code`, e.g. `asr/ja/`.
    /// There's no automated installer for these yet (unlike the Japanese
    /// dictionary — see `installer.rs`), so this only matters once one has
    /// been placed there manually and the `asr` Cargo feature is enabled.
    pub fn asr_model_dir(&self, language_code: &str) -> PathBuf {
        self.dirs.data_dir().join("asr").join(language_code)
    }

    /// Where a native VOICEVOX Engine install (a `run` executable plus its
    /// bundled model/library files) lives — see
    /// `langspark_core::install_voicevox_engine`.
    pub fn voicevox_engine_dir(&self) -> PathBuf {
        self.dirs.data_dir().join("voicevox_engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// `Settings::load` reads process-wide env vars, and `cargo test` runs
    /// tests in parallel threads sharing that env — without serializing,
    /// `test_env_override_wins_over_file`'s `LANGSPARK_ACTIVE_LANGUAGE` can
    /// leak into a concurrently-running test that also calls `load`.
    static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_settings_default() {
        let settings = Settings::default();
        assert_eq!(settings.active_language, "ja");
        assert_eq!(settings.srs_algorithm, "sm2");
    }

    #[test]
    fn test_settings_save_and_load_roundtrip() {
        let _guard = ENV_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut settings = Settings::default();
        settings.active_language = "es".to_string();
        settings.save(&path).unwrap();

        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.active_language, "es");
    }

    #[test]
    fn test_settings_load_missing_file_uses_defaults() {
        let _guard = ENV_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.toml");

        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn test_env_override_wins_over_file() {
        let _guard = ENV_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        Settings::default().save(&path).unwrap();

        // SAFETY: test-only; ENV_TEST_LOCK serializes every test that reads
        // or writes LANGSPARK_* env vars so this doesn't race with them.
        unsafe {
            std::env::set_var("LANGSPARK_ACTIVE_LANGUAGE", "es");
        }
        let loaded = Settings::load(&path).unwrap();
        unsafe {
            std::env::remove_var("LANGSPARK_ACTIVE_LANGUAGE");
        }
        assert_eq!(loaded.active_language, "es");
    }

    #[test]
    fn test_app_dirs_paths() {
        if let Some(dirs) = AppDirs::new() {
            assert!(dirs.config_file().ends_with("config.toml"));
            assert!(dirs.database_file().ends_with("langspark.db"));
            assert!(dirs.dictionaries_dir().ends_with("dictionaries"));
            assert!(dirs.audio_cache_dir().ends_with("audio"));
            assert!(dirs.asr_model_dir("ja").ends_with("asr/ja"));
            assert!(dirs.books_dir().ends_with("books"));
            assert!(dirs.translation_model_dir().ends_with("translation_model"));
            assert!(dirs.translation_cache_dir().ends_with("translations"));
        }
    }

    #[test]
    fn test_books_data_dir_save_and_load_roundtrip() {
        let _guard = ENV_TEST_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let mut settings = Settings::default();
        settings.books_data_dir = Some(PathBuf::from("/custom/books"));
        settings.save(&path).unwrap();

        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.books_data_dir, Some(PathBuf::from("/custom/books")));
    }
}
