//! Application state: opens the SQLite database, wires up repositories and
//! the language manager, and loads the data each tab needs.
//!
//! Real-time language switching mid-session is an explicit non-goal (see
//! design.md), so `AppState` is built once per run for the language chosen
//! in Preferences at last save — switching languages takes effect on restart.

use langspark_core::{
    default_migrations, initialize_schema, run_migrations, Database, Language, LanguageManager,
    SqliteKanjiRepository, SqliteSrsRepository, SqliteVocabularyRepository,
};
use std::path::Path;
use std::sync::Arc;

pub struct AppState {
    pub language_manager: LanguageManager,
    pub vocabulary_repo: SqliteVocabularyRepository,
    pub kanji_repo: SqliteKanjiRepository,
    pub srs_repo: SqliteSrsRepository,
    /// Dictionary for the active language, loaded from `dict_dir` (see
    /// `AppDirs::dictionaries_dir`) if a matching `<code>.json` file exists.
    /// Empty (nothing loaded) if no dictionary has been installed yet.
    pub dictionary: langspark_core::DictionaryManager,
}

impl AppState {
    /// Open (creating if necessary) the database at `db_path` and wire up
    /// every repository for `active_language`. If `dict_dir` is given and
    /// contains `<active_language code>.json`, the dictionary is loaded too.
    pub fn open(db_path: &Path, active_language: Language, dict_dir: Option<&Path>) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Database::open(db_path)?;
        initialize_schema(&db.conn())?;
        run_migrations(&mut db.conn(), &default_migrations())?;

        // First-run seed: an empty Japanese vocabulary table (a brand new
        // database, or an existing one where the user simply hasn't added
        // Japanese words yet) gets the standard JLPT N5-N3 school vocabulary
        // pre-populated, each with a ready-to-review SRS card, so the
        // Vocabulary/Review tabs aren't empty on first launch. Only runs
        // once per database: after this, the table is no longer empty.
        if active_language == Language::Japanese {
            let ja_count: i64 =
                db.conn().query_row("SELECT COUNT(*) FROM vocabulary WHERE language = 'ja'", [], |r| r.get(0))?;
            if ja_count == 0 {
                match langspark_core::seed_ja_school_vocabulary(&db) {
                    Ok(n) => log::info!("seeded {n} Japanese school vocabulary words on first run"),
                    Err(e) => log::warn!("failed to seed Japanese school vocabulary: {e}"),
                }
            }
        }

        let db = Arc::new(db);

        let language_manager = LanguageManager::new(active_language);
        let dictionary = load_dictionary(dict_dir, language_manager.get_active_code());

        Ok(Self {
            language_manager,
            vocabulary_repo: SqliteVocabularyRepository::new(db.clone()),
            kanji_repo: SqliteKanjiRepository::new(db.clone()),
            srs_repo: SqliteSrsRepository::new(db),
            dictionary,
        })
    }

    fn active_code(&self) -> &'static str {
        self.language_manager.get_active_code()
    }

    /// Everything the tabs need for the active language, loaded in one shot.
    /// Intended to run inside `task::run_blocking` since it does DB I/O.
    pub fn load_tab_data(&self) -> anyhow::Result<TabData> {
        let language = self.active_code();

        let vocabulary = self.vocabulary_repo.get_by_language(language)?;
        let kanji = if self.language_manager.supports_kanji() {
            self.kanji_repo.search("")?.into_iter().filter(|k| k.language == language).collect()
        } else {
            Vec::new()
        };
        let due_cards = self.srs_repo.get_due_cards(language)?;

        Ok(TabData { vocabulary, kanji, due_cards })
    }
}

/// Load the dictionary for `language_code` from `<dict_dir>/<language_code>.json`,
/// if present. Missing file or parse failure just leaves the dictionary empty
/// (the "Add Word" UI stays disabled) rather than failing startup.
fn load_dictionary(dict_dir: Option<&Path>, language_code: &str) -> langspark_core::DictionaryManager {
    let mut manager = langspark_core::DictionaryManager::new();
    let Some(dir) = dict_dir else { return manager };
    let path = dir.join(format!("{language_code}.json"));
    let Ok(json) = std::fs::read_to_string(&path) else { return manager };

    let result = match language_code {
        "ja" => manager.load_japanese(&json, None),
        _ => Ok(()),
    };
    if let Err(e) = result {
        log::warn!("failed to parse dictionary at {}: {e}", path.display());
    }

    // Supplemental Tatoeba example sentences (see `installer::install_tatoeba_examples`),
    // for the many words JMdict's own much smaller curated example subset
    // doesn't cover. Optional — silently absent until installed from Preferences.
    let tatoeba_path = dir.join(format!("tatoeba_{language_code}.tsv"));
    if let Ok(tsv) = std::fs::read_to_string(&tatoeba_path) {
        manager.load_tatoeba_examples(language_code, &tsv);
    }

    manager
}

/// Bundle of everything loaded from the database for populating tabs at startup.
pub struct TabData {
    pub vocabulary: Vec<langspark_core::VocabularyEntry>,
    pub kanji: Vec<langspark_core::KanjiEntry>,
    pub due_cards: Vec<langspark_core::SrsCard>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_seeds_japanese_school_vocabulary_on_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nested").join("langspark.db");

        let state = AppState::open(&db_path, Language::Japanese, None).unwrap();
        assert!(db_path.exists());

        let data = state.load_tab_data().unwrap();
        // First-run seed: a fresh Japanese database gets the standard JLPT
        // N5-N3 school vocabulary pre-populated, each with a due SRS card
        // (see AppState::open / langspark_core::seed_ja_school_vocabulary).
        let expected = langspark_core::ja_school_vocabulary_len();
        assert_eq!(data.vocabulary.len(), expected);
        assert_eq!(data.due_cards.len(), expected);
        assert!(data.kanji.is_empty());
        assert!(!state.dictionary.is_loaded("ja"));

        // Re-opening the same database must not seed a second time.
        drop(state);
        let state = AppState::open(&db_path, Language::Japanese, None).unwrap();
        assert_eq!(state.load_tab_data().unwrap().vocabulary.len(), expected);
    }

    #[test]
    fn test_open_respects_kanji_support() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::open(&dir.path().join("langspark.db"), Language::Japanese, None).unwrap();
        assert!(state.language_manager.supports_kanji());
    }

    #[test]
    fn test_open_loads_dictionary_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let dict_dir = dir.path().join("dictionaries");
        std::fs::create_dir_all(&dict_dir).unwrap();
        std::fs::write(
            dict_dir.join("ja.json"),
            r#"{"words": [{"id": "1", "kanji": [], "kana": [{"text": "たべる"}], "sense": [{"gloss": [{"lang": "eng", "text": "to eat"}]}]}]}"#,
        )
        .unwrap();

        let state = AppState::open(&dir.path().join("langspark.db"), Language::Japanese, Some(&dict_dir)).unwrap();
        assert!(state.dictionary.is_loaded("ja"));
        assert_eq!(state.dictionary.search("ja", "eat").len(), 1);
    }
}
