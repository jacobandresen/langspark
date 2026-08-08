//! Typed error categories for cases callers (particularly the GUI) need to
//! distinguish and react to differently — e.g. showing an "install this
//! language's voice model" prompt instead of a generic failure dialog.
//!
//! Most of `langspark-core`'s functions still return `anyhow::Result` for
//! ergonomics; this type exists for the subset of call sites where the
//! caller's behavior actually branches on *why* something failed.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LangSparkError {
    #[error("dictionary error: {0}")]
    Dictionary(String),

    #[error("database error: {0}")]
    Database(String),

    #[error("audio error: {0}")]
    Audio(String),

    /// A required resource (dictionary file, voice model, ASR model) hasn't
    /// been installed for `language` yet.
    #[error("{resource} is not installed for {language}")]
    MissingResource { language: String, resource: String },

    #[error("{0}")]
    Other(String),
}

impl LangSparkError {
    /// A short, non-technical message suitable for displaying directly to a user
    /// (e.g. in a toast notification), as opposed to `Display`'s more detailed form.
    pub fn user_message(&self) -> String {
        match self {
            LangSparkError::Dictionary(_) => "There was a problem loading the dictionary.".to_string(),
            LangSparkError::Database(_) => "There was a problem accessing your saved data.".to_string(),
            LangSparkError::Audio(_) => "There was a problem with audio playback or recording.".to_string(),
            LangSparkError::MissingResource { language, resource } => {
                format!("{resource} for {language} isn't installed yet. Install it from Preferences.")
            }
            LangSparkError::Other(msg) => msg.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_resource_user_message() {
        let err = LangSparkError::MissingResource {
            language: "Japanese".to_string(),
            resource: "TTS voice".to_string(),
        };
        assert!(err.user_message().contains("Japanese"));
        assert!(err.user_message().contains("Install it from Preferences"));
    }

    #[test]
    fn test_display_vs_user_message() {
        let err = LangSparkError::Dictionary("failed to parse JSON at line 4".to_string());
        assert!(err.to_string().contains("failed to parse JSON"));
        assert_eq!(err.user_message(), "There was a problem loading the dictionary.");
    }
}
