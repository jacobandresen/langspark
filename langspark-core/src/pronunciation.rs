//! Pronunciation scoring: compares a speech-recognition transcript against
//! the expected text and produces a score + human-readable feedback.
//!
//! Implements Tier 1 (text matching, per design.md's tiered scoring plan) plus
//! the groundwork for Tier 2 (morae/phoneme segmentation) — full phoneme-level
//! comparison is tracked as future work (tasks 10.8, 28.2).

use serde::{Deserialize, Serialize};

/// Feedback from scoring a user's pronunciation attempt against expected text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PronunciationResult {
    /// Overall match score, 0-100
    pub score: f32,
    /// What the speech recognizer transcribed
    pub recognized_text: String,
    /// The text the user was expected to say
    pub expected_text: String,
    /// Human-readable feedback message
    pub feedback: String,
    /// Whether the score met the "correct" threshold
    pub is_correct: bool,
    /// Language the attempt was scored in (e.g. "ja", "es")
    pub language: String,
}

impl PronunciationResult {
    /// Score threshold above which an attempt counts as correct
    pub const CORRECT_THRESHOLD: f32 = 80.0;

    pub fn new(score: f32, recognized_text: String, expected_text: String, language: &str) -> Self {
        let is_correct = score >= Self::CORRECT_THRESHOLD;
        let feedback = if is_correct {
            "Great pronunciation!".to_string()
        } else if score >= 50.0 {
            "Close! Keep practicing.".to_string()
        } else {
            "Try again, listen closely to the reference audio.".to_string()
        };

        Self {
            score,
            recognized_text,
            expected_text,
            feedback,
            is_correct,
            language: language.to_string(),
        }
    }
}

/// Levenshtein (edit) distance between two strings, operating on Unicode
/// scalar values so multi-byte characters (kana, kanji) count as one edit.
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (curr[j - 1] + 1).min(prev[j] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// Normalize text for comparison: strips whitespace and lowercases (kana
/// width/variant normalization is left to callers, since it needs a lookup
/// table beyond simple char mapping).
pub fn normalize_text(text: &str, language: &str) -> String {
    crate::dictionary::normalize_for_language(text, language)
}

/// Score a pronunciation attempt by normalizing both texts and computing a
/// similarity percentage from their edit distance (0 = no match, 100 = identical).
pub fn score_pronunciation(recognized: &str, expected: &str, language: &str) -> PronunciationResult {
    let norm_recognized = normalize_text(recognized, language);
    let norm_expected = normalize_text(expected, language);

    let max_len = norm_recognized.chars().count().max(norm_expected.chars().count());
    let score = if max_len == 0 {
        100.0
    } else {
        let distance = levenshtein_distance(&norm_recognized, &norm_expected);
        (1.0 - (distance as f32 / max_len as f32)).max(0.0) * 100.0
    };

    PronunciationResult::new(score, recognized.to_string(), expected.to_string(), language)
}

/// How a single expected/recognized character relates to the other string,
/// from a character-level alignment of the two — see [`diff_chars`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOp {
    /// Present, unchanged, in both strings.
    Match,
    /// Present in both strings but differs (a substitution).
    Substitute,
    /// Present in `expected` but missing from `recognized`.
    Missing,
}

/// Character-level alignment between `recognized` and `expected`, classifying
/// every character of `expected` as [`DiffOp::Match`], [`DiffOp::Substitute`],
/// or [`DiffOp::Missing`], so a caller (e.g. the pronunciation UI) can
/// highlight exactly which part of the expected word was or wasn't heard,
/// rather than just showing a bare percentage. Alignment is the standard
/// Levenshtein edit-distance DP with a backtrace, operating on `char`s (not
/// bytes) so kana/kanji count as single units, same as
/// [`levenshtein_distance`].
pub fn diff_chars(recognized: &str, expected: &str) -> Vec<(char, DiffOp)> {
    let recognized: Vec<char> = recognized.chars().collect();
    let expected: Vec<char> = expected.chars().collect();
    let (m, n) = (recognized.len(), expected.len());

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if recognized[i - 1] == expected[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1).min(dp[i][j - 1] + 1).min(dp[i - 1][j - 1] + cost);
        }
    }

    let mut ops = Vec::with_capacity(n);
    let (mut i, mut j) = (m, n);
    while j > 0 {
        if i > 0 && dp[i][j] == dp[i - 1][j - 1] + if recognized[i - 1] == expected[j - 1] { 0 } else { 1 } {
            let op = if recognized[i - 1] == expected[j - 1] { DiffOp::Match } else { DiffOp::Substitute };
            ops.push((expected[j - 1], op));
            i -= 1;
            j -= 1;
        } else if i > 0 && dp[i][j] == dp[i - 1][j] + 1 {
            // An extra recognized character with no counterpart in expected —
            // doesn't map to any expected char, so it's dropped from this
            // per-expected-character view.
            i -= 1;
        } else {
            ops.push((expected[j - 1], DiffOp::Missing));
            j -= 1;
        }
    }
    ops.reverse();
    ops
}

/// Scores pronunciation by comparing recognized speech to expected text.
pub struct PronunciationScorer {
    language: String,
}

impl PronunciationScorer {
    pub fn new(language: &str) -> Self {
        Self { language: language.to_string() }
    }

    pub fn score(&self, recognized: &str, expected: &str) -> PronunciationResult {
        score_pronunciation(recognized, expected, &self.language)
    }
}

/// Split Japanese kana text into morae (the rhythmic unit used for Tier 2
/// phoneme-level comparison) — each mora is one kana, except small
/// "ゃゅょぁぃぅぇぉ" etc. which combine with the preceding kana (e.g. "きゃ" is one mora).
pub fn segment_japanese_morae(kana: &str) -> Vec<String> {
    const SMALL_KANA: &[char] = &[
        'ゃ', 'ゅ', 'ょ', 'ぁ', 'ぃ', 'ぅ', 'ぇ', 'ぉ', 'ャ', 'ュ', 'ョ', 'ァ', 'ィ', 'ゥ', 'ェ', 'ォ',
    ];

    let mut morae = Vec::new();
    for c in kana.chars() {
        if SMALL_KANA.contains(&c) {
            if let Some(last) = morae.last_mut() {
                let combined: &mut String = last;
                combined.push(c);
                continue;
            }
        }
        morae.push(c.to_string());
    }
    morae
}

/// Segment text into its phoneme-adjacent units (morae) for Tier 2 scoring.
/// `language` is currently always "ja" — kept as a parameter for consistency
/// with `normalize_text`/`score_pronunciation`, and since other languages may
/// be added later.
pub fn segment_units(text: &str, _language: &str) -> Vec<String> {
    segment_japanese_morae(text)
}

/// Edit distance over a sequence of string units (morae/syllables) rather
/// than individual characters, so e.g. mistaking "きゃ" for "きや" (which
/// differ by a small-kana substitution, one unit) doesn't get penalized as
/// two character edits.
fn unit_levenshtein_distance(a: &[String], b: &[String]) -> usize {
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (curr[j - 1] + 1).min(prev[j] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Tier 2 pronunciation scoring: compares recognized and expected text at
/// the phoneme/morae-unit level (via [`segment_units`]) instead of raw
/// characters, which better reflects how mispronunciations actually work
/// (e.g. swapping one mora matters more than swapping one character within it).
pub fn score_pronunciation_tier2(recognized: &str, expected: &str, language: &str) -> PronunciationResult {
    let norm_recognized = normalize_text(recognized, language);
    let norm_expected = normalize_text(expected, language);

    let recognized_units = segment_units(&norm_recognized, language);
    let expected_units = segment_units(&norm_expected, language);

    let max_len = recognized_units.len().max(expected_units.len());
    let score = if max_len == 0 {
        100.0
    } else {
        let distance = unit_levenshtein_distance(&recognized_units, &expected_units);
        (1.0 - (distance as f32 / max_len as f32)).max(0.0) * 100.0
    };

    PronunciationResult::new(score, recognized.to_string(), expected.to_string(), language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("うけとる", "うけとる"), 0);
        assert_eq!(levenshtein_distance("うけとる", "たべる"), 3);
    }

    #[test]
    fn test_diff_chars_exact_match() {
        let ops = diff_chars("うけとる", "うけとる");
        assert!(ops.iter().all(|(_, op)| *op == DiffOp::Match));
        assert_eq!(ops.iter().map(|(c, _)| *c).collect::<String>(), "うけとる");
    }

    #[test]
    fn test_diff_chars_missing_suffix() {
        // Recognizer only caught the first two morae.
        let ops = diff_chars("うけ", "うけとる");
        assert_eq!(ops, vec![
            ('う', DiffOp::Match),
            ('け', DiffOp::Match),
            ('と', DiffOp::Missing),
            ('る', DiffOp::Missing),
        ]);
    }

    #[test]
    fn test_diff_chars_substitution() {
        let ops = diff_chars("うけとろ", "うけとる");
        assert_eq!(ops.last(), Some(&('る', DiffOp::Substitute)));
    }

    #[test]
    fn test_score_pronunciation_exact_match() {
        let result = score_pronunciation("うけとる", "うけとる", "ja");
        assert_eq!(result.score, 100.0);
        assert!(result.is_correct);
    }

    #[test]
    fn test_score_pronunciation_mismatch() {
        let result = score_pronunciation("たべる", "うけとる", "ja");
        assert!(result.score < 50.0);
        assert!(!result.is_correct);
    }

    #[test]
    fn test_segment_japanese_morae() {
        assert_eq!(segment_japanese_morae("うけとる"), vec!["う", "け", "と", "る"]);
        // きゃ is one mora, not two
        assert_eq!(segment_japanese_morae("きゃく"), vec!["きゃ", "く"]);
    }

    #[test]
    fn test_pronunciation_scorer_struct() {
        let scorer = PronunciationScorer::new("ja");
        let result = scorer.score("うけとる", "うけとる");
        assert!(result.is_correct);
    }

    #[test]
    fn test_score_pronunciation_tier2_exact_match() {
        let result = score_pronunciation_tier2("うけとる", "うけとる", "ja");
        assert_eq!(result.score, 100.0);
    }

    #[test]
    fn test_score_pronunciation_tier2_one_mora_off() {
        // "きゃく" vs "きやく": differs by one mora unit ("きゃ" vs "き"+"や"),
        // so tier2 (unit-level) sees this differently than plain char-level distance.
        let result = score_pronunciation_tier2("きゃく", "きやく", "ja");
        assert!(result.score < 100.0);
    }

    #[test]
    fn test_segment_units() {
        assert_eq!(segment_units("うけとる", "ja"), vec!["う", "け", "と", "る"]);
    }
}
