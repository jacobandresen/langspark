//! Application state: opens the SQLite database, wires up repositories and
//! the language manager, and loads the data each tab needs.
//!
//! Real-time language switching mid-session is an explicit non-goal (see
//! design.md), so `AppState` is built once per run for the language chosen
//! in Preferences at last save — switching languages takes effect on restart.

use langspark_core::{
    initialize_schema, Database, Language, LanguageManager, ReviewStats, SqliteDeckRepository, SqliteKanjiRepository,
    SqliteReviewRepository, SqliteSrsRepository, SqliteVocabularyRepository,
};
use std::path::Path;
use std::sync::Arc;

pub struct AppState {
    pub language_manager: LanguageManager,
    pub vocabulary_repo: SqliteVocabularyRepository,
    pub kanji_repo: SqliteKanjiRepository,
    pub srs_repo: SqliteSrsRepository,
    pub deck_repo: SqliteDeckRepository,
    pub review_repo: SqliteReviewRepository,
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
        let db = Arc::new(db);

        let language_manager = LanguageManager::new(active_language);
        let dictionary = load_dictionary(dict_dir, language_manager.get_active_code());

        Ok(Self {
            language_manager,
            vocabulary_repo: SqliteVocabularyRepository::new(db.clone()),
            kanji_repo: SqliteKanjiRepository::new(db.clone()),
            srs_repo: SqliteSrsRepository::new(db.clone()),
            deck_repo: SqliteDeckRepository::new(db.clone()),
            review_repo: SqliteReviewRepository::new(db),
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

        let history = self.review_repo.get_by_language(language)?;
        let ratings: Vec<u32> = history.iter().map(|r| r.rating).collect();
        let review_dates: Vec<String> = history
            .iter()
            .filter_map(|r| r.reviewed_at.as_ref())
            .map(|dt| dt.split(' ').next().unwrap_or(dt).to_string())
            .collect();
        let stats = langspark_core::build_review_stats(&ratings, &review_dates);

        let decks = self.deck_repo.get_by_language(language)?;
        let deck_stats: Vec<_> = decks
            .into_iter()
            .map(|deck| {
                let card_ids = self.deck_repo.card_ids(deck.id.unwrap_or(0)).unwrap_or_default();
                crate::statistics::compute_deck_stats(deck, &card_ids, &due_cards)
            })
            .collect();

        Ok(TabData { vocabulary, kanji, due_cards, stats, deck_stats })
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
        "es" => manager.load_spanish(&json, None),
        _ => Ok(()),
    };
    if let Err(e) = result {
        log::warn!("failed to parse dictionary at {}: {e}", path.display());
    }
    manager
}

/// Bundle of everything loaded from the database for populating tabs at startup.
pub struct TabData {
    pub vocabulary: Vec<langspark_core::VocabularyEntry>,
    pub kanji: Vec<langspark_core::KanjiEntry>,
    pub due_cards: Vec<langspark_core::SrsCard>,
    pub stats: ReviewStats,
    pub deck_stats: Vec<crate::statistics::DeckStats>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_creates_database_and_loads_empty_tab_data() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("nested").join("langspark.db");

        let state = AppState::open(&db_path, Language::Japanese, None).unwrap();
        assert!(db_path.exists());

        let data = state.load_tab_data().unwrap();
        assert!(data.vocabulary.is_empty());
        assert!(data.kanji.is_empty());
        assert!(data.due_cards.is_empty());
        assert_eq!(data.stats.total_reviews, 0);
        assert!(!state.dictionary.is_loaded("ja"));
    }

    #[test]
    fn test_open_respects_kanji_support() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::open(&dir.path().join("langspark.db"), Language::Spanish, None).unwrap();
        assert!(!state.language_manager.supports_kanji());
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
