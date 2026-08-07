//! LangSpark GTK4 Application
//!
//! Main application entry point. Initializes GTK, sets up the application window,
//! and coordinates between UI components and core logic.

use adw::Application as AdwApplication;
use gtk4::prelude::*;

mod app;
mod config;
mod diagnostics;
mod kanji;
mod preferences;
mod pronunciation;
mod review;
mod state;
mod statistics;
mod task;
mod ui;
mod vocabulary;
mod widgets;

use config::{AppDirs, Settings};
use langspark_core::Language;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

fn main() -> glib::ExitCode {
    // Initialize logging
    env_logger::init();

    // Create the application
    let app = AdwApplication::builder()
        .application_id("org.langspark.LangSpark")
        .build();

    app.connect_activate(|app| {
        ui::load_styles();

        // Load settings (falling back to defaults) and resolve the active language
        let dirs = AppDirs::new();
        let settings = dirs
            .as_ref()
            .and_then(|d| Settings::load(&d.config_file()).ok())
            .unwrap_or_default();
        let active_language: Language = settings.active_language.parse().unwrap_or(Language::Japanese);
        // Honor a custom dictionary_data_dir override (also respected by the
        // Preferences installer, see preferences.rs), falling back to the
        // default XDG dictionaries dir.
        let dict_dir = settings.dictionary_data_dir.clone().or_else(|| dirs.as_ref().map(|d| d.dictionaries_dir()));
        let settings = Rc::new(RefCell::new(settings));

        // Open the database, falling back to an in-memory one if the real
        // path can't be created (e.g. no writable home directory) so the app
        // still starts, just without persistence.
        let db_path = dirs.as_ref().map(|d| d.database_file()).unwrap_or_else(|| PathBuf::from(":memory:"));
        let state = state::AppState::open(&db_path, active_language, dict_dir.as_deref())
            .or_else(|e| {
                log::warn!("failed to open database at {}: {e}; falling back to in-memory", db_path.display());
                state::AppState::open(std::path::Path::new(":memory:"), active_language, dict_dir.as_deref())
            })
            .expect("failed to open even an in-memory database");
        let state = Arc::new(state);

        // Create the main window
        let (window, toast_overlay) = app::build_main_window(app, active_language, settings, state);

        // Run startup dependency checks
        if let Some(dirs) = &dirs {
            for issue in diagnostics::check_dependencies(active_language, &dirs.dictionaries_dir()) {
                diagnostics::show_error_toast(&toast_overlay, &issue.message);
            }
        }

        window.present();
    });

    // Run the application
    app.run()
}

/// Consolidated GTK widget-construction smoke test.
///
/// GTK can only be initialized from one OS thread per process, but the
/// `#[test]` harness spawns a fresh thread per test function — so as soon as
/// two different `#[test]`s each called `gtk4::init()`, the second panicked
/// with "Attempted to initialize GTK from two different threads" (this bit
/// us: see git history). Building one instance of every tab/dialog/widget
/// here, in a single test, is the only reliable way to smoke-test widget
/// construction under `cargo test`. Skips (rather than fails) when no
/// display is available, e.g. headless CI without Xvfb.
#[cfg(test)]
mod gtk_smoke {
    use crate::{app, kanji, preferences, pronunciation, review, statistics, vocabulary, widgets};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::Arc;

    #[test]
    fn test_every_widget_builds() {
        if gtk4::init().is_err() {
            return;
        }

        let temp_db = tempfile::NamedTempFile::new().unwrap();
        let state = Arc::new(
            crate::state::AppState::open(temp_db.path(), langspark_core::Language::Japanese, None).unwrap(),
        );
        let app = adw::Application::builder().application_id("org.langspark.LangSparkTest").build();
        let settings = Rc::new(RefCell::new(crate::config::Settings::default()));
        let _main_window = app::build_main_window(&app, langspark_core::Language::Japanese, settings, state);

        let vocab_entry = vocabulary::dialog::tests::sample_entry();
        let _vocab_dialog = vocabulary::dialog::build(&vocab_entry, &[], vocabulary::dialog::tests::noop_callbacks());
        let _vocab_tab = vocabulary::build_tab(
            &[vocab_entry],
            vocabulary::VocabTabCallbacks {
                add_word: None,
                on_play: None,
                delete: std::rc::Rc::new(|_, _, _| {}),
                example_lookup: None,
            },
        );

        let kanji_entry = kanji::dialog::tests::sample_entry();
        let _kanji_dialog = kanji::dialog::build(&kanji_entry);
        let _kanji_tab = kanji::build_tab(&[kanji_entry]);

        let review_items = vec![review::tests::sample_item("front")];
        let _review_session = review::ReviewSession::new(review_items, |_, _| {});

        let words = vec![pronunciation::PracticeWord { text: "受け取る".to_string(), reading: None }];
        let _pronunciation_tab = pronunciation::PronunciationTab::new(words, pronunciation::tests::noop_callbacks());

        let stats = langspark_core::ReviewStats::default();
        let _stats_tab = statistics::build_tab(&stats, &[], &[], &[]);

        let _waveform = widgets::waveform::Waveform::new();

        let settings = Rc::new(RefCell::new(crate::config::Settings::default()));
        let _prefs_dialog = preferences::build(settings, |_| {});

        let registry = langspark_core::LanguageRegistry::new();
        let _selector = widgets::language_selector::LanguageSelector::new(&registry, langspark_core::Language::Japanese);
    }
}
