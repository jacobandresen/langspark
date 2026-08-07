//! Spaced Repetition System module
//!
//! Implements the SM-2 algorithm for scheduling vocabulary reviews.
//!
//! The SM-2 algorithm is based on the original algorithm by Piotr Wozniak,
//! with modifications for language learning applications.

use chrono::Duration;
use serde::{Deserialize, Serialize};

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
    /// FSRS memory stability in days (how long until recall probability
    /// decays to ~90%). Unused by `SM2Backend`. `0.0` until the card has had
    /// its first FSRS-scored review — FSRS derives the initial value from
    /// the first rating rather than at card creation.
    pub stability: f64,
    /// FSRS difficulty, 1 (easiest) to 10 (hardest). Unused by `SM2Backend`.
    /// `0.0` until the card has had its first FSRS-scored review.
    pub difficulty: f64,
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
            stability: 0.0,
            difficulty: 0.0,
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

// ---------------------------------------------------------------------
// FSRS (Free Spaced Repetition Scheduler)
// ---------------------------------------------------------------------

/// FSRS scheduler: models memory as a (stability, difficulty) pair per card
/// rather than SM-2's single ease factor, and schedules the next review for
/// whenever predicted recall probability decays to `desired_retention`.
///
/// Follows the FSRS algorithm's general structure (initial stability/
/// difficulty from the first rating, a power-function forgetting curve, and
/// separate stability-growth/stability-after-lapse formulas) with 17 tunable
/// weights. **The default weights below are reasonable placeholders modeled
/// on FSRS's published parameter roles, not a verified byte-for-byte copy of
/// the official reference implementation's fitted values** — FSRS is
/// normally most accurate when its weights are re-fit to a user's own review
/// history via its optimizer, which this project doesn't implement. Treat
/// this as "FSRS-shaped scheduling that works out of the box," not a
/// certified match to Anki's FSRS.
pub struct FSRSBackend {
    /// w[0..4]: initial stability (days) for first rating Again/Hard/Good/Easy.
    /// w[4],w[5]: initial difficulty base/scale. w[6]: difficulty delta per
    /// grade. w[7]: difficulty mean-reversion weight. w[8..11]: stability
    /// growth on success. w[11..15]: stability after a lapse (Again).
    /// w[15]: Hard penalty. w[16]: Easy bonus.
    weights: [f64; 17],
    /// Target recall probability at the scheduled review date (FSRS default: 0.9).
    desired_retention: f64,
}

impl Default for FSRSBackend {
    fn default() -> Self {
        Self {
            weights: [
                0.4, 0.9, 2.3, 10.9, // w0-3: initial stability by first rating
                5.0, 1.0, // w4, w5: initial difficulty base/scale
                0.9, // w6: difficulty delta per grade
                0.02, // w7: difficulty mean reversion
                1.5, 0.15, 0.85, // w8-10: stability growth on success
                1.2, 0.35, 1.05, 0.35, // w11-14: stability after lapse
                0.6, // w15: Hard penalty
                1.4, // w16: Easy bonus
            ],
            desired_retention: 0.9,
        }
    }
}

impl FSRSBackend {
    /// Use `desired_retention` (0.0-1.0 recall probability target) instead
    /// of FSRS's default 0.9.
    pub fn with_desired_retention(desired_retention: f64) -> Self {
        Self { desired_retention, ..Self::default() }
    }

    const MIN_DIFFICULTY: f64 = 1.0;
    const MAX_DIFFICULTY: f64 = 10.0;
    const MIN_STABILITY: f64 = 0.01;

    /// Days since `card.last_reviewed` (0 if never reviewed, i.e. a New card).
    fn elapsed_days(&self, card: &SrsCard) -> f64 {
        let Some(last) = &card.last_reviewed else { return 0.0 };
        let Ok(last_date) = chrono::NaiveDate::parse_from_str(last, "%Y-%m-%d") else { return 0.0 };
        let today = chrono::Local::now().date_naive();
        (today - last_date).num_days().max(0) as f64
    }

    /// Predicted recall probability after `elapsed` days, given stability
    /// `s` (days) — FSRS's power-function forgetting curve, calibrated so
    /// R(t=s) ≈ 0.9.
    fn retrievability(&self, elapsed: f64, s: f64) -> f64 {
        if s <= 0.0 {
            return 0.0;
        }
        (1.0 + elapsed / (9.0 * s)).powf(-1.0)
    }

    /// Initial stability (days) from the first rating given to a New card.
    fn init_stability(&self, rating: u32) -> f64 {
        let idx = (rating.clamp(RATING_AGAIN, RATING_EASY) - 1) as usize;
        self.weights[idx].max(Self::MIN_STABILITY)
    }

    /// Initial difficulty (1-10) from the first rating given to a New card.
    fn init_difficulty(&self, rating: u32) -> f64 {
        let d = self.weights[4] - (rating as f64 - 3.0) * self.weights[5];
        d.clamp(Self::MIN_DIFFICULTY, Self::MAX_DIFFICULTY)
    }

    /// Next difficulty after a review, with mean reversion toward the
    /// "reviewed Good from new" difficulty so cards don't drift indefinitely.
    fn next_difficulty(&self, card: &SrsCard, rating: u32) -> f64 {
        let w6 = self.weights[6];
        let w7 = self.weights[7];
        let updated = card.difficulty - (rating as f64 - 3.0) * w6;
        let reverted = w7 * self.init_difficulty(RATING_GOOD) + (1.0 - w7) * updated;
        reverted.clamp(Self::MIN_DIFFICULTY, Self::MAX_DIFFICULTY)
    }

    /// Stability after a successful recall (Hard/Good/Easy).
    fn stability_after_success(&self, card: &SrsCard, rating: u32, r: f64) -> f64 {
        let (w8, w9, w10) = (self.weights[8], self.weights[9], self.weights[10]);
        let hard_penalty = if rating == RATING_HARD { self.weights[15] } else { 1.0 };
        let easy_bonus = if rating == RATING_EASY { self.weights[16] } else { 1.0 };
        let d = card.difficulty;
        let s = card.stability;
        let growth =
            1.0 + (w8.exp()) * (11.0 - d) * s.powf(-w9) * (((1.0 - r) * w10).exp() - 1.0) * hard_penalty * easy_bonus;
        (s * growth.max(0.0)).max(Self::MIN_STABILITY)
    }

    /// Stability after a lapse (Again on a card that had a stability already).
    fn stability_after_lapse(&self, card: &SrsCard, r: f64) -> f64 {
        let (w11, w12, w13, w14) = (self.weights[11], self.weights[12], self.weights[13], self.weights[14]);
        let d = card.difficulty.max(Self::MIN_DIFFICULTY);
        let s = w11 * d.powf(-w12) * (((card.stability + 1.0).powf(w13)) - 1.0) * (((1.0 - r) * w14).exp());
        s.max(Self::MIN_STABILITY)
    }

    /// The (stability, difficulty) this card would have after `rating`,
    /// without mutating it — shared by `next_interval` (preview) and
    /// `update_card` (commit).
    fn next_stability_and_difficulty(&self, card: &SrsCard, rating: u32) -> (f64, f64) {
        if card.state == CardState::New {
            return (self.init_stability(rating), self.init_difficulty(rating));
        }
        let elapsed = self.elapsed_days(card);
        let r = self.retrievability(elapsed, card.stability);
        let difficulty = self.next_difficulty(card, rating);
        let stability = if rating == RATING_AGAIN {
            self.stability_after_lapse(card, r)
        } else {
            self.stability_after_success(card, rating, r)
        };
        (stability, difficulty)
    }
}

impl SrsBackend for FSRSBackend {
    fn next_interval(&self, card: &SrsCard, rating: u32) -> i32 {
        let (stability, _) = self.next_stability_and_difficulty(card, rating);
        if rating == RATING_AGAIN && card.state != CardState::New {
            // A lapse always drops back into short-term (re)learning, not a
            // multi-day interval derived from the (now much lower) stability.
            return 0;
        }
        // Solve retrievability(t, stability) = desired_retention for t.
        let interval = 9.0 * stability * (1.0 / self.desired_retention - 1.0);
        interval.round().max(1.0) as i32
    }

    fn update_card(&self, card: &mut SrsCard, rating: u32) {
        let (stability, difficulty) = self.next_stability_and_difficulty(card, rating);
        let new_interval = self.next_interval(card, rating);

        // State transitions mirror SM2Backend's for consistency in the UI
        // (both backends expose the same New/Learning/Review lifecycle).
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

        card.repetitions = match (card.state.clone(), rating) {
            (CardState::Learning, RATING_AGAIN) => 0,
            (CardState::Review, RATING_AGAIN) => 0,
            (CardState::Review, RATING_HARD) => card.repetitions,
            (CardState::Review, RATING_GOOD) => card.repetitions + 1,
            (CardState::Review, RATING_EASY) => card.repetitions + 2,
            _ => card.repetitions + 1,
        };

        card.stability = stability;
        card.difficulty = difficulty;
        card.interval_days = new_interval;

        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        card.last_reviewed = Some(today.clone());
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
        (today + Duration::days(interval as i64)).format("%Y-%m-%d").to_string()
    }
}

/// Manages all SRS cards and scheduling
///
/// Unlike the stateless helpers below (which operate on caller-supplied
/// slices), `SrsManager` owns the in-memory working set of cards for the
/// active session — e.g. the set loaded from the database for the active
/// language — and provides:
/// - Card creation and management
/// - Daily review queue generation
/// - Language-specific card filtering
pub struct SrsManager {
    cards: Vec<SrsCard>,
}

impl Default for SrsManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SrsManager {
    /// Create a new, empty SRS manager
    pub fn new() -> Self {
        Self { cards: Vec::new() }
    }

    /// Load a set of cards into the manager's working set (e.g. after
    /// fetching them from the database for the active language).
    pub fn load_cards(&mut self, cards: Vec<SrsCard>) {
        self.cards = cards;
    }

    /// Add a single card to the working set
    pub fn add_card(&mut self, card: SrsCard) {
        self.cards.push(card);
    }

    /// Remove a card from the working set by ID
    pub fn remove_card(&mut self, card_id: i64) {
        self.cards.retain(|c| c.id != Some(card_id));
    }

    /// All cards currently tracked
    pub fn cards(&self) -> &[SrsCard] {
        &self.cards
    }

    /// Cards in the working set for a specific language
    pub fn cards_for_language(&self, language: &str) -> Vec<&SrsCard> {
        self.cards.iter().filter(|c| c.language == language).collect()
    }

    /// Cards due for review today, filtered to a specific language
    pub fn due_cards_for_language(&self, language: &str) -> Vec<&SrsCard> {
        self.cards
            .iter()
            .filter(|c| c.language == language && c.is_due_today())
            .collect()
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

    /// Like `process_review`, but with the backend selected by `algorithm`
    /// ("fsrs" for `FSRSBackend`, anything else falls back to SM-2).
    pub fn process_review_with_algorithm(&self, mut card: SrsCard, rating: u32, algorithm: &str) -> SrsCard {
        if algorithm == "fsrs" {
            FSRSBackend::default().update_card(&mut card, rating);
        } else {
            SM2Backend.update_card(&mut card, rating);
        }
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

// ---------------------------------------------------------------------
// Statistics tracking
// ---------------------------------------------------------------------

/// Aggregate review statistics for a study session or date range
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewStats {
    pub total_reviews: i32,
    pub correct_reviews: i32,
    pub retention_rate: f64,
    pub streak_days: i32,
}

/// Percentage of ratings that counted as a successful recall (Good or Easy).
/// Returns 0.0 for an empty slice rather than dividing by zero.
pub fn calculate_retention_rate(ratings: &[u32]) -> f64 {
    if ratings.is_empty() {
        return 0.0;
    }
    let correct = ratings.iter().filter(|&&r| r >= RATING_GOOD).count();
    (correct as f64 / ratings.len() as f64) * 100.0
}

/// Length of the current consecutive-day study streak, given the distinct
/// dates (YYYY-MM-DD, any order) on which at least one review happened.
/// The streak counts back from today; a gap breaks it.
pub fn calculate_streak(review_dates: &[String]) -> i32 {
    use std::collections::HashSet;

    let dates: HashSet<&str> = review_dates.iter().map(String::as_str).collect();
    if dates.is_empty() {
        return 0;
    }

    let mut streak = 0;
    let mut cursor = chrono::Local::now().date_naive();
    loop {
        let cursor_str = cursor.format("%Y-%m-%d").to_string();
        if dates.contains(cursor_str.as_str()) {
            streak += 1;
            cursor -= Duration::days(1);
        } else {
            break;
        }
    }
    streak
}

/// Build a `ReviewStats` summary from a session's ratings and the set of
/// distinct dates studied so far (for streak calculation).
pub fn build_review_stats(ratings: &[u32], review_dates: &[String]) -> ReviewStats {
    let correct = ratings.iter().filter(|&&r| r >= RATING_GOOD).count() as i32;
    ReviewStats {
        total_reviews: ratings.len() as i32,
        correct_reviews: correct,
        retention_rate: calculate_retention_rate(ratings),
        streak_days: calculate_streak(review_dates),
    }
}

// ---------------------------------------------------------------------
// Deck management
// ---------------------------------------------------------------------

/// In-memory organization of card IDs into named decks, language-aware.
/// Persistence is handled separately by `SqliteDeckRepository`; this is the
/// business-logic layer used while a deck is actively being studied/edited.
#[derive(Debug, Default)]
pub struct DeckManager {
    /// deck_id -> card IDs
    deck_cards: std::collections::HashMap<i64, Vec<i64>>,
}

impl DeckManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a card to a deck (no-op if already present)
    pub fn add_card(&mut self, deck_id: i64, card_id: i64) {
        let cards = self.deck_cards.entry(deck_id).or_default();
        if !cards.contains(&card_id) {
            cards.push(card_id);
        }
    }

    /// Remove a card from a deck
    pub fn remove_card(&mut self, deck_id: i64, card_id: i64) {
        if let Some(cards) = self.deck_cards.get_mut(&deck_id) {
            cards.retain(|&id| id != card_id);
        }
    }

    /// Card IDs belonging to a deck
    pub fn cards_in_deck(&self, deck_id: i64) -> &[i64] {
        self.deck_cards.get(&deck_id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Number of a deck's cards that are due today, given the full card set
    pub fn due_count_in_deck(&self, deck_id: i64, all_cards: &[SrsCard]) -> usize {
        let deck_card_ids = self.cards_in_deck(deck_id);
        all_cards
            .iter()
            .filter(|c| c.id.map(|id| deck_card_ids.contains(&id)).unwrap_or(false) && c.is_due_today())
            .count()
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

    #[test]
    fn test_srs_manager_language_filtering() {
        let mut manager = SrsManager::new();
        let mut ja_card = SrsCard::new("vocabulary", "ja");
        ja_card.id = Some(1);
        let mut es_card = SrsCard::new("vocabulary", "es");
        es_card.id = Some(2);

        manager.add_card(ja_card);
        manager.add_card(es_card);

        assert_eq!(manager.cards().len(), 2);
        assert_eq!(manager.cards_for_language("ja").len(), 1);
        assert_eq!(manager.cards_for_language("es").len(), 1);
        assert_eq!(manager.due_cards_for_language("ja").len(), 1); // new cards are due

        manager.remove_card(1);
        assert_eq!(manager.cards().len(), 1);
        assert!(manager.cards_for_language("ja").is_empty());
    }

    #[test]
    fn test_calculate_retention_rate() {
        assert_eq!(calculate_retention_rate(&[]), 0.0);
        assert_eq!(
            calculate_retention_rate(&[RATING_GOOD, RATING_EASY, RATING_AGAIN, RATING_HARD]),
            50.0
        );
        assert_eq!(calculate_retention_rate(&[RATING_GOOD, RATING_EASY]), 100.0);
    }

    #[test]
    fn test_calculate_streak() {
        assert_eq!(calculate_streak(&[]), 0);

        let today = chrono::Local::now().date_naive();
        let yesterday = (today - Duration::days(1)).format("%Y-%m-%d").to_string();
        let today_str = today.format("%Y-%m-%d").to_string();
        let two_weeks_ago = (today - Duration::days(14)).format("%Y-%m-%d").to_string();

        // Consecutive streak of 2
        assert_eq!(calculate_streak(&[today_str.clone(), yesterday.clone()]), 2);

        // Gap breaks the streak at 1
        assert_eq!(calculate_streak(&[today_str, two_weeks_ago]), 1);
    }

    #[test]
    fn test_build_review_stats() {
        let today_str = chrono::Local::now().format("%Y-%m-%d").to_string();
        let stats = build_review_stats(&[RATING_GOOD, RATING_EASY, RATING_AGAIN], &[today_str]);
        assert_eq!(stats.total_reviews, 3);
        assert_eq!(stats.correct_reviews, 2);
        assert!((stats.retention_rate - (200.0 / 3.0)).abs() < 0.01);
        assert_eq!(stats.streak_days, 1);
    }

    #[test]
    fn test_deck_manager() {
        let mut manager = DeckManager::new();
        manager.add_card(1, 100);
        manager.add_card(1, 101);
        manager.add_card(1, 100); // duplicate, no-op
        manager.add_card(2, 200);

        assert_eq!(manager.cards_in_deck(1), &[100, 101]);
        assert_eq!(manager.cards_in_deck(2), &[200]);
        assert!(manager.cards_in_deck(99).is_empty());

        manager.remove_card(1, 100);
        assert_eq!(manager.cards_in_deck(1), &[101]);

        let mut card = SrsCard::new("vocabulary", "ja");
        card.id = Some(101);
        assert_eq!(manager.due_count_in_deck(1, &[card]), 1);
    }

    // -------------------------------------------------------------------
    // FSRSBackend
    // -------------------------------------------------------------------

    #[test]
    fn test_fsrs_first_review_initializes_stability_and_difficulty() {
        let backend = FSRSBackend::default();
        let mut card = SrsCard::new("vocabulary", "ja");
        assert_eq!(card.stability, 0.0);
        assert_eq!(card.difficulty, 0.0);

        backend.update_card(&mut card, RATING_GOOD);

        assert!(card.stability > 0.0);
        assert!((1.0..=10.0).contains(&card.difficulty));
        assert_eq!(card.state, CardState::Learning);
        assert!(card.interval_days >= 1);
        assert!(card.next_review_date.is_some());
    }

    #[test]
    fn test_fsrs_easy_grants_more_stability_than_again() {
        let backend = FSRSBackend::default();
        let mut easy_card = SrsCard::new("vocabulary", "ja");
        backend.update_card(&mut easy_card, RATING_EASY);

        let mut again_card = SrsCard::new("vocabulary", "ja");
        backend.update_card(&mut again_card, RATING_AGAIN);

        assert!(easy_card.stability > again_card.stability);
        // A first-review lapse shouldn't leave the card in New forever, but
        // it also shouldn't advance past Learning like a pass does.
        assert_eq!(again_card.state, CardState::New);
    }

    #[test]
    fn test_fsrs_lapse_after_review_resets_to_learning_and_shrinks_interval() {
        let backend = FSRSBackend::default();
        let mut card = SrsCard::new("vocabulary", "ja");
        backend.update_card(&mut card, RATING_GOOD);
        backend.update_card(&mut card, RATING_GOOD); // now in Review with some stability
        assert_eq!(card.state, CardState::Review);
        let stability_before_lapse = card.stability;

        backend.update_card(&mut card, RATING_AGAIN);

        assert_eq!(card.state, CardState::Learning);
        assert_eq!(card.repetitions, 0);
        // A lapse always schedules a same-day (re-)review, not a multi-day gap.
        assert_eq!(card.interval_days, 0);
        assert!(card.stability < stability_before_lapse);
    }

    #[test]
    fn test_fsrs_stability_grows_across_repeated_good_reviews() {
        let backend = FSRSBackend::default();
        let mut card = SrsCard::new("vocabulary", "ja");
        let mut stabilities = Vec::new();
        for _ in 0..4 {
            backend.update_card(&mut card, RATING_GOOD);
            stabilities.push(card.stability);
            // Simulate time passing so the next review isn't computed at
            // elapsed=0 (which would otherwise floor retrievability oddly).
            card.last_reviewed = Some(
                (chrono::Local::now() - Duration::days(card.interval_days as i64)).format("%Y-%m-%d").to_string(),
            );
        }
        // Consecutive successful reviews should grow (non-decreasing) stability.
        for window in stabilities.windows(2) {
            assert!(window[1] >= window[0], "stability should not shrink on repeated Good ratings: {stabilities:?}");
        }
    }

    #[test]
    fn test_fsrs_higher_desired_retention_yields_shorter_intervals() {
        let mut card = SrsCard::new("vocabulary", "ja");
        FSRSBackend::default().update_card(&mut card, RATING_GOOD);

        let relaxed = FSRSBackend::with_desired_retention(0.7).next_interval(&card, RATING_GOOD);
        let strict = FSRSBackend::with_desired_retention(0.97).next_interval(&card, RATING_GOOD);

        assert!(strict < relaxed, "aiming for higher retention should schedule sooner: strict={strict} relaxed={relaxed}");
    }

    #[test]
    fn test_fsrs_difficulty_stays_in_bounds_across_many_reviews() {
        let backend = FSRSBackend::default();
        let mut card = SrsCard::new("vocabulary", "ja");
        for rating in [RATING_AGAIN, RATING_EASY, RATING_AGAIN, RATING_HARD, RATING_GOOD, RATING_EASY].repeat(5) {
            backend.update_card(&mut card, rating);
            assert!((1.0..=10.0).contains(&card.difficulty), "difficulty out of bounds: {}", card.difficulty);
            assert!(card.stability >= 0.01, "stability collapsed to non-positive: {}", card.stability);
        }
    }

    #[test]
    fn test_srs_manager_process_review_with_algorithm_selects_backend() {
        let manager = SrsManager::new();
        let card = SrsCard::new("vocabulary", "ja");

        let sm2_result = manager.process_review_with_algorithm(card.clone(), RATING_GOOD, "sm2");
        assert!((sm2_result.ease_factor - 2.5).abs() < f64::EPSILON); // SM-2 leaves ease factor unchanged on Good
        assert_eq!(sm2_result.stability, 0.0); // SM-2 never touches FSRS fields

        let fsrs_result = manager.process_review_with_algorithm(card, RATING_GOOD, "fsrs");
        assert!(fsrs_result.stability > 0.0);
    }
}
