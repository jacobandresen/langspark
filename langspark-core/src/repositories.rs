//! Repository implementations
//!
//! Concrete implementations of the Repository trait for SQLite.

use crate::database::{Database, Repository};
use crate::language::Language;
use anyhow::{Context, Result};
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    fn from_row(row: &Row) -> Result<Self> {
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
            .collect::<Result<Vec<_>>>()?;
        
        Ok(entries)
    }
    
    /// Search by word, reading, or meaning
    pub fn search(&self, query: &str, language: Option<&str>) -> Result<Vec<VocabularyEntry>> {
        let conn = self.db.conn();
        let query_pattern = format!("%{}%", query);
        
        let (sql, params) = if let Some(lang) = language {
            (
                "SELECT id, word, reading, meaning, language, level, part_of_speech, tags, created_at, updated_at 
                 FROM vocabulary 
                 WHERE language = ? AND (word LIKE ? OR reading LIKE ? OR meaning LIKE ?) 
                 ORDER BY word",
                params![lang, &query_pattern, &query_pattern, &query_pattern],
            )
        } else {
            (
                "SELECT id, word, reading, meaning, language, level, part_of_speech, tags, created_at, updated_at 
                 FROM vocabulary 
                 WHERE word LIKE ? OR reading LIKE ? OR meaning LIKE ? 
                 ORDER BY word",
                params![&query_pattern, &query_pattern, &query_pattern],
            )
        };
        
        let mut stmt = conn.prepare(sql)?;
        let entries = stmt.query_map(params, VocabularyEntry::from_row)?
            .collect::<Result<Vec<_>>>()?;
        
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
    fn from_row(row: &Row) -> Result<Self> {
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
            .or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(e) })
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
        )?.collect::<Result<Vec<_>>>()?;
        
        Ok(entries)
    }
}

/// SRS card state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CardState {
    New,
    Learning,
    Review,
}

impl CardState {
    pub fn as_str(&self) -> &'static str {
        match self {
            CardState::New => "new",
            CardState::Learning => "learning",
            CardState::Review => "review",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "new" => Some(CardState::New),
            "learning" => Some(CardState::Learning),
            "review" => Some(CardState::Review),
            _ => None,
        }
    }
}

/// SRS card entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrsCard {
    pub id: Option<i64>,
    pub vocab_id: Option<i64>,
    pub kanji_id: Option<i64>,
    pub card_type: String,
    pub state: CardState,
    pub repetitions: i32,
    pub ease_factor: f64,
    pub interval_days: i32,
    pub next_review_date: Option<String>,
    pub last_reviewed: Option<String>,
    pub language: String,
    pub created_at: Option<String>,
}

impl SrsCard {
    fn from_row(row: &Row) -> Result<Self> {
        let state_str: String = row.get(4)?;
        let state = CardState::from_str(&state_str)
            .context(format!("Invalid card state: {}", state_str))?;
        
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

/// Rating for SRS (1=Again, 2=Hard, 3=Good, 4=Easy)
pub type SrsRating = u32;

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
            .collect::<Result<Vec<_>>>()?;
        
        Ok(cards)
    }
    
    /// Update card after review
    pub fn update_after_review(&self, card_id: i64, rating: SrsRating) -> Result<()> {
        let conn = self.db.conn();
        
        // Get current card state
        let mut stmt = conn.prepare(
            "SELECT state, repetitions, ease_factor, interval_days FROM srs_cards WHERE id = ?",
        )?;
        
        let row = stmt.query_row(params![card_id], |row| {
            let state_str: String = row.get(0)?;
            let repetitions: i32 = row.get(1)?;
            let ease_factor: f64 = row.get(2)?;
            let interval_days: i32 = row.get(3)?;
            
            Ok((state_str, repetitions, ease_factor, interval_days))
        })?;
        
        let (state_str, repetitions, ease_factor, interval_days) = row;
        let state = CardState::from_str(&state_str)
            .context("Invalid card state")?;
        
        // Calculate new values based on SM-2 algorithm
        let (new_state, new_repetitions, new_ease_factor, new_interval) = 
            calculate_sm2(state, repetitions, ease_factor, interval_days, rating);
        
        let new_next_review = chrono::Local::now() + chrono::Duration::days(new_interval as i64);
        
        // Update the card
        let mut update_stmt = conn.prepare(
            "UPDATE srs_cards 
             SET state = ?, repetitions = ?, ease_factor = ?, interval_days = ?, next_review_date = ?, last_reviewed = CURRENT_TIMESTAMP 
             WHERE id = ?",
        )?;
        
        update_stmt.execute(params![
            new_state.as_str(),
            new_repetitions,
            new_ease_factor,
            new_interval,
            new_next_review.format("%Y-%m-%d %H:%M:%S").to_string(),
            card_id,
        ])?;
        
        Ok(())
    }
}

/// Calculate SM-2 algorithm values
/// Based on the Anki SM-2 algorithm
fn calculate_sm2(
    current_state: CardState,
    repetitions: i32,
    ease_factor: f64,
    interval: i32,
    rating: SrsRating,
) -> (CardState, i32, f64, i32) {
    match rating {
        1 => {
            // Again - reset repetitions, decrease ease factor
            let new_ease_factor = (ease_factor - 0.20).max(1.3);
            (CardState::New, 0, new_ease_factor, 1)
        }
        2 => {
            // Hard - reset repetitions, same interval
            let new_ease_factor = (ease_factor - 0.15).max(1.3);
            let new_interval = (interval as f64 * new_ease_factor) as i32;
            (CardState::Learning, 0, new_ease_factor, new_interval.max(1))
        }
        3 => {
            // Good - increment repetitions
            let new_repetitions = repetitions + 1;
            let new_interval = match new_repetitions {
                1 => 1,      // First correct review: 1 day
                2 => 6,      // Second correct review: 6 days
                _ => (interval as f64 * ease_factor) as i32,
            };
            let new_state = match new_repetitions {
                1 => CardState::Learning,
                2 => CardState::Learning,
                3 => CardState::Review,
                _ => CardState::Review,
            };
            (new_state, new_repetitions, ease_factor, new_interval.max(1))
        }
        4 => {
            // Easy - increment repetitions, increase ease factor
            let new_repetitions = repetitions + 1;
            let new_ease_factor = (ease_factor + 0.15).min(2.5);
            let new_interval = match new_repetitions {
                1 => 1,
                2 => 6,
                _ => (interval as f64 * new_ease_factor) as i32,
            };
            let new_state = match new_repetitions {
                1 => CardState::Learning,
                2 => CardState::Learning,
                3 => CardState::Review,
                _ => CardState::Review,
            };
            (new_state, new_repetitions, new_ease_factor, new_interval.max(1))
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sm2_again_rating() {
        let (state, reps, ef, interval) = calculate_sm2(
            CardState::New, 0, 2.5, 0, 1,
        );
        assert_eq!(state, CardState::New);
        assert_eq!(reps, 0);
        assert!((ef - 2.3).abs() < 0.01);
        assert_eq!(interval, 1);
    }
    
    #[test]
    fn test_sm2_good_first_review() {
        let (state, reps, ef, interval) = calculate_sm2(
            CardState::New, 0, 2.5, 0, 3,
        );
        assert_eq!(state, CardState::Learning);
        assert_eq!(reps, 1);
        assert!((ef - 2.5).abs() < 0.01);
        assert_eq!(interval, 1);
    }
    
    #[test]
    fn test_sm2_good_second_review() {
        let (state, reps, ef, interval) = calculate_sm2(
            CardState::Learning, 1, 2.5, 1, 3,
        );
        assert_eq!(state, CardState::Learning);
        assert_eq!(reps, 2);
        assert!((ef - 2.5).abs() < 0.01);
        assert_eq!(interval, 6);
    }
    
    #[test]
    fn test_sm2_good_third_review() {
        let (state, reps, ef, interval) = calculate_sm2(
            CardState::Learning, 2, 2.5, 6, 3,
        );
        assert_eq!(state, CardState::Review);
        assert_eq!(reps, 3);
        assert!((ef - 2.5).abs() < 0.01);
        assert!((interval as f64 - 15.0).abs() < 0.01);
    }
}
