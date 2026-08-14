//! Reading material: Aozora Bunko (青空文庫) classic Japanese texts.
//!
//! Aozora Bunko publishes a bulk CSV catalog (author, title, NDC
//! classification code, and a direct download URL per work) plus per-work
//! `.txt` files marked up with a simple ruby (furigana) notation. This
//! module turns that CSV into a browsable [`BookCatalogEntry`] list (see
//! `installer::install_aozora_catalog`) and a single work's raw text into
//! [`BookText`] (see `installer::fetch_book`), ready for the reader widget
//! to render and hit-test against.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One work listed in the Aozora Bunko catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookCatalogEntry {
    /// Aozora's own work id (作品ID) — used as the cache key for the
    /// downloaded/parsed text (see `installer::fetch_book`).
    pub id: String,
    pub title: String,
    /// Author's family + given name (姓 + 名), joined with a space.
    pub author: String,
    /// Coarse genre label resolved from the work's NDC classification code
    /// (分類番号) — see `genre_for_ndc`. `None` for codes outside the
    /// Japanese-literature range this maps, grouped as "Other" by the UI.
    pub genre: Option<String>,
    /// Direct URL to the work's Shift-JIS `.txt`-in-`.zip` file (the
    /// catalog's テキストファイルURL column), fetched lazily when the work
    /// is opened rather than bundled into the catalog itself.
    pub text_url: String,
}

/// A run of text within a paragraph: either plain text, or a base/furigana
/// pair from Aozora's ruby markup (e.g. `｜漢字《かんじ》`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextRun {
    Plain(String),
    Ruby { base: String, reading: String },
}

/// One paragraph of a book — in Aozora's plain-text format, one non-blank
/// line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BookParagraph {
    pub runs: Vec<TextRun>,
}

/// A fully parsed book, ready for the reader widget.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BookText {
    pub paragraphs: Vec<BookParagraph>,
}

/// Resolve a coarse genre label from an NDC (Nippon Decimal Classification)
/// code — only the literature class (`9xy`) Aozora catalogs almost
/// everything under. Aozora's own `分類番号` values aren't a bare code: they
/// look like `"NDC 913"`, `"NDC K913"` (a `K` marks children's literature —
/// still filed under the same numeric code), or occasionally empty, so this
/// pulls out the first run of digits rather than assuming a fixed prefix
/// (verified against a live download of the real catalog — see
/// `installer::tests::test_parse_aozora_catalog_csv_builds_entries_by_header_name`
/// for the parser this feeds).
///
/// Within `9xy`, `x` is the literature's language of origin (1=Japanese,
/// 2=Chinese, 3=English, 4=German, 5=French, ...) and `y` is the genre —
/// genre is what this groups by, so a Japanese novel (913) and a translated
/// English one (933) land in the same "Novels & Stories" bucket. Codes
/// outside the literature class, or missing/malformed, resolve to `None`;
/// the Books tab groups those under "Other" the same way the Vocabulary tab
/// groups entries with no level under "Uncategorized" (see
/// `vocabulary::group_by_level`).
pub fn genre_for_ndc(code: &str) -> Option<String> {
    let digits: String = code.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    let bytes = digits.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'9' {
        return None;
    }
    let label = match bytes[2] {
        b'1' => "Poetry & Waka",
        b'2' => "Drama",
        b'3' => "Novels & Stories",
        b'4' => "Essays",
        b'5' => "Diaries & Travel Writing",
        b'6' => "Records & Reportage",
        b'8' => "Anthologies",
        _ => return None,
    };
    Some(label.to_string())
}

/// Strip Aozora's boilerplate: the front-matter legend block (bounded by two
/// `----...` separator lines, present on most but not all works) and the
/// 底本 (source edition) colophon footer that follows the actual text.
/// Heuristic (Aozora's plain-text format has no strict schema), matching the
/// convention other Aozora tooling (e.g. aozora-corpus-generator) uses.
fn strip_boilerplate(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();

    let is_separator = |line: &str| {
        let trimmed = line.trim();
        trimmed.len() >= 4 && trimmed.chars().all(|c| c == '-')
    };
    let dash_positions: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| is_separator(l)).map(|(i, _)| i).collect();
    let body_start = if dash_positions.len() >= 2 { dash_positions[1] + 1 } else { 0 };

    let body = &lines[body_start..];
    let body_end = body.iter().position(|l| l.trim_start().starts_with("底本：")).unwrap_or(body.len());

    body[..body_end].join("\n")
}

fn is_kanji(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}')
}

fn find_char(chars: &[char], from: usize, target: char) -> Option<usize> {
    chars[from..].iter().position(|&c| c == target).map(|p| from + p)
}

/// Parse one line of Aozora ruby markup into a sequence of [`TextRun`]s:
/// `｜base《reading》` (explicit base — the `｜` disambiguates where the
/// base run starts when it's not just the preceding kanji run), bare
/// `base《reading》` (base defaults to the maximal run of kanji immediately
/// before `《`), and `［＃...］` editorial annotations (stripped entirely —
/// these are notes to a human transcriber, not part of the text).
fn parse_paragraph(line: &str) -> BookParagraph {
    let chars: Vec<char> = line.chars().collect();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut plain = String::new();
    let mut i = 0;

    let flush = |plain: &mut String, runs: &mut Vec<TextRun>| {
        if !plain.is_empty() {
            runs.push(TextRun::Plain(std::mem::take(plain)));
        }
    };

    while i < chars.len() {
        match chars[i] {
            '［' if chars.get(i + 1) == Some(&'＃') => {
                // ［＃...］ editorial annotation — drop it entirely.
                match find_char(&chars, i + 1, '］') {
                    Some(end) => i = end + 1,
                    None => break, // unterminated — nothing useful left on this line
                }
            }
            '｜' => {
                // ｜base《reading》 — explicit base marker.
                let parsed =
                    find_char(&chars, i + 1, '《').and_then(|open| find_char(&chars, open + 1, '》').map(|close| (open, close)));
                match parsed {
                    Some((open, close)) => {
                        flush(&mut plain, &mut runs);
                        let base: String = chars[i + 1..open].iter().collect();
                        let reading: String = chars[open + 1..close].iter().collect();
                        runs.push(TextRun::Ruby { base, reading });
                        i = close + 1;
                    }
                    None => {
                        plain.push(chars[i]);
                        i += 1;
                    }
                }
            }
            '《' => {
                // base《reading》 — base is the trailing kanji run already in `plain`.
                match find_char(&chars, i + 1, '》') {
                    Some(close) => {
                        let mut base_len = 0;
                        while base_len < plain.chars().count() {
                            let c = plain.chars().rev().nth(base_len).unwrap();
                            if !is_kanji(c) {
                                break;
                            }
                            base_len += 1;
                        }
                        if base_len == 0 {
                            // No preceding kanji to attach to — not a ruby
                            // annotation after all, keep the brackets literal.
                            plain.push(chars[i]);
                            i += 1;
                            continue;
                        }
                        let split_at = plain.chars().count() - base_len;
                        let base: String = plain.chars().skip(split_at).collect();
                        plain.truncate(plain.char_indices().nth(split_at).map(|(b, _)| b).unwrap_or(plain.len()));
                        flush(&mut plain, &mut runs);
                        let reading: String = chars[i + 1..close].iter().collect();
                        runs.push(TextRun::Ruby { base, reading });
                        i = close + 1;
                    }
                    None => {
                        plain.push(chars[i]);
                        i += 1;
                    }
                }
            }
            c => {
                plain.push(c);
                i += 1;
            }
        }
    }
    flush(&mut plain, &mut runs);

    BookParagraph { runs }
}

/// Parse the JSON catalog `installer::install_aozora_catalog` writes to
/// `<books_dir>/catalog.json` back into a `Vec<BookCatalogEntry>` — kept
/// here (rather than parsed with `serde_json` directly by `langspark-gui`)
/// so JSON handling stays confined to `langspark-core`, matching every other
/// data source (see ARCHITECTURE.md's core/GUI split).
pub fn load_book_catalog(json: &str) -> Result<Vec<BookCatalogEntry>> {
    serde_json::from_str(json).context("failed to parse book catalog")
}

/// Parse a whole Aozora `.txt` file's contents (already Shift-JIS-decoded to
/// UTF-8) into a [`BookText`]: strips the header/footer boilerplate (see
/// `strip_boilerplate`), then treats each remaining non-blank line as one
/// paragraph (see `parse_paragraph`).
pub fn parse_book_text(raw: &str) -> BookText {
    let body = strip_boilerplate(raw);
    let paragraphs = body.lines().filter(|l| !l.trim().is_empty()).map(parse_paragraph).collect();
    BookText { paragraphs }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genre_for_ndc_maps_known_codes() {
        // Real-world formats, verified against a live download of Aozora's
        // catalog: a "NDC " prefix always present, sometimes a "K" marking
        // children's literature, and the genre digit (ones place) shared
        // across every language-of-origin hundred-block.
        assert_eq!(genre_for_ndc("NDC 913"), Some("Novels & Stories".to_string())); // Japanese novel
        assert_eq!(genre_for_ndc("NDC 933"), Some("Novels & Stories".to_string())); // translated English novel
        assert_eq!(genre_for_ndc("NDC K913"), Some("Novels & Stories".to_string())); // children's literature
        assert_eq!(genre_for_ndc("NDC 911"), Some("Poetry & Waka".to_string()));
        assert_eq!(genre_for_ndc("NDC 289"), None); // biography — outside the literature class
        assert_eq!(genre_for_ndc("NDC 400"), None); // outside the literature class
        assert_eq!(genre_for_ndc(""), None);
    }

    #[test]
    fn test_strip_boilerplate_removes_legend_and_colophon() {
        let text = "吾輩は猫である\n夏目漱石\n\
-------------------------------------------------------\n\
【テキスト中に現れる記号について】\n\
-------------------------------------------------------\n\
\n\
吾輩は猫である。名前はまだ無い。\n\
\n\
\n\
底本：「吾輩は猫である」新潮文庫\n\
1961（昭和36）年発行\n";

        let body = strip_boilerplate(text);
        assert!(body.contains("吾輩は猫である。名前はまだ無い。"));
        assert!(!body.contains("底本："));
        assert!(!body.contains("テキスト中に現れる記号"));
    }

    #[test]
    fn test_strip_boilerplate_without_legend_block_still_trims_colophon() {
        let text = "本文がここから始まる。\n\n底本：何かの本\n";
        let body = strip_boilerplate(text);
        assert_eq!(body.trim(), "本文がここから始まる。");
    }

    #[test]
    fn test_parse_paragraph_explicit_ruby() {
        let para = parse_paragraph("｜東京《とうきょう》に行く");
        assert_eq!(
            para.runs,
            vec![
                TextRun::Ruby { base: "東京".to_string(), reading: "とうきょう".to_string() },
                TextRun::Plain("に行く".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_paragraph_implicit_ruby_uses_trailing_kanji_run() {
        let para = parse_paragraph("私は猫《ねこ》です");
        assert_eq!(
            para.runs,
            vec![
                TextRun::Plain("私は".to_string()),
                TextRun::Ruby { base: "猫".to_string(), reading: "ねこ".to_string() },
                TextRun::Plain("です".to_string()),
            ]
        );
    }

    #[test]
    fn test_parse_paragraph_strips_editorial_annotation() {
        let para = parse_paragraph("吾輩は猫である［＃「猫」に傍点］。");
        assert_eq!(para.runs, vec![TextRun::Plain("吾輩は猫である。".to_string())]);
    }

    #[test]
    fn test_parse_paragraph_plain_text_unaffected() {
        let para = parse_paragraph("名前はまだ無い。");
        assert_eq!(para.runs, vec![TextRun::Plain("名前はまだ無い。".to_string())]);
    }

    #[test]
    fn test_load_book_catalog_round_trips_through_json() {
        let entries = vec![BookCatalogEntry {
            id: "1234".to_string(),
            title: "吾輩は猫である".to_string(),
            author: "夏目 漱石".to_string(),
            genre: Some("Novels & Stories".to_string()),
            text_url: "https://www.aozora.gr.jp/cards/000148/files/789.zip".to_string(),
        }];
        let json = serde_json::to_string(&entries).unwrap();
        let loaded = load_book_catalog(&json).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].title, "吾輩は猫である");
    }

    #[test]
    fn test_load_book_catalog_errors_on_malformed_json() {
        assert!(load_book_catalog("not json").is_err());
    }

    #[test]
    fn test_parse_book_text_skips_blank_lines() {
        let raw = "一行目。\n\n二行目。\n";
        let book = parse_book_text(raw);
        assert_eq!(book.paragraphs.len(), 2);
    }
}
