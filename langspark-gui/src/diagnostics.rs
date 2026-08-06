//! Startup dependency checks and error-reporting UI helpers.

use langspark_core::Language;
use std::path::Path;

/// A single startup issue found by [`check_dependencies`] — missing hardware
/// or an uninstalled language resource. Non-fatal: the app should still
/// launch and show these as warnings (graceful degradation, per design.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyIssue {
    pub message: String,
}

/// Check for common missing dependencies before the user hits them mid-task:
/// audio hardware, and whether `language`'s dictionary file exists under `dict_dir`.
pub fn check_dependencies(language: Language, dict_dir: &Path) -> Vec<DependencyIssue> {
    let mut issues = Vec::new();

    let (has_input, has_output) = langspark_core::audio_devices_available();
    if !has_input {
        issues.push(DependencyIssue {
            message: "No microphone detected — pronunciation recording won't work.".to_string(),
        });
    }
    if !has_output {
        issues.push(DependencyIssue {
            message: "No audio output device detected — pronunciation playback won't work.".to_string(),
        });
    }

    let expected_dict = dict_dir.join(format!("{}.json", language.code()));
    if !expected_dict.exists() {
        issues.push(DependencyIssue {
            message: format!(
                "{} dictionary isn't installed yet ({} not found). Install it from Preferences.",
                language.display_name(),
                expected_dict.display()
            ),
        });
    }

    issues
}

/// Show a dismissible error toast in a window that has an `adw::ToastOverlay`
/// (see `app::build_main_window`), per task 22.3.
pub fn show_error_toast(overlay: &adw::ToastOverlay, message: &str) {
    let toast = adw::Toast::builder().title(message).timeout(5).build();
    overlay.add_toast(toast);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_dependencies_flags_missing_dictionary() {
        let dir = tempfile::tempdir().unwrap();
        let issues = check_dependencies(Language::Japanese, dir.path());
        assert!(issues.iter().any(|i| i.message.contains("dictionary isn't installed")));
    }

    #[test]
    fn test_check_dependencies_dictionary_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ja.json"), "{}").unwrap();
        let issues = check_dependencies(Language::Japanese, dir.path());
        assert!(!issues.iter().any(|i| i.message.contains("dictionary isn't installed")));
    }
}
