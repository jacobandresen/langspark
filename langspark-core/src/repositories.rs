//! Repository implementations
//!
//! Concrete implementations of the Repository trait for SQLite.

use crate::database::Database;
use crate::srs::{CardState, SrsBackend, SrsCard, SM2Backend};
use anyhow::Result;
use rusqlite::{params, Row, Error as RusqliteError, types::Type};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Rating for SRS (1=Again, 2=Hard, 3=Good, 4=Easy)
pub type SrsRating = u32;

/// Type alias for rusqlite Result for from_row methods
type RResult<T> = std::result::Result<T, RusqliteError>;

/// Vocabulary entry with full fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VocabularyEntry {
    pub id: Option<i64>,
    pub word: String,
    pub reading: Option<String>,
    pub meaning: String,
    pub language: String,
    pub level: Option<String>,
    pub part_of_speech: Option<String>,
    pub tags: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl VocabularyEntry {
    fn from_row(row: &Row) -> RResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            word: row.get(1)?,
            reading: row.get(2)?,
            meaning: row.get(3)?,
            language: row.get(4)?,
            level: row.get(5)?,
            part_of_speech: row.get(6)?,
            tags: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    }
}

/// SQLite repository for vocabulary operations
pub struct SqliteVocabularyRepository {
    db: Arc<Database>,
}

impl SqliteVocabularyRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
    
    /// Filter by language
    pub fn get_by_language(&self, language: &str) -> Result<Vec<VocabularyEntry>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, word, reading, meaning, language, level, part_of_speech, tags, created_at, updated_at 
             FROM vocabulary 
             WHERE language = ? 
             ORDER BY word",
        )?;
        
        let entries = stmt.query_map(params![language], VocabularyEntry::from_row)?
            .collect::<RResult<Vec<_>>>()
            .map_err(|e| anyhow::Error::new(e))?;
        
        Ok(entries)
    }
    
    /// Create a new vocabulary entry, returning its assigned ID
    pub fn create(&self, entry: &VocabularyEntry) -> Result<i64> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO vocabulary (word, reading, meaning, language, level, part_of_speech, tags)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                entry.word,
                entry.reading,
                entry.meaning,
                entry.language,
                entry.level,
                entry.part_of_speech,
                entry.tags,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Fetch a single entry by ID
    pub fn get_by_id(&self, id: i64) -> Result<Option<VocabularyEntry>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, word, reading, meaning, language, level, part_of_speech, tags, created_at, updated_at
             FROM vocabulary WHERE id = ?",
        )?;
        match stmt.query_row(params![id], VocabularyEntry::from_row) {
            Ok(entry) => Ok(Some(entry)),
            Err(RusqliteError::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::Error::new(e)),
        }
    }

    /// Update an existing entry's editable fields and bump `updated_at`
    pub fn update(&self, id: i64, entry: &VocabularyEntry) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE vocabulary
             SET word = ?, reading = ?, meaning = ?, level = ?, part_of_speech = ?, tags = ?, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
            params![
                entry.word,
                entry.reading,
                entry.meaning,
                entry.level,
                entry.part_of_speech,
                entry.tags,
                id,
            ],
        )?;
        Ok(())
    }

    /// Delete an entry and any SRS cards referencing it
    pub fn delete(&self, id: i64) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM srs_cards WHERE vocab_id = ?", params![id])?;
        conn.execute("DELETE FROM vocabulary WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Search by word, reading, or meaning
    pub fn search(&self, query: &str, language: Option<&str>) -> Result<Vec<VocabularyEntry>> {
        let conn = self.db.conn();
        let query_pattern = format!("%{}%", query);
        let lang_param: Box<dyn rusqlite::ToSql> = language.map(|s| Box::new(s.to_string()) as Box<dyn rusqlite::ToSql>).unwrap_or_else(|| Box::new(""));
        
        let sql = if language.is_some() {
            "SELECT id, word, reading, meaning, language, level, part_of_speech, tags, created_at, updated_at 
             FROM vocabulary 
             WHERE language = ? AND (word LIKE ? OR reading LIKE ? OR meaning LIKE ?) 
             ORDER BY word"
        } else {
            "SELECT id, word, reading, meaning, language, level, part_of_speech, tags, created_at, updated_at 
             FROM vocabulary 
             WHERE word LIKE ? OR reading LIKE ? OR meaning LIKE ? 
             ORDER BY word"
        };
        
        let mut stmt = conn.prepare(sql)?;
        
        let params: Vec<Box<dyn rusqlite::ToSql>> = if language.is_some() {
            vec![
                lang_param,
                Box::new(query_pattern.clone()),
                Box::new(query_pattern.clone()),
                Box::new(query_pattern),
            ]
        } else {
            vec![
                Box::new(query_pattern.clone()),
                Box::new(query_pattern.clone()),
                Box::new(query_pattern),
            ]
        };
        
        let entries = stmt.query_map(rusqlite::params_from_iter(params), VocabularyEntry::from_row)?
            .collect::<RResult<Vec<_>>>()
            .map_err(|e| anyhow::Error::new(e))?;
        
        Ok(entries)
    }
}

/// Kanji entry with full fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanjiEntry {
    pub id: Option<i64>,
    pub character: String,
    pub on_readings: Option<String>,
    pub kun_readings: Option<String>,
    pub meanings: String,
    pub stroke_count: Option<i32>,
    pub radical: Option<String>,
    pub jlpt_level: Option<i32>,
    pub grade: Option<i32>,
    pub language: String,
    pub created_at: Option<String>,
}

impl KanjiEntry {
    fn from_row(row: &Row) -> RResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            character: row.get(1)?,
            on_readings: row.get(2)?,
            kun_readings: row.get(3)?,
            meanings: row.get(4)?,
            stroke_count: row.get(5)?,
            radical: row.get(6)?,
            jlpt_level: row.get(7)?,
            grade: row.get(8)?,
            language: row.get(9)?,
            created_at: row.get(10)?,
        })
    }
}

/// SQLite repository for kanji operations
pub struct SqliteKanjiRepository {
    db: Arc<Database>,
}

impl SqliteKanjiRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
    
    /// Create a new kanji entry, returning its assigned ID
    pub fn create(&self, entry: &KanjiEntry) -> Result<i64> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO kanji (character, on_readings, kun_readings, meanings, stroke_count, radical, jlpt_level, grade, language)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                entry.character,
                entry.on_readings,
                entry.kun_readings,
                entry.meanings,
                entry.stroke_count,
                entry.radical,
                entry.jlpt_level,
                entry.grade,
                entry.language,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Fetch a single entry by ID
    pub fn get_by_id(&self, id: i64) -> Result<Option<KanjiEntry>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, character, on_readings, kun_readings, meanings, stroke_count, radical, jlpt_level, grade, language, created_at
             FROM kanji WHERE id = ?",
        )?;
        match stmt.query_row(params![id], KanjiEntry::from_row) {
            Ok(entry) => Ok(Some(entry)),
            Err(RusqliteError::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::Error::new(e)),
        }
    }

    /// Update an existing kanji entry
    pub fn update(&self, id: i64, entry: &KanjiEntry) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE kanji SET on_readings = ?, kun_readings = ?, meanings = ?, stroke_count = ?, radical = ?, jlpt_level = ?, grade = ?
             WHERE id = ?",
            params![
                entry.on_readings,
                entry.kun_readings,
                entry.meanings,
                entry.stroke_count,
                entry.radical,
                entry.jlpt_level,
                entry.grade,
                id,
            ],
        )?;
        Ok(())
    }

    /// Delete a kanji entry and any SRS cards referencing it
    pub fn delete(&self, id: i64) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM srs_cards WHERE kanji_id = ?", params![id])?;
        conn.execute("DELETE FROM kanji WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Get kanji by character
    pub fn get_by_character(&self, character: &str) -> Result<Option<KanjiEntry>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, character, on_readings, kun_readings, meanings, stroke_count, radical, jlpt_level, grade, language, created_at 
             FROM kanji 
             WHERE character = ?",
        )?;
        
        stmt.query_row(params![character], |row| KanjiEntry::from_row(row))
            .map(Some)
            .map_err(|e| anyhow::Error::new(e))
            .or_else(|e| if e.downcast_ref::<rusqlite::Error>() == Some(&rusqlite::Error::QueryReturnedNoRows) { Ok(None) } else { Err(e) })
    }
    
    /// Search by character, reading, or meaning
    pub fn search(&self, query: &str) -> Result<Vec<KanjiEntry>> {
        let conn = self.db.conn();
        let query_pattern = format!("%{}%", query);
        
        let mut stmt = conn.prepare(
            "SELECT id, character, on_readings, kun_readings, meanings, stroke_count, radical, jlpt_level, grade, language, created_at 
             FROM kanji 
             WHERE character LIKE ? OR on_readings LIKE ? OR kun_readings LIKE ? OR meanings LIKE ? 
             ORDER BY character",
        )?;
        
        let entries = stmt.query_map(
            params![&query_pattern, &query_pattern, &query_pattern, &query_pattern],
            KanjiEntry::from_row,
        )?.collect::<RResult<Vec<_>>>()
            .map_err(|e| anyhow::Error::new(e))?;
        
        Ok(entries)
    }
}

/// Database-specific implementation for SrsCard
impl SrsCard {
    fn from_row(row: &Row) -> RResult<Self> {
        let state_str: String = row.get(4)?;
        let state = CardState::from_str(&state_str)
            .ok_or_else(|| RusqliteError::FromSqlConversionFailure(
                4, 
                Type::Text,
                Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Invalid card state: {}", state_str)))
            ))?;
        
        Ok(Self {
            id: row.get(0)?,
            vocab_id: row.get(1)?,
            kanji_id: row.get(2)?,
            card_type: row.get(3)?,
            state,
            repetitions: row.get(5)?,
            ease_factor: row.get(6)?,
            interval_days: row.get(7)?,
            next_review_date: row.get(8)?,
            last_reviewed: row.get(9)?,
            language: row.get(10)?,
            created_at: row.get(11)?,
            stability: row.get(12)?,
            difficulty: row.get(13)?,
        })
    }
}

/// SQLite repository for SRS card operations
pub struct SqliteSrsRepository {
    db: Arc<Database>,
}

impl SqliteSrsRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }
    
    /// Create a new SRS card, returning its assigned ID
    pub fn create(&self, card: &SrsCard) -> Result<i64> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO srs_cards (vocab_id, kanji_id, card_type, state, repetitions, ease_factor, interval_days, next_review_date, last_reviewed, language, stability, difficulty)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                card.vocab_id,
                card.kanji_id,
                card.card_type,
                card.state.as_str(),
                card.repetitions,
                card.ease_factor,
                card.interval_days,
                card.next_review_date,
                card.last_reviewed,
                card.language,
                card.stability,
                card.difficulty,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Delete a card
    pub fn delete(&self, card_id: i64) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM srs_cards WHERE id = ?", params![card_id])?;
        Ok(())
    }

    /// Get cards due for review today
    pub fn get_due_cards(&self, language: &str) -> Result<Vec<SrsCard>> {
        let conn = self.db.conn();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        
        let mut stmt = conn.prepare(
            "SELECT id, vocab_id, kanji_id, card_type, state, repetitions, ease_factor, interval_days, next_review_date, last_reviewed, language, created_at, stability, difficulty
             FROM srs_cards
             WHERE language = ? AND date(next_review_date) <= date(?)
             ORDER BY next_review_date ASC",
        )?;
        
        let cards = stmt.query_map(params![language, today], SrsCard::from_row)?
            .collect::<RResult<Vec<_>>>()
            .map_err(|e| anyhow::Error::new(e))?;
        
        Ok(cards)
    }
    
    /// Update card after review using the SM-2 backend. Kept for existing
    /// callers/tests; prefer `update_after_review_with_algorithm` so the
    /// user's chosen algorithm (see `Settings::srs_algorithm`) is honored.
    pub fn update_after_review(&self, card_id: i64, rating: SrsRating) -> Result<()> {
        self.update_after_review_with_algorithm(card_id, rating, "sm2")
    }

    /// Update card after review, using FSRS if `algorithm == "fsrs"` and
    /// SM-2 otherwise.
    pub fn update_after_review_with_algorithm(&self, card_id: i64, rating: SrsRating, algorithm: &str) -> Result<()> {
        let conn = self.db.conn();

        // Load the full card from database
        let card = self.get_card_by_id(card_id)?;

        let mut card_for_update = card.clone();
        if algorithm == "fsrs" {
            crate::srs::FSRSBackend::default().update_card(&mut card_for_update, rating);
        } else {
            SM2Backend.update_card(&mut card_for_update, rating);
        }

        // Update the card in database
        let mut update_stmt = conn.prepare(
            "UPDATE srs_cards
             SET state = ?, repetitions = ?, ease_factor = ?, interval_days = ?, next_review_date = ?, last_reviewed = ?, stability = ?, difficulty = ?
             WHERE id = ?",
        )?;

        update_stmt.execute(params![
            card_for_update.state.as_str(),
            card_for_update.repetitions,
            card_for_update.ease_factor,
            card_for_update.interval_days,
            card_for_update.next_review_date,
            card_for_update.last_reviewed,
            card_for_update.stability,
            card_for_update.difficulty,
            card_id,
        ])?;

        Ok(())
    }
    
    /// Get a single card by ID
    pub fn get_card_by_id(&self, card_id: i64) -> Result<SrsCard> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, vocab_id, kanji_id, card_type, state, repetitions, ease_factor, interval_days, next_review_date, last_reviewed, language, created_at, stability, difficulty
             FROM srs_cards WHERE id = ?",
        )?;

        let card = stmt.query_row(params![card_id], SrsCard::from_row)?;
        Ok(card)
    }
}

/// A user-created deck grouping SRS cards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
    pub id: Option<i64>,
    pub name: String,
    pub description: Option<String>,
    pub language: String,
    pub created_at: Option<String>,
}

impl Deck {
    fn from_row(row: &Row) -> RResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            language: row.get(3)?,
            created_at: row.get(4)?,
        })
    }
}

/// SQLite repository for deck operations
pub struct SqliteDeckRepository {
    db: Arc<Database>,
}

impl SqliteDeckRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn create(&self, deck: &Deck) -> Result<i64> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO decks (name, description, language) VALUES (?, ?, ?)",
            params![deck.name, deck.description, deck.language],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<Deck>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare("SELECT id, name, description, language, created_at FROM decks WHERE id = ?")?;
        match stmt.query_row(params![id], Deck::from_row) {
            Ok(deck) => Ok(Some(deck)),
            Err(RusqliteError::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::Error::new(e)),
        }
    }

    pub fn get_by_language(&self, language: &str) -> Result<Vec<Deck>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, language, created_at FROM decks WHERE language = ? ORDER BY name",
        )?;
        let decks = stmt
            .query_map(params![language], Deck::from_row)?
            .collect::<RResult<Vec<_>>>()
            .map_err(anyhow::Error::new)?;
        Ok(decks)
    }

    pub fn update(&self, id: i64, deck: &Deck) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE decks SET name = ?, description = ? WHERE id = ?",
            params![deck.name, deck.description, id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM deck_cards WHERE deck_id = ?", params![id])?;
        conn.execute("DELETE FROM decks WHERE id = ?", params![id])?;
        Ok(())
    }

    /// Add a card to a deck
    pub fn add_card(&self, deck_id: i64, card_id: i64) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT OR IGNORE INTO deck_cards (deck_id, card_id) VALUES (?, ?)",
            params![deck_id, card_id],
        )?;
        Ok(())
    }

    /// Remove a card from a deck
    pub fn remove_card(&self, deck_id: i64, card_id: i64) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "DELETE FROM deck_cards WHERE deck_id = ? AND card_id = ?",
            params![deck_id, card_id],
        )?;
        Ok(())
    }

    /// Card IDs belonging to a deck
    pub fn card_ids(&self, deck_id: i64) -> Result<Vec<i64>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare("SELECT card_id FROM deck_cards WHERE deck_id = ?")?;
        let ids = stmt
            .query_map(params![deck_id], |row| row.get::<_, i64>(0))?
            .collect::<RResult<Vec<_>>>()
            .map_err(anyhow::Error::new)?;
        Ok(ids)
    }
}

/// A single review event, kept for statistics/history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub id: Option<i64>,
    pub card_id: i64,
    pub rating: SrsRating,
    pub review_duration_seconds: Option<i32>,
    pub reviewed_at: Option<String>,
    pub language: String,
}

impl ReviewRecord {
    fn from_row(row: &Row) -> RResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            card_id: row.get(1)?,
            rating: row.get(2)?,
            review_duration_seconds: row.get(3)?,
            reviewed_at: row.get(4)?,
            language: row.get(5)?,
        })
    }
}

/// SQLite repository for review history
pub struct SqliteReviewRepository {
    db: Arc<Database>,
}

impl SqliteReviewRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Record a completed review
    pub fn create(&self, record: &ReviewRecord) -> Result<i64> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT INTO review_history (card_id, rating, review_duration_seconds, language) VALUES (?, ?, ?, ?)",
            params![record.card_id, record.rating, record.review_duration_seconds, record.language],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// All reviews for a language, most recent first
    pub fn get_by_language(&self, language: &str) -> Result<Vec<ReviewRecord>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, card_id, rating, review_duration_seconds, reviewed_at, language
             FROM review_history WHERE language = ? ORDER BY reviewed_at DESC",
        )?;
        let records = stmt
            .query_map(params![language], ReviewRecord::from_row)?
            .collect::<RResult<Vec<_>>>()
            .map_err(anyhow::Error::new)?;
        Ok(records)
    }

    /// Count of reviews for a language on a given date (YYYY-MM-DD)
    pub fn count_on_date(&self, language: &str, date: &str) -> Result<i64> {
        let conn = self.db.conn();
        conn.query_row(
            "SELECT COUNT(*) FROM review_history WHERE language = ? AND date(reviewed_at) = date(?)",
            params![language, date],
            |row| row.get(0),
        )
        .map_err(anyhow::Error::new)
    }
}

/// Installation/registration record for a language, mirrors `LanguageManager`'s
/// in-memory tracking but persisted across restarts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageRecord {
    pub id: Option<i64>,
    pub code: String,
    pub name: String,
    pub is_installed: bool,
    pub version: Option<String>,
    pub installed_at: Option<String>,
}

impl LanguageRecord {
    fn from_row(row: &Row) -> RResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            code: row.get(1)?,
            name: row.get(2)?,
            is_installed: row.get(3)?,
            version: row.get(4)?,
            installed_at: row.get(5)?,
        })
    }
}

/// SQLite repository for language installation records
pub struct SqliteLanguageRepository {
    db: Arc<Database>,
}

impl SqliteLanguageRepository {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Register a language if it isn't already present
    pub fn ensure_registered(&self, code: &str, name: &str) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "INSERT OR IGNORE INTO languages (code, name, is_installed) VALUES (?, ?, FALSE)",
            params![code, name],
        )?;
        Ok(())
    }

    pub fn get_by_code(&self, code: &str) -> Result<Option<LanguageRecord>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, code, name, is_installed, version, installed_at FROM languages WHERE code = ?",
        )?;
        match stmt.query_row(params![code], LanguageRecord::from_row) {
            Ok(record) => Ok(Some(record)),
            Err(RusqliteError::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::Error::new(e)),
        }
    }

    pub fn get_all(&self) -> Result<Vec<LanguageRecord>> {
        let conn = self.db.conn();
        let mut stmt =
            conn.prepare("SELECT id, code, name, is_installed, version, installed_at FROM languages ORDER BY name")?;
        let records = stmt
            .query_map([], LanguageRecord::from_row)?
            .collect::<RResult<Vec<_>>>()
            .map_err(anyhow::Error::new)?;
        Ok(records)
    }

    /// Mark a language installed (or not) with an optional resource version
    pub fn set_installed(&self, code: &str, installed: bool, version: Option<&str>) -> Result<()> {
        let conn = self.db.conn();
        conn.execute(
            "UPDATE languages SET is_installed = ?, version = ?, installed_at = CASE WHEN ? THEN CURRENT_TIMESTAMP ELSE installed_at END
             WHERE code = ?",
            params![installed, version, installed, code],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::initialize_schema;
    use tempfile::NamedTempFile;

    /// Returns the tempfile alongside the DB — SQLite refuses writes once it
    /// detects the backing file has been deleted out from under it, so the
    /// caller must keep the returned tempfile alive for the DB's lifetime.
    fn setup_db() -> (Arc<Database>, NamedTempFile) {
        let temp = NamedTempFile::new().unwrap();
        let db = Database::open(temp.path()).unwrap();
        initialize_schema(&db.conn()).unwrap();
        crate::database::run_migrations(&mut db.conn(), &crate::database::default_migrations()).unwrap();
        (Arc::new(db), temp)
    }

    #[test]
    fn test_vocabulary_crud() {
        let (db, _temp) = setup_db();
        let repo = SqliteVocabularyRepository::new(db);
        let entry = VocabularyEntry {
            id: None,
            word: "受け取る".to_string(),
            reading: Some("うけとる".to_string()),
            meaning: "to receive".to_string(),
            language: "ja".to_string(),
            level: Some("N4".to_string()),
            part_of_speech: Some("verb".to_string()),
            tags: None,
            created_at: None,
            updated_at: None,
        };

        let id = repo.create(&entry).unwrap();
        let fetched = repo.get_by_id(id).unwrap().unwrap();
        assert_eq!(fetched.word, "受け取る");

        let mut updated = fetched.clone();
        updated.meaning = "to accept".to_string();
        repo.update(id, &updated).unwrap();
        assert_eq!(repo.get_by_id(id).unwrap().unwrap().meaning, "to accept");

        repo.delete(id).unwrap();
        assert!(repo.get_by_id(id).unwrap().is_none());
    }

    #[test]
    fn test_kanji_crud() {
        let (db, _temp) = setup_db();
        let repo = SqliteKanjiRepository::new(db);
        let entry = KanjiEntry {
            id: None,
            character: "受".to_string(),
            on_readings: Some("ジュ".to_string()),
            kun_readings: Some("う.ける".to_string()),
            meanings: "receive".to_string(),
            stroke_count: Some(8),
            radical: None,
            jlpt_level: Some(3),
            grade: Some(3),
            language: "ja".to_string(),
            created_at: None,
        };

        let id = repo.create(&entry).unwrap();
        assert_eq!(repo.get_by_id(id).unwrap().unwrap().character, "受");

        repo.delete(id).unwrap();
        assert!(repo.get_by_id(id).unwrap().is_none());
    }

    #[test]
    fn test_srs_card_create_and_delete() {
        let (db, _temp) = setup_db();
        let repo = SqliteSrsRepository::new(db);
        let card = SrsCard::new("vocabulary", "ja");
        let id = repo.create(&card).unwrap();
        assert!(repo.get_card_by_id(id).is_ok());
        repo.delete(id).unwrap();
        assert!(repo.get_card_by_id(id).is_err());
    }

    #[test]
    fn test_deck_crud_and_cards() {
        let (db, _temp) = setup_db();
        let decks = SqliteDeckRepository::new(db.clone());
        let srs = SqliteSrsRepository::new(db);

        let deck_id = decks
            .create(&Deck {
                id: None,
                name: "JLPT N4".to_string(),
                description: None,
                language: "ja".to_string(),
                created_at: None,
            })
            .unwrap();
        let card_id = srs.create(&SrsCard::new("vocabulary", "ja")).unwrap();

        decks.add_card(deck_id, card_id).unwrap();
        assert_eq!(decks.card_ids(deck_id).unwrap(), vec![card_id]);

        decks.remove_card(deck_id, card_id).unwrap();
        assert!(decks.card_ids(deck_id).unwrap().is_empty());

        let fetched = decks.get_by_id(deck_id).unwrap().unwrap();
        assert_eq!(fetched.name, "JLPT N4");

        decks.delete(deck_id).unwrap();
        assert!(decks.get_by_id(deck_id).unwrap().is_none());
    }

    #[test]
    fn test_review_history() {
        let (db, _temp) = setup_db();
        let srs = SqliteSrsRepository::new(db.clone());
        let reviews = SqliteReviewRepository::new(db);

        let card_id = srs.create(&SrsCard::new("vocabulary", "ja")).unwrap();
        reviews
            .create(&ReviewRecord {
                id: None,
                card_id,
                rating: 3,
                review_duration_seconds: Some(5),
                reviewed_at: None,
                language: "ja".to_string(),
            })
            .unwrap();

        let history = reviews.get_by_language("ja").unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].card_id, card_id);

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(reviews.count_on_date("ja", &today).unwrap(), 1);
    }

    #[test]
    fn test_language_repository() {
        let (db, _temp) = setup_db();
        let repo = SqliteLanguageRepository::new(db);
        repo.ensure_registered("ja", "Japanese").unwrap();
        repo.ensure_registered("ja", "Japanese").unwrap(); // idempotent

        let all = repo.get_all().unwrap();
        assert_eq!(all.len(), 1);
        assert!(!all[0].is_installed);

        repo.set_installed("ja", true, Some("3.5.0")).unwrap();
        let record = repo.get_by_code("ja").unwrap().unwrap();
        assert!(record.is_installed);
        assert_eq!(record.version, Some("3.5.0".to_string()));
    }
}


