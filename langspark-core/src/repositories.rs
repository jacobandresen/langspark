//! Repository implementations
//!
//! Concrete implementations of the Repository trait for SQLite.

use crate::database::{Database, Repository};
use crate::language::Language;
use crate::srs::{CardState, RATING_AGAIN, RATING_EASY, RATING_GOOD, RATING_HARD, SrsBackend, SrsCard, SM2Backend};
use anyhow::{Context, Result};
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
    
    /// Get cards due for review today
    pub fn get_due_cards(&self, language: &str) -> Result<Vec<SrsCard>> {
        let conn = self.db.conn();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        
        let mut stmt = conn.prepare(
            "SELECT id, vocab_id, kanji_id, card_type, state, repetitions, ease_factor, interval_days, next_review_date, last_reviewed, language, created_at 
             FROM srs_cards 
             WHERE language = ? AND date(next_review_date) <= date(?) 
             ORDER BY next_review_date ASC",
        )?;
        
        let cards = stmt.query_map(params![language, today], SrsCard::from_row)?
            .collect::<RResult<Vec<_>>>()
            .map_err(|e| anyhow::Error::new(e))?;
        
        Ok(cards)
    }
    
    /// Update card after review
    pub fn update_after_review(&self, card_id: i64, rating: SrsRating) -> Result<()> {
        let conn = self.db.conn();
        
        // Load the full card from database
        let card = self.get_card_by_id(card_id)?;
        
        // Use the SM-2 backend to update the card
        let backend = SM2Backend;
        let mut card_for_update = card.clone();
        backend.update_card(&mut card_for_update, rating);
        
        // Update the card in database
        let mut update_stmt = conn.prepare(
            "UPDATE srs_cards 
             SET state = ?, repetitions = ?, ease_factor = ?, interval_days = ?, next_review_date = ?, last_reviewed = ?
             WHERE id = ?",
        )?;
        
        update_stmt.execute(params![
            card_for_update.state.as_str(),
            card_for_update.repetitions,
            card_for_update.ease_factor,
            card_for_update.interval_days,
            card_for_update.next_review_date,
            card_for_update.last_reviewed,
            card_id,
        ])?;
        
        Ok(())
    }
    
    /// Get a single card by ID
    pub fn get_card_by_id(&self, card_id: i64) -> Result<SrsCard> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, vocab_id, kanji_id, card_type, state, repetitions, ease_factor, interval_days, next_review_date, last_reviewed, language, created_at 
             FROM srs_cards WHERE id = ?",
        )?;
        
        let card = stmt.query_row(params![card_id], SrsCard::from_row)?;
        Ok(card)
    }
}


