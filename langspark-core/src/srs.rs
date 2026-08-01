//! Spaced Repetition System module
//!
//! Implements the SM-2 algorithm for scheduling vocabulary reviews.
//!
//! The SM-2 algorithm is based on the original algorithm by Piotr Wozniak,
//! with modifications for language learning applications.

use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Card state in the SRS system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CardState {
    /// New card that hasn't been seen yet
    New,
    /// Card is in the learning process (first few reviews)
    Learning,
    /// Card is in regular review schedule
    Review,
}

impl CardState {
    /// Convert to string representation for database storage
    pub fn as_str(&self) -> &'static str {
        match self {
            CardState::New => "new",
            CardState::Learning => "learning",
            CardState::Review => "review",
        }
    }

    /// Parse from string representation
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "new" => Some(CardState::New),
            "learning" => Some(CardState::Learning),
            "review" => Some(CardState::Review),
            _ => None,
        }
    }
}

/// SRS rating constants (1=Again, 2=Hard, 3=Good, 4=Easy)
pub const RATING_AGAIN: u32 = 1;
pub const RATING_HARD: u32 = 2;
pub const RATING_GOOD: u32 = 3;
pub const RATING_EASY: u32 = 4;

/// Represents a card in the SRS system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrsCard {
    /// Unique identifier for the card
    pub id: Option<i64>,
    /// Reference to vocabulary entry
    pub vocab_id: Option<i64>,
    /// Reference to kanji entry (if applicable)
    pub kanji_id: Option<i64>,
    /// Type of card (e.g., "vocabulary", "kanji", "recognition")
    pub card_type: String,
    /// Current state of the card
    pub state: CardState,
    /// Number of consecutive successful reviews
    pub repetitions: i32,
    /// Ease factor (affects interval growth rate)
    pub ease_factor: f64,
    /// Current interval in days
    pub interval_days: i32,
    /// Next review date (YYYY-MM-DD format)
    pub next_review_date: Option<String>,
    /// Last reviewed date (YYYY-MM-DD format)
    pub last_reviewed: Option<String>,
    /// Language of the card content
    pub language: String,
    /// Creation timestamp
    pub created_at: Option<String>,
}

impl SrsCard {
    /// Create a new SRS card
    pub fn new(card_type: &str, language: &str) -> Self {
        Self {
            id: None,
            vocab_id: None,
            kanji_id: None,
            card_type: card_type.to_string(),
            state: CardState::New,
            repetitions: 0,
            ease_factor: 2.5, // Default ease factor
            interval_days: 0,
            next_review_date: None,
            last_reviewed: None,
            language: language.to_string(),
            created_at: None,
        }
    }

    /// Check if the card is due for review today
    pub fn is_due_today(&self) -> bool {
        if let Some(next_review) = &self.next_review_date {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            next_review <= &today
        } else {
            // New cards are considered due
            true
        }
    }
}

/// Trait for SRS backend implementations
pub trait SrsBackend {
    /// Calculate the next review interval in days
    fn next_interval(&self, card: &SrsCard, rating: u32) -> i32;

    /// Update card state based on rating
    fn update_card(&self, card: &mut SrsCard, rating: u32);

    /// Get the next review date as a string
    fn next_review_date(&self, card: &SrsCard, rating: u32) -> String;
}

/// SM-2 algorithm implementation
/// 
/// Based on the original SM-2 algorithm by Piotr Wozniak with the following modifications:
/// - Added ease factor adjustment based on performance
/// - Added learning state for initial reviews
/// - Optimized for language learning
pub struct SM2Backend;

impl SM2Backend {
    /// Initial interval for first review (1 day)
    const INITIAL_INTERVAL: i32 = 1;
    /// Interval after first successful review (3 days)
    const SECOND_INTERVAL: i32 = 3;
    /// Minimum ease factor
    const MIN_EASE_FACTOR: f64 = 1.3;
    /// Maximum ease factor
    const MAX_EASE_FACTOR: f64 = 3.0;
}

impl SrsBackend for SM2Backend {
    fn next_interval(&self, card: &SrsCard, rating: u32) -> i32 {
        match card.state {
            CardState::New => {
                // First time seeing the card
                if rating >= RATING_GOOD {
                    SM2Backend::SECOND_INTERVAL
                } else {
                    // Failed first review - stay in new state
                    0
                }
            }
            CardState::Learning => {
                // In learning phase - use shorter intervals
                match rating {
                    RATING_AGAIN => 0, // Back to new
                    RATING_HARD => 1,
                    RATING_GOOD => 2,
                    RATING_EASY => 3,
                    _ => 1,
                }
            }
            CardState::Review => {
                // Standard SM-2 algorithm for review state
                let new_interval = match rating {
                    RATING_AGAIN => {
                        // Reset repetitions, use minimal interval
                        1
                    }
                    RATING_HARD => {
                        // Keep same interval, don't increase repetitions
                        card.interval_days
                    }
                    RATING_GOOD => {
                        // Increase interval
                        (card.interval_days as f64 * card.ease_factor).ceil() as i32
                    }
                    RATING_EASY => {
                        // Larger increase for easy
                        (card.interval_days as f64 * card.ease_factor * 1.2).ceil() as i32
                    }
                    _ => card.interval_days,
                };
                
                new_interval.max(1) // Ensure at least 1 day
            }
        }
    }

    fn update_card(&self, card: &mut SrsCard, rating: u32) {
        // Update ease factor first (based on rating)
        let new_ease_factor = match rating {
            RATING_AGAIN => (card.ease_factor - 0.2).max(SM2Backend::MIN_EASE_FACTOR),
            RATING_HARD => (card.ease_factor - 0.15).max(SM2Backend::MIN_EASE_FACTOR),
            RATING_GOOD => card.ease_factor, // No change
            RATING_EASY => (card.ease_factor + 0.1).min(SM2Backend::MAX_EASE_FACTOR),
            _ => card.ease_factor,
        };
        card.ease_factor = new_ease_factor;

        // Calculate new interval and update state
        let new_interval = self.next_interval(card, rating);
        
        card.state = match card.state {
            CardState::New => {
                if rating >= RATING_GOOD {
                    CardState::Learning
                } else {
                    CardState::New
                }
            }
            CardState::Learning => {
                if rating >= RATING_GOOD {
                    CardState::Review
                } else {
                    CardState::New
                }
            }
            CardState::Review => {
                if rating == RATING_AGAIN {
                    CardState::Learning
                } else {
                    CardState::Review
                }
            }
        };

        // Update repetitions
        card.repetitions = match (card.state.clone(), rating) {
            (CardState::Learning, RATING_AGAIN) => 0,
            (CardState::Review, RATING_AGAIN) => 0,
            (CardState::Review, RATING_HARD) => card.repetitions,
            (CardState::Review, RATING_GOOD) => card.repetitions + 1,
            (CardState::Review, RATING_EASY) => card.repetitions + 2,
            _ => card.repetitions + 1,
        };

        card.interval_days = new_interval;
        
        // Update timestamps
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        card.last_reviewed = Some(today.clone());
        
        // Calculate next review date
        if new_interval > 0 {
            let next_date = chrono::Local::now() + Duration::days(new_interval as i64);
            card.next_review_date = Some(next_date.format("%Y-%m-%d").to_string());
        } else {
            card.next_review_date = Some(today);
        }
    }

    fn next_review_date(&self, card: &SrsCard, rating: u32) -> String {
        let interval = self.next_interval(card, rating);
        let today = chrono::Local::now();
        let next_date = today + Duration::days(interval as i64);
        next_date.format("%Y-%m-%d").to_string()
    }
}

/// Manages all SRS cards and scheduling
/// 
/// Provides high-level interface for SRS operations including:
/// - Card creation and management
/// - Daily review queue generation
/// - Statistics tracking
/// - Language-specific card filtering
pub struct SrsManager;

impl SrsManager {
    /// Create a new SRS manager
    pub fn new() -> Self {
        SrsManager
    }

    /// Create a new card for a vocabulary entry
    pub fn create_vocab_card(&self, vocab_id: i64, language: &str) -> SrsCard {
        let mut card = SrsCard::new("vocabulary", language);
        card.vocab_id = Some(vocab_id);
        card
    }

    /// Create a new card for a kanji entry
    pub fn create_kanji_card(&self, kanji_id: i64, language: &str) -> SrsCard {
        let mut card = SrsCard::new("kanji", language);
        card.kanji_id = Some(kanji_id);
        card
    }

    /// Process a card review with a rating
    /// 
    /// Returns the updated card with new scheduling
    pub fn process_review(&self, mut card: SrsCard, rating: u32) -> SrsCard {
        let backend = SM2Backend;
        backend.update_card(&mut card, rating);
        card
    }

    /// Get cards due for review today
    pub fn get_due_cards<'a>(&self, cards: &'a [SrsCard]) -> Vec<&'a SrsCard> {
        cards.iter().filter(|c| c.is_due_today()).collect()
    }

    /// Sort cards by next review date (oldest first)
    pub fn sort_by_due_date(cards: &mut [SrsCard]) {
        cards.sort_by(|a, b| {
            let default_date = "9999-12-31".to_string();
            let a_date = a.next_review_date.as_ref().unwrap_or(&default_date);
            let b_date = b.next_review_date.as_ref().unwrap_or(&default_date);
            a_date.cmp(b_date)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_state_conversion() {
        assert_eq!(CardState::New.as_str(), "new");
        assert_eq!(CardState::Learning.as_str(), "learning");
        assert_eq!(CardState::Review.as_str(), "review");
        
        assert_eq!(CardState::from_str("new"), Some(CardState::New));
        assert_eq!(CardState::from_str("learning"), Some(CardState::Learning));
        assert_eq!(CardState::from_str("review"), Some(CardState::Review));
        assert_eq!(CardState::from_str("invalid"), None);
    }

    #[test]
    fn test_srs_card_creation() {
        let card = SrsCard::new("vocabulary", "ja");
        assert_eq!(card.card_type, "vocabulary");
        assert_eq!(card.language, "ja");
        assert_eq!(card.state, CardState::New);
        assert_eq!(card.repetitions, 0);
        assert!((card.ease_factor - 2.5).abs() < f64::EPSILON);
        assert_eq!(card.interval_days, 0);
    }

    #[test]
    fn test_new_card_first_review() {
        let backend = SM2Backend;
        let mut card = SrsCard::new("vocabulary", "ja");
        
        // First review with Good rating should move to Learning with 3-day interval
        let interval = backend.next_interval(&card, RATING_GOOD);
        assert_eq!(interval, 3);
        
        // Process the review
        backend.update_card(&mut card, RATING_GOOD);
        assert_eq!(card.state, CardState::Learning);
        assert_eq!(card.repetitions, 1);
        assert_eq!(card.interval_days, 3);
        assert!(card.next_review_date.is_some());
    }

    #[test]
    fn test_learning_to_review_progression() {
        let backend = SM2Backend;
        let mut card = SrsCard::new("vocabulary", "ja");
        
        // First review - move to Learning
        backend.update_card(&mut card, RATING_GOOD);
        assert_eq!(card.state, CardState::Learning);
        
        // Second review with Good rating should move to Review
        backend.update_card(&mut card, RATING_GOOD);
        assert_eq!(card.state, CardState::Review);
    }

    #[test]
    fn test_ease_factor_adjustment() {
        let backend = SM2Backend;
        let mut card = SrsCard::new("vocabulary", "ja");
        
        // Set initial ease factor
        card.ease_factor = 2.5;
        
        // Easy rating should increase ease factor
        backend.update_card(&mut card, RATING_EASY);
        assert!(card.ease_factor > 2.5);
        
        // Again rating should decrease ease factor  
        backend.update_card(&mut card, RATING_AGAIN);
        assert!(card.ease_factor < 2.5);
    }

    #[test]
    fn test_interval_calculation() {
        let backend = SM2Backend;
        let mut card = SrsCard::new("vocabulary", "ja");
        
        // Move to review state and set some interval
        backend.update_card(&mut card, RATING_GOOD); // -> Learning
        backend.update_card(&mut card, RATING_GOOD); // -> Review
        card.interval_days = 5; // Set explicit interval
        card.ease_factor = 2.5;
        
        // Good rating should increase interval
        let new_interval = backend.next_interval(&card, RATING_GOOD);
        assert!(new_interval > 5);
        
        // Again rating should reset to minimal
        let reset_interval = backend.next_interval(&card, RATING_AGAIN);
        assert_eq!(reset_interval, 1);
    }

    #[test]
    fn test_due_today_detection() {
        // New card with no next review date should be due
        let card = SrsCard::new("vocabulary", "ja");
        assert!(card.is_due_today());
        
        // Card with future date should not be due
        let mut future_card = SrsCard::new("vocabulary", "ja");
        future_card.next_review_date = Some("2099-12-31".to_string());
        assert!(!future_card.is_due_today());
        
        // Card with past date should be due
        let mut past_card = SrsCard::new("vocabulary", "ja");
        past_card.next_review_date = Some("2000-01-01".to_string());
        assert!(past_card.is_due_today());
    }

    #[test]
    fn test_card_sorting() {
        let mut cards = vec![
            {
                let mut card = SrsCard::new("vocab", "ja");
                card.next_review_date = Some("2024-01-10".to_string());
                card
            },
            {
                let mut card = SrsCard::new("vocab", "ja");
                card.next_review_date = Some("2024-01-01".to_string());
                card
            },
            {
                let mut card = SrsCard::new("vocab", "ja");
                card.next_review_date = Some("2024-01-15".to_string());
                card
            },
        ];
        
        SrsManager::sort_by_due_date(&mut cards);
        
        // Should be sorted by date: 01, 10, 15
        assert_eq!(cards[0].next_review_date, Some("2024-01-01".to_string()));
        assert_eq!(cards[1].next_review_date, Some("2024-01-10".to_string()));
        assert_eq!(cards[2].next_review_date, Some("2024-01-15".to_string()));
    }
}
