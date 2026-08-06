//! Dictionary integration module
//!
//! Loads and queries language-specific dictionaries. Japanese dictionaries
//! (JMdict, Kanjidic) come from the `scriptin/jmdict-simplified` JSON export;
//! Spanish uses a small custom JSON schema (see [`spanish`] module) since no
//! equivalently maintained JSON export exists — see design.md "Open Questions".

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
    /// Primary written form (kanji form for Japanese, headword for Spanish)
    pub word: String,
    /// Phonetic reading, if distinct from `word` (kana for Japanese)
    pub reading: Option<String>,
    /// Glosses/translations
    pub meanings: Vec<String>,
    /// Parts of speech (e.g. "adj-na", "verb")
    pub part_of_speech: Vec<String>,
    /// Proficiency level tag (JLPT level for Japanese, CEFR level for Spanish)
    pub level: Option<String>,
    /// Language code ("ja", "es")
    pub language: String,
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
}

#[derive(Debug, Deserialize)]
struct JmdictGloss {
    #[serde(default)]
    lang: Option<String>,
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

            VocabEntry {
                id: w.id,
                word,
                reading,
                meanings,
                part_of_speech,
                level: None,
                language: "ja".to_string(),
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
// Spanish dictionary (custom minimal schema)
// ---------------------------------------------------------------------

pub mod spanish {
    //! Minimal Spanish dictionary JSON schema.
    //!
    //! There is no `jmdict-simplified`-equivalent maintained JSON export for
    //! Spanish, so LangSpark defines its own small schema that can be produced
    //! from open sources (e.g. a Wiktionary extract) via an offline conversion
    //! script. Each entry is a flat JSON object:
    //! ```json
    //! {"word": "recibir", "reading": "re.θi.'βiɾ", "meanings": ["to receive"], "part_of_speech": ["verb"], "cefr_level": "B1"}
    //! ```
    use super::VocabEntry;
    use anyhow::{Context, Result};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct SpanishFile {
        entries: Vec<SpanishEntry>,
    }

    #[derive(Debug, Deserialize)]
    struct SpanishEntry {
        word: String,
        #[serde(default)]
        reading: Option<String>,
        #[serde(default)]
        meanings: Vec<String>,
        #[serde(default)]
        part_of_speech: Vec<String>,
        #[serde(default)]
        cefr_level: Option<String>,
    }

    /// Parse a LangSpark Spanish dictionary JSON document into vocabulary entries.
    pub fn load_spanish_dictionary(json: &str) -> Result<Vec<VocabEntry>> {
        let file: SpanishFile =
            serde_json::from_str(json).context("failed to parse Spanish dictionary JSON")?;

        Ok(file
            .entries
            .into_iter()
            .enumerate()
            .map(|(i, e)| VocabEntry {
                id: i.to_string(),
                word: e.word,
                reading: e.reading,
                meanings: e.meanings,
                part_of_speech: e.part_of_speech,
                level: e.cefr_level,
                language: "es".to_string(),
            })
            .collect())
    }
}

// ---------------------------------------------------------------------
// Fuzzy matching helpers (language-specific normalization)
// ---------------------------------------------------------------------

/// Normalize a query for matching against a given language's dictionary.
///
/// Japanese: lowercased, whitespace stripped (kana width differences are left
/// to the caller since converting hiragana/katakana requires a lookup table).
/// Spanish: lowercased and accented vowels folded to their plain form so
/// "recibir" matches a query typed without accents.
pub fn normalize_for_language(text: &str, language: &str) -> String {
    match language {
        "es" => text
            .to_lowercase()
            .chars()
            .map(|c| match c {
                'á' => 'a',
                'é' => 'e',
                'í' => 'i',
                'ó' => 'o',
                'ú' => 'u',
                'ü' => 'u',
                'ñ' => 'n',
                other => other,
            })
            .collect(),
        _ => text.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase(),
    }
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
}

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

    /// Load the Spanish dictionary.
    pub fn load_spanish(&mut self, spanish_json: &str, version: Option<&str>) -> Result<()> {
        let entries = spanish::load_spanish_dictionary(spanish_json)?;
        self.entries.insert("es".to_string(), entries);
        if let Some(v) = version {
            self.versions.insert("es".to_string(), v.to_string());
        }
        Ok(())
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
                "sense": [{"partOfSpeech": ["v5r", "vt"], "gloss": [{"lang": "eng", "text": "to receive"}]}]
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

    const SPANISH_FIXTURE: &str = r#"{
        "entries": [
            {"word": "recibir", "reading": "re.θi.'βiɾ", "meanings": ["to receive"], "part_of_speech": ["verb"], "cefr_level": "B1"},
            {"word": "comer", "meanings": ["to eat"], "part_of_speech": ["verb"], "cefr_level": "A1"}
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
    fn test_load_spanish_dictionary() {
        let entries = spanish::load_spanish_dictionary(SPANISH_FIXTURE).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].word, "recibir");
        assert_eq!(entries[0].level, Some("B1".to_string()));
        assert_eq!(entries[0].language, "es");
    }

    #[test]
    fn test_normalize_for_language_spanish_accents() {
        assert_eq!(normalize_for_language("Recibí", "es"), "recibi");
        assert_eq!(normalize_for_language("mañana", "es"), "manana");
    }

    #[test]
    fn test_dictionary_manager_search_and_filter() {
        let mut manager = DictionaryManager::new();
        manager.load_japanese(JMDICT_FIXTURE, Some("3.5.0")).unwrap();
        manager.load_spanish(SPANISH_FIXTURE, Some("1.0")).unwrap();

        assert!(manager.is_loaded("ja"));
        assert!(manager.is_loaded("es"));
        assert!(!manager.is_loaded("fr"));

        let results = manager.search("ja", "うけとる");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].word, "受け取る");

        // Fuzzy: searching without accent still matches "recibir"'s meaning
        let results = manager.search("es", "receive");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].word, "recibir");

        let filtered = manager.filter(
            "es",
            &VocabFilter {
                level: Some("A1".to_string()),
                part_of_speech: None,
            },
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].word, "comer");
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
