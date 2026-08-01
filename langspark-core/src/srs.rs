//! Spaced Repetition System module
//!
//! Implements the SM-2 algorithm for scheduling vocabulary reviews.

/// Trait for SRS backend implementations
pub trait SrsBackend {
    /// Calculate the next review interval
    fn next_interval(&self, card: &SrsCard, rating: u32) -> i64;
}

/// SM-2 algorithm implementation
pub struct SM2Backend;

/// Represents a card in the SRS system
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SrsCard;

/// Manages all SRS cards and scheduling
pub struct SrsManager;
