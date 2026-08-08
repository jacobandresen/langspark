//! Dictionary integration module
//!
//! Loads and queries the Japanese dictionary (JMdict, Kanjidic), sourced from
//! the `scriptin/jmdict-simplified` JSON export.

use crate::repositories::KanjiEntry;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trait for dictionary operations
pub trait Dictionary {
    /// Look up a word
    fn lookup(&self, query: &str) -> Result<Vec<VocabEntry>>;
}

/// A vocabulary entry as returned by a dictionary lookup (as opposed to
/// [`crate::repositories::VocabularyEntry`], which is a user-owned database row).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VocabEntry {
    /// Dictionary-assigned entry ID
    pub id: String,
    /// Primary written form (kanji form)
    pub word: String,
    /// Phonetic reading, if distinct from `word` (kana)
    pub reading: Option<String>,
    /// Glosses/translations
    pub meanings: Vec<String>,
    /// Parts of speech (e.g. "adj-na", "verb")
    pub part_of_speech: Vec<String>,
    /// Proficiency level tag (JLPT level)
    pub level: Option<String>,
    /// Language code (currently always "ja")
    pub language: String,
    /// Example sentences using this word (Tatoeba-sourced, via the
    /// `jmdict-examples-eng` dictionary asset; see `installer.rs`). Empty if
    /// this word simply has none in the source data (most words don't —
    /// only ~13% of JMdict entries have any).
    pub examples: Vec<ExampleSentence>,
}

/// A Japanese/English example sentence pair for a vocabulary word, sourced
/// from the Tatoeba corpus via JMdict's `jmdict-examples-eng` asset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExampleSentence {
    pub japanese: String,
    pub english: String,
}

/// Supplemental example sentences from the full Tatoeba Japanese-English
/// sentence-pair corpus (see `installer::install_tatoeba_examples`) — not
/// just the much smaller subset JMdict links to specific dictionary entries
/// (`jmdict-examples-eng`, ~13% of entries). Used as a fallback by
/// `DictionaryManager::examples_for` when a word has no JMdict-linked
/// examples, since Tatoeba's raw sentences are user-submitted with no
/// curation, while JMdict's own subset is generally cleaner.
pub struct TatoebaExamples {
    pairs: Vec<ExampleSentence>,
}

impl TatoebaExamples {
    /// Parse `japanese\tenglish` lines (one sentence pair per line), as
    /// written by `installer::install_tatoeba_examples`. Malformed lines are
    /// skipped rather than failing the whole load.
    pub fn load(tsv: &str) -> Self {
        let pairs = tsv
            .lines()
            .filter_map(|line| {
                let (japanese, english) = line.split_once('\t')?;
                Some(ExampleSentence { japanese: japanese.to_string(), english: english.to_string() })
            })
            .collect();
        Self { pairs }
    }

    /// Up to `limit` sentences containing `word` as a substring, shortest
    /// first (shorter sentences tend to be simpler and more useful to a
    /// learner). Substring matching (rather than proper word-boundary/
    /// morphological matching, which Japanese's lack of spaces makes
    /// nontrivial) means this can occasionally match `word` as part of a
    /// longer, unrelated word.
    pub fn examples_for(&self, word: &str, limit: usize) -> Vec<ExampleSentence> {
        let mut matches: Vec<&ExampleSentence> = self.pairs.iter().filter(|p| p.japanese.contains(word)).collect();
        matches.sort_by_key(|p| p.japanese.chars().count());
        matches.into_iter().take(limit).cloned().collect()
    }
}

// ---------------------------------------------------------------------
// JMdict (jmdict-simplified words.json format)
// ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JmdictFile {
    #[serde(default)]
    version: Option<String>,
    words: Vec<JmdictEntry>,
}

#[derive(Debug, Deserialize)]
struct JmdictEntry {
    id: String,
    #[serde(default)]
    kanji: Vec<JmdictKanji>,
    #[serde(default)]
    kana: Vec<JmdictKana>,
    #[serde(default)]
    sense: Vec<JmdictSense>,
}

#[derive(Debug, Deserialize)]
struct JmdictKanji {
    text: String,
}

#[derive(Debug, Deserialize)]
struct JmdictKana {
    text: String,
}

#[derive(Debug, Deserialize)]
struct JmdictSense {
    #[serde(default, rename = "partOfSpeech")]
    part_of_speech: Vec<String>,
    #[serde(default)]
    gloss: Vec<JmdictGloss>,
    /// Only present in the `jmdict-examples-eng` asset; absent (defaults to
    /// empty) when parsing the plain `jmdict-eng` format, so this struct
    /// works for either.
    #[serde(default)]
    examples: Vec<JmdictExample>,
}

#[derive(Debug, Deserialize)]
struct JmdictGloss {
    #[serde(default)]
    lang: Option<String>,
    text: String,
}

#[derive(Debug, Deserialize)]
struct JmdictExample {
    #[serde(default)]
    sentences: Vec<JmdictExampleSentence>,
}

#[derive(Debug, Deserialize)]
struct JmdictExampleSentence {
    lang: String,
    text: String,
}

/// Parse a `jmdict-simplified` `words.json` document into vocabulary entries.
pub fn load_jmdict(json: &str) -> Result<Vec<VocabEntry>> {
    let file: JmdictFile = serde_json::from_str(json).context("failed to parse JMdict JSON")?;

    let entries = file
        .words
        .into_iter()
        .map(|w| {
            let word = w
                .kanji
                .first()
                .map(|k| k.text.clone())
                .or_else(|| w.kana.first().map(|k| k.text.clone()))
                .unwrap_or_default();
            let reading = w.kana.first().map(|k| k.text.clone());
            let meanings: Vec<String> = w
                .sense
                .iter()
                .flat_map(|s| s.gloss.iter())
                .filter(|g| g.lang.as_deref().unwrap_or("eng") == "eng")
                .map(|g| g.text.clone())
                .collect();
            let part_of_speech: Vec<String> = w
                .sense
                .iter()
                .flat_map(|s| s.part_of_speech.iter().cloned())
                .collect();
            let examples: Vec<ExampleSentence> = w
                .sense
                .iter()
                .flat_map(|s| s.examples.iter())
                .filter_map(|ex| {
                    let japanese = ex.sentences.iter().find(|s| s.lang == "jpn")?.text.clone();
                    let english = ex.sentences.iter().find(|s| s.lang == "eng")?.text.clone();
                    Some(ExampleSentence { japanese, english })
                })
                .collect();

            VocabEntry {
                id: w.id,
                word,
                reading,
                meanings,
                part_of_speech,
                level: None,
                language: "ja".to_string(),
                examples,
            }
        })
        .collect();

    let _ = file.version; // reserved for dictionary version checks (see check_for_update)
    Ok(entries)
}

// ---------------------------------------------------------------------
// Kanjidic (jmdict-simplified kanjidic.json format)
// ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct KanjidicFile {
    characters: Vec<KanjidicEntry>,
}

#[derive(Debug, Deserialize)]
struct KanjidicEntry {
    literal: String,
    #[serde(default)]
    misc: KanjidicMisc,
    #[serde(default, rename = "readingMeaning")]
    reading_meaning: Option<KanjidicReadingMeaning>,
}

#[derive(Debug, Deserialize, Default)]
struct KanjidicMisc {
    #[serde(default)]
    grade: Option<i32>,
    #[serde(default, rename = "strokeCounts")]
    stroke_counts: Vec<i32>,
    #[serde(default, rename = "jlptLevel")]
    jlpt_level: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct KanjidicReadingMeaning {
    #[serde(default)]
    groups: Vec<KanjidicGroup>,
}

#[derive(Debug, Deserialize)]
struct KanjidicGroup {
    #[serde(default)]
    readings: Vec<KanjidicReading>,
    #[serde(default)]
    meanings: Vec<KanjidicMeaning>,
}

#[derive(Debug, Deserialize)]
struct KanjidicReading {
    #[serde(rename = "type")]
    reading_type: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct KanjidicMeaning {
    #[serde(default)]
    lang: Option<String>,
    value: String,
}

/// Parse a `jmdict-simplified` `kanjidic.json` document into kanji entries.
pub fn load_kanjidic(json: &str) -> Result<Vec<KanjiEntry>> {
    let file: KanjidicFile = serde_json::from_str(json).context("failed to parse Kanjidic JSON")?;

    let entries = file
        .characters
        .into_iter()
        .map(|c| {
            let mut on_readings = Vec::new();
            let mut kun_readings = Vec::new();
            let mut meanings = Vec::new();

            if let Some(rm) = &c.reading_meaning {
                for group in &rm.groups {
                    for r in &group.readings {
                        match r.reading_type.as_str() {
                            "ja_on" => on_readings.push(r.value.clone()),
                            "ja_kun" => kun_readings.push(r.value.clone()),
                            _ => {}
                        }
                    }
                    for m in &group.meanings {
                        if m.lang.as_deref().unwrap_or("en") == "en" {
                            meanings.push(m.value.clone());
                        }
                    }
                }
            }

            KanjiEntry {
                id: None,
                character: c.literal,
                on_readings: Some(on_readings.join("; ")),
                kun_readings: Some(kun_readings.join("; ")),
                meanings: meanings.join("; "),
                stroke_count: c.misc.stroke_counts.first().copied(),
                radical: None,
                jlpt_level: c.misc.jlpt_level,
                grade: c.misc.grade,
                language: "ja".to_string(),
                created_at: None,
            }
        })
        .collect();

    Ok(entries)
}

// ---------------------------------------------------------------------
// Fuzzy matching helpers (language-specific normalization)
// ---------------------------------------------------------------------

/// Normalize a query for matching against a language's dictionary: lowercased,
/// whitespace stripped (kana width differences are left to the caller since
/// converting hiragana/katakana requires a lookup table). `language` is
/// currently always "ja" — kept as a parameter since normalization is
/// inherently per-language and other languages may be added later.
pub fn normalize_for_language(text: &str, _language: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase()
}

// ---------------------------------------------------------------------
// DictionaryManager
// ---------------------------------------------------------------------

/// Filter criteria for narrowing a dictionary search.
#[derive(Debug, Clone, Default)]
pub struct VocabFilter {
    pub level: Option<String>,
    pub part_of_speech: Option<String>,
}

/// Metadata about a loaded dictionary, used for update checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryVersion {
    pub language: String,
    pub version: String,
}

/// Manages loaded dictionaries, keyed by language code. Loading is cached in
/// memory per language so switching between two already-loaded languages
/// doesn't re-parse JSON.
#[derive(Default)]
pub struct DictionaryManager {
    entries: HashMap<String, Vec<VocabEntry>>,
    versions: HashMap<String, String>,
    tatoeba: HashMap<String, TatoebaExamples>,
}

/// Cap on how many Tatoeba-sourced sentences `examples_for` falls back to
/// per word, keeping the vocabulary dialog's example list from being
/// dominated by uncurated sentences once the (usually better) JMdict-linked
/// ones run out.
const TATOEBA_FALLBACK_LIMIT: usize = 3;

impl DictionaryManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load Japanese dictionaries (JMdict + Kanjidic word list only; kanji
    /// entries are handled separately via [`load_kanjidic`] and the kanji repository).
    pub fn load_japanese(&mut self, jmdict_json: &str, version: Option<&str>) -> Result<()> {
        let entries = load_jmdict(jmdict_json)?;
        self.entries.insert("ja".to_string(), entries);
        if let Some(v) = version {
            self.versions.insert("ja".to_string(), v.to_string());
        }
        Ok(())
    }

    /// Load supplemental Tatoeba example sentences for `language` (currently
    /// meaningful for Japanese only — see `installer::install_tatoeba_examples`).
    pub fn load_tatoeba_examples(&mut self, language: &str, tsv: &str) {
        self.tatoeba.insert(language.to_string(), TatoebaExamples::load(tsv));
    }

    /// Whether a dictionary is already loaded (and cached) for a language.
    pub fn is_loaded(&self, language: &str) -> bool {
        self.entries.contains_key(language)
    }

    /// Search entries for a language by word, reading, or meaning, with fuzzy
    /// (accent/case-insensitive) matching.
    pub fn search(&self, language: &str, query: &str) -> Vec<&VocabEntry> {
        let normalized_query = normalize_for_language(query, language);
        self.entries
            .get(language)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| {
                        normalize_for_language(&e.word, language).contains(&normalized_query)
                            || e.reading
                                .as_deref()
                                .map(|r| normalize_for_language(r, language).contains(&normalized_query))
                                .unwrap_or(false)
                            || e.meanings
                                .iter()
                                .any(|m| m.to_lowercase().contains(&normalized_query))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Example sentences for a word, matched by exact form (kanji or reading)
    /// against the loaded dictionary — used by the vocabulary detail dialog,
    /// which only has the saved word text (not a dictionary entry id) to
    /// look up against. Falls back to `TatoebaExamples` (if loaded — see
    /// `load_tatoeba_examples`) when JMdict's own much smaller curated
    /// example subset has nothing for this word, which is most words: only
    /// ~13% of JMdict entries have any.
    pub fn examples_for(&self, language: &str, word: &str) -> Vec<ExampleSentence> {
        let from_dict: Vec<ExampleSentence> = self
            .entries
            .get(language)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| e.word == word || e.reading.as_deref() == Some(word))
                    .flat_map(|e| e.examples.iter().cloned())
                    .collect()
            })
            .unwrap_or_default();

        if !from_dict.is_empty() {
            return from_dict;
        }

        self.tatoeba.get(language).map(|t| t.examples_for(word, TATOEBA_FALLBACK_LIMIT)).unwrap_or_default()
    }

    /// Filter entries for a language by proficiency level and/or part of speech.
    pub fn filter(&self, language: &str, filter: &VocabFilter) -> Vec<&VocabEntry> {
        self.entries
            .get(language)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| {
                        filter
                            .level
                            .as_ref()
                            .map(|lvl| e.level.as_deref() == Some(lvl.as_str()))
                            .unwrap_or(true)
                            && filter
                                .part_of_speech
                                .as_ref()
                                .map(|pos| e.part_of_speech.iter().any(|p| p == pos))
                                .unwrap_or(true)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The version string recorded when a language's dictionary was loaded, if any.
    pub fn version(&self, language: &str) -> Option<&str> {
        self.versions.get(language).map(String::as_str)
    }

    /// Whether `latest_version` differs from the currently loaded version for `language`.
    /// Returns `true` (update available) if no version has been recorded yet.
    pub fn check_for_update(&self, language: &str, latest_version: &str) -> bool {
        match self.version(language) {
            Some(current) => current != latest_version,
            None => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JMDICT_FIXTURE: &str = r#"{
        "version": "3.5.0",
        "words": [
            {
                "id": "1",
                "kanji": [{"common": true, "text": "受け取る", "tags": []}],
                "kana": [{"common": true, "text": "うけとる", "tags": [], "appliesToKanji": ["*"]}],
                "sense": [{
                    "partOfSpeech": ["v5r", "vt"],
                    "gloss": [{"lang": "eng", "text": "to receive"}],
                    "examples": [{
                        "sentences": [
                            {"lang": "jpn", "text": "彼はプレゼントを受け取った。"},
                            {"lang": "eng", "text": "He received the present."}
                        ],
                        "source": {"type": "tatoeba", "value": "1"},
                        "text": "受け取る"
                    }]
                }]
            },
            {
                "id": "2",
                "kanji": [],
                "kana": [{"common": true, "text": "たべる", "tags": [], "appliesToKanji": []}],
                "sense": [{"partOfSpeech": ["v1"], "gloss": [{"lang": "eng", "text": "to eat"}]}]
            }
        ]
    }"#;

    const KANJIDIC_FIXTURE: &str = r#"{
        "characters": [
            {
                "literal": "受",
                "misc": {"grade": 3, "strokeCounts": [8], "jlptLevel": 3},
                "readingMeaning": {
                    "groups": [{
                        "readings": [{"type": "ja_on", "value": "ジュ"}, {"type": "ja_kun", "value": "う.ける"}],
                        "meanings": [{"lang": "en", "value": "receive"}, {"lang": "en", "value": "accept"}]
                    }]
                }
            }
        ]
    }"#;

    #[test]
    fn test_load_jmdict() {
        let entries = load_jmdict(JMDICT_FIXTURE).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].word, "受け取る");
        assert_eq!(entries[0].reading, Some("うけとる".to_string()));
        assert_eq!(entries[0].meanings, vec!["to receive"]);
        assert_eq!(entries[0].language, "ja");
        // Word with no kanji form falls back to kana
        assert_eq!(entries[1].word, "たべる");
    }

    #[test]
    fn test_load_jmdict_parses_example_sentences() {
        let entries = load_jmdict(JMDICT_FIXTURE).unwrap();
        assert_eq!(
            entries[0].examples,
            vec![ExampleSentence {
                japanese: "彼はプレゼントを受け取った。".to_string(),
                english: "He received the present.".to_string(),
            }]
        );
        // Word with no examples in the source data gets an empty list, not an error.
        assert!(entries[1].examples.is_empty());
    }

    #[test]
    fn test_load_kanjidic() {
        let entries = load_kanjidic(KANJIDIC_FIXTURE).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].character, "受");
        assert_eq!(entries[0].on_readings, Some("ジュ".to_string()));
        assert_eq!(entries[0].kun_readings, Some("う.ける".to_string()));
        assert_eq!(entries[0].meanings, "receive; accept");
        assert_eq!(entries[0].jlpt_level, Some(3));
    }

    #[test]
    fn test_normalize_for_language_strips_whitespace_and_lowercases() {
        assert_eq!(normalize_for_language("うけとる ", "ja"), "うけとる");
        assert_eq!(normalize_for_language("Foo Bar", "ja"), "foobar");
    }

    #[test]
    fn test_dictionary_manager_search_and_filter() {
        let mut manager = DictionaryManager::new();
        manager.load_japanese(JMDICT_FIXTURE, Some("3.5.0")).unwrap();

        assert!(manager.is_loaded("ja"));
        assert!(!manager.is_loaded("es"));

        let results = manager.search("ja", "うけとる");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].word, "受け取る");

        let filtered = manager.filter(
            "ja",
            &VocabFilter {
                level: None,
                part_of_speech: Some("v1".to_string()),
            },
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].word, "たべる");
    }

    #[test]
    fn test_dictionary_manager_examples_for() {
        let mut manager = DictionaryManager::new();
        manager.load_japanese(JMDICT_FIXTURE, Some("3.5.0")).unwrap();

        let examples = manager.examples_for("ja", "受け取る");
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].english, "He received the present.");

        // Word with no examples in the source data, and no Tatoeba fallback loaded.
        assert!(manager.examples_for("ja", "たべる").is_empty());
        // No exact match.
        assert!(manager.examples_for("ja", "食べる").is_empty());
        // Nothing loaded for this language.
        assert!(manager.examples_for("es", "recibir").is_empty());

        // Matches by reading too, not just the kanji/primary word form.
        assert_eq!(manager.examples_for("ja", "うけとる").len(), 1);
    }

    #[test]
    fn test_dictionary_manager_examples_for_falls_back_to_tatoeba() {
        let mut manager = DictionaryManager::new();
        manager.load_japanese(JMDICT_FIXTURE, Some("3.5.0")).unwrap();
        manager.load_tatoeba_examples("ja", "たべるまえに、てをあらう。\tWash your hands before eating.\n");

        // JMdict itself has no examples for たべる, so the Tatoeba fallback kicks in.
        let examples = manager.examples_for("ja", "たべる");
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].english, "Wash your hands before eating.");

        // Words with a JMdict-linked example don't consult Tatoeba at all —
        // 受け取る's fixture example, not anything from the Tatoeba corpus.
        let examples = manager.examples_for("ja", "受け取る");
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].english, "He received the present.");
    }

    #[test]
    fn test_tatoeba_examples_matches_substring_shortest_first() {
        let tatoeba = TatoebaExamples::load(
            "食べることが好きです。\tI like eating.\n食べる。\tEat.\n関係ない文。\tUnrelated sentence.\n",
        );
        let matches = tatoeba.examples_for("食べる", 10);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].english, "Eat."); // shorter sentence first
        assert_eq!(matches[1].english, "I like eating.");
    }

    #[test]
    fn test_tatoeba_examples_respects_limit() {
        let tatoeba = TatoebaExamples::load("食べる。\tA.\n食べるよ。\tB.\n食べるね。\tC.\n");
        assert_eq!(tatoeba.examples_for("食べる", 2).len(), 2);
    }

    #[test]
    fn test_dictionary_manager_version_checking() {
        let mut manager = DictionaryManager::new();
        assert!(manager.check_for_update("ja", "3.5.0")); // nothing loaded yet

        manager.load_japanese(JMDICT_FIXTURE, Some("3.5.0")).unwrap();
        assert!(!manager.check_for_update("ja", "3.5.0"));
        assert!(manager.check_for_update("ja", "3.6.0"));
    }
}
