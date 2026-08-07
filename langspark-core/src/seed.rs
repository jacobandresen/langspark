//! Standard Japanese school vocabulary (JLPT N5-N3), embedded in the binary
//! and seeded into a fresh database on first startup (see `langspark-gui`'s
//! `AppState::open`, which calls this only when no Japanese vocabulary
//! exists yet) so the Vocabulary/Review tabs aren't empty on first launch.
//!
//! Sourced from general JLPT-vocabulary knowledge, not a downloaded or
//! independently verified official list — N5/N4 are highly standardized
//! across sources; N3 is somewhat less uniform between vendors' lists but
//! still solid common-core vocabulary.

use crate::database::Database;
use anyhow::Result;

const JA_SCHOOL_VOCAB_TSV: &str = include_str!("../data/ja_school_vocab.tsv");

struct SeedWord<'a> {
    word: &'a str,
    reading: &'a str,
    meaning: &'a str,
    level: &'a str,
    part_of_speech: &'a str,
}

fn parse_ja_school_vocab(tsv: &str) -> Vec<SeedWord<'_>> {
    tsv.lines()
        .skip(1) // header
        .filter(|line| !line.is_empty())
        .filter_map(|line| match line.split('\t').collect::<Vec<&str>>()[..] {
            [word, reading, meaning, level, part_of_speech] => {
                Some(SeedWord { word, reading, meaning, level, part_of_speech })
            }
            _ => None,
        })
        .collect()
}

/// Number of words in the embedded Japanese school vocabulary list.
pub fn ja_school_vocabulary_len() -> usize {
    parse_ja_school_vocab(JA_SCHOOL_VOCAB_TSV).len()
}

/// Insert the embedded Japanese school vocabulary (JLPT N5-N3) plus a
/// matching "new" SRS card per word (so they're immediately due for
/// review), in a single transaction — inserting ~600 words one
/// autocommitted statement at a time (the normal repository-based path)
/// takes minutes under SQLite's default per-statement fsync; batching into
/// one transaction takes well under a second.
///
/// Intended to be called only when the vocabulary table is empty for "ja"
/// (see `AppState::open`) — it doesn't check for existing words itself, so
/// calling it twice would create duplicates.
pub fn seed_ja_school_vocabulary(db: &Database) -> Result<usize> {
    let words = parse_ja_school_vocab(JA_SCHOOL_VOCAB_TSV);

    let mut conn = db.conn();
    let tx = conn.transaction()?;
    for w in &words {
        tx.execute(
            "INSERT INTO vocabulary (word, reading, meaning, language, level, part_of_speech, tags)
             VALUES (?, ?, ?, 'ja', ?, ?, 'school')",
            rusqlite::params![w.word, w.reading, w.meaning, w.level, w.part_of_speech],
        )?;
        let vocab_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO srs_cards (vocab_id, card_type, state, repetitions, ease_factor, interval_days, language, stability, difficulty)
             VALUES (?, 'vocabulary', 'new', 0, 2.5, 0, 'ja', 0, 0)",
            rusqlite::params![vocab_id],
        )?;
    }
    tx.commit()?;

    Ok(words.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ja_school_vocab_has_expected_shape() {
        let words = parse_ja_school_vocab(JA_SCHOOL_VOCAB_TSV);
        assert!(words.len() > 500, "expected 500+ words, got {}", words.len());
        assert!(words.iter().all(|w| !w.word.is_empty() && !w.reading.is_empty() && !w.meaning.is_empty()));
        assert!(words.iter().all(|w| matches!(w.level, "N5" | "N4" | "N3")));
    }

    #[test]
    fn test_seed_ja_school_vocabulary_inserts_words_and_cards() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let db = Database::open(temp.path()).unwrap();
        crate::database::initialize_schema(&db.conn()).unwrap();
        crate::database::run_migrations(&mut db.conn(), &crate::database::default_migrations()).unwrap();

        let inserted = seed_ja_school_vocabulary(&db).unwrap();
        assert_eq!(inserted, ja_school_vocabulary_len());

        let vocab_count: i64 = db.conn().query_row("SELECT COUNT(*) FROM vocabulary", [], |r| r.get(0)).unwrap();
        let card_count: i64 = db.conn().query_row("SELECT COUNT(*) FROM srs_cards", [], |r| r.get(0)).unwrap();
        assert_eq!(vocab_count as usize, inserted);
        assert_eq!(card_count as usize, inserted);
    }
}
