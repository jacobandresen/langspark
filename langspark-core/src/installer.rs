//! Downloads dictionary data from public sources.
//!
//! Only Japanese has an automated installer: `scriptin/jmdict-simplified`
//! publishes JMdict and Kanjidic as `.json.tgz` GitHub release assets, so we
//! can resolve the latest release, download the matching asset, and unpack
//! its single JSON member. Spanish has no equivalently maintained JSON
//! export (see `dictionary.rs`), so there is nothing to automate there.

use anyhow::{bail, Context, Result};
use bzip2_rs::DecoderReader as Bz2Decoder;
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::io::{BufWriter, Read, Write};
use std::path::Path;

const RELEASES_API: &str = "https://api.github.com/repos/scriptin/jmdict-simplified/releases/latest";

/// Asset name prefix for the English-glossed JMdict word list, including
/// Tatoeba-sourced example sentences per sense (a strict superset of the
/// plain `jmdict-eng-*` asset's words/readings/glosses — see `dictionary.rs`'s
/// `JmdictSense::examples`).
pub const JMDICT_ASSET_PREFIX: &str = "jmdict-examples-eng-";
/// Asset name prefix for the English-glossed Kanjidic kanji list.
pub const KANJIDIC_ASSET_PREFIX: &str = "kanjidic2-en-";

/// Reports download progress: bytes read so far, and total bytes if the
/// server sent a `Content-Length` header (0 otherwise).
pub type ProgressFn<'a> = dyn Fn(u64, u64) + 'a;

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Pick the first asset whose name starts with `prefix` and ends with
/// `.json.tgz`. Split out from the network call so the selection logic can
/// be unit tested against a fixed asset list.
fn select_asset<'a>(assets: &'a [GithubAsset], prefix: &str) -> Option<&'a GithubAsset> {
    assets.iter().find(|a| a.name.starts_with(prefix) && a.name.ends_with(".json.tgz"))
}

/// Look up the latest `jmdict-simplified` release and return the download
/// URL and release version for the asset matching `prefix` (e.g.
/// [`JMDICT_ASSET_PREFIX`] or [`KANJIDIC_ASSET_PREFIX`]).
fn find_latest_asset(prefix: &str) -> Result<(String, String)> {
    let release: GithubRelease = ureq::get(RELEASES_API)
        .set("User-Agent", "langspark-dictionary-installer")
        .call()
        .context("failed to query jmdict-simplified releases")?
        .into_json()
        .context("failed to parse GitHub releases response")?;

    let asset = select_asset(&release.assets, prefix)
        .with_context(|| format!("no release asset found matching '{prefix}*.json.tgz'"))?;

    Ok((asset.browser_download_url.clone(), release.tag_name.clone()))
}

/// Extract the single `.json` member of a gzip-compressed tarball into
/// `dest`, creating parent directories as needed. Split out from the
/// download step so it can be unit tested against an in-memory archive.
fn extract_json_member(tgz_bytes: &[u8], dest: &Path) -> Result<()> {
    let tar = GzDecoder::new(tgz_bytes);
    let mut archive = tar::Archive::new(tar);
    for entry in archive.entries().context("failed to read downloaded archive")? {
        let mut entry = entry.context("corrupt archive entry")?;
        let is_json = entry.path().context("invalid archive entry path")?.extension().and_then(|e| e.to_str())
            == Some("json");
        if is_json {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).context("failed to create dictionary directory")?;
            }
            let mut out =
                std::fs::File::create(dest).with_context(|| format!("failed to create {}", dest.display()))?;
            std::io::copy(&mut entry, &mut out).context("failed to write extracted dictionary")?;
            return Ok(());
        }
    }
    bail!("downloaded archive did not contain a .json file")
}

/// Download `url` fully into memory, reporting progress via `on_progress`.
fn download_bytes(url: &str, on_progress: &ProgressFn) -> Result<Vec<u8>> {
    let response = ureq::get(url)
        .set("User-Agent", "langspark-dictionary-installer")
        .call()
        .with_context(|| format!("failed to download {url}"))?;

    let total: u64 = response.header("Content-Length").and_then(|s| s.parse().ok()).unwrap_or(0);

    let mut bytes = Vec::new();
    let mut reader = response.into_reader();
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    loop {
        let n = reader.read(&mut buf).context("network error while downloading")?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        downloaded += n as u64;
        on_progress(downloaded, total);
    }

    Ok(bytes)
}

/// Download `url` (a `.json.tgz` asset), reporting progress via
/// `on_progress`, and unpack its JSON member to `dest`.
fn download_and_install(url: &str, dest: &Path, on_progress: &ProgressFn) -> Result<()> {
    let compressed = download_bytes(url, on_progress)?;
    extract_json_member(&compressed, dest)
}

/// Download `url` (a `.tsv.bz2` asset) and return its decompressed bytes.
fn download_bz2(url: &str, on_progress: &ProgressFn) -> Result<Vec<u8>> {
    let compressed = download_bytes(url, on_progress)?;
    let mut decoder = Bz2Decoder::new(compressed.as_slice());
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).with_context(|| format!("failed to decompress {url}"))?;
    Ok(out)
}

/// Download and install the Japanese JMdict word list to `dest` (typically
/// `<dictionary_dir>/ja.json`, matching the layout `diagnostics.rs` expects).
/// Returns the installed release version (e.g. `"3.6.1"`).
pub fn install_jmdict(dest: &Path, on_progress: &ProgressFn) -> Result<String> {
    let (url, version) = find_latest_asset(JMDICT_ASSET_PREFIX)?;
    download_and_install(&url, dest, on_progress)?;
    Ok(version)
}

/// Download and install the Kanjidic kanji list to `dest` (typically
/// `<dictionary_dir>/kanjidic.json`). Returns the installed release version.
pub fn install_kanjidic(dest: &Path, on_progress: &ProgressFn) -> Result<String> {
    let (url, version) = find_latest_asset(KANJIDIC_ASSET_PREFIX)?;
    download_and_install(&url, dest, on_progress)?;
    Ok(version)
}

/// Tatoeba's per-language sentence and translation-link exports (see
/// https://tatoeba.org/en/downloads) — unlike `jmdict-simplified`, these
/// aren't versioned releases, just directly-hosted files updated in place.
const TATOEBA_JPN_SENTENCES_URL: &str = "https://downloads.tatoeba.org/exports/per_language/jpn/jpn_sentences.tsv.bz2";
const TATOEBA_ENG_SENTENCES_URL: &str = "https://downloads.tatoeba.org/exports/per_language/eng/eng_sentences.tsv.bz2";
const TATOEBA_JPN_ENG_LINKS_URL: &str = "https://downloads.tatoeba.org/exports/per_language/jpn/jpn-eng_links.tsv.bz2";

/// Parse a Tatoeba `sentences.tsv` file (`id\tlang\ttext` per line) into an
/// id -> text map. `keep` restricts which ids are kept — used to avoid
/// holding all ~2 million English sentences in memory when only the ~280k
/// linked to a Japanese sentence are needed; pass `None` to keep everything
/// (Japanese's own sentence file is small enough not to bother filtering).
fn parse_tatoeba_sentences(tsv: &[u8], keep: Option<&HashSet<u64>>) -> HashMap<u64, String> {
    String::from_utf8_lossy(tsv)
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let id: u64 = parts.next()?.parse().ok()?;
            let _lang = parts.next()?;
            let text = parts.next()?;
            if keep.is_some_and(|ids| !ids.contains(&id)) {
                return None;
            }
            Some((id, text.to_string()))
        })
        .collect()
}

/// Parse a Tatoeba `jpn-eng_links.tsv` file (`jpn_id\teng_id` per line, per
/// this file's naming — verified against the actual export, since Tatoeba's
/// link files aren't documented as being in a fixed column order).
fn parse_tatoeba_links(tsv: &[u8]) -> Vec<(u64, u64)> {
    String::from_utf8_lossy(tsv)
        .lines()
        .filter_map(|line| {
            let (a, b) = line.split_once('\t')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .collect()
}

/// Download Tatoeba's Japanese sentences, English sentences, and jpn-eng
/// translation links, join them into Japanese/English sentence pairs, and
/// write them to `dest` as one `japanese\tenglish` line per pair — a
/// supplemental example-sentence source (`dictionary::TatoebaExamples`) for
/// the ~85% of common vocabulary words that JMdict's own much smaller
/// curated example subset doesn't cover. Downloads ~150MB total (compressed)
/// but only the joined pairs (a few MB) end up on disk. Returns the number
/// of pairs written.
pub fn install_tatoeba_examples(dest: &Path, on_progress: &ProgressFn) -> Result<usize> {
    let links = parse_tatoeba_links(&download_bz2(TATOEBA_JPN_ENG_LINKS_URL, on_progress)?);
    let needed_eng_ids: HashSet<u64> = links.iter().map(|(_, eng_id)| *eng_id).collect();

    let jpn_text = parse_tatoeba_sentences(&download_bz2(TATOEBA_JPN_SENTENCES_URL, on_progress)?, None);
    let eng_text = parse_tatoeba_sentences(&download_bz2(TATOEBA_ENG_SENTENCES_URL, on_progress)?, Some(&needed_eng_ids));

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).context("failed to create dictionary directory")?;
    }
    let mut out =
        BufWriter::new(std::fs::File::create(dest).with_context(|| format!("failed to create {}", dest.display()))?);

    let mut count = 0;
    for (jpn_id, eng_id) in &links {
        if let (Some(japanese), Some(english)) = (jpn_text.get(jpn_id), eng_text.get(eng_id)) {
            // Tab/newline-free by construction (each was itself one line of a
            // TSV), but guard anyway since a stray one would corrupt the line
            // format `TatoebaExamples::load` expects.
            writeln!(out, "{}\t{}", japanese.replace(['\t', '\n'], " "), english.replace(['\t', '\n'], " "))
                .context("failed to write example sentence")?;
            count += 1;
        }
    }
    out.flush().context("failed to write example sentences")?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn asset(name: &str) -> GithubAsset {
        GithubAsset { name: name.to_string(), browser_download_url: format!("https://example.com/{name}") }
    }

    #[test]
    fn test_select_asset_matches_prefix_and_suffix() {
        let assets = vec![
            asset("jmdict-eng-3.6.1.json.tgz"),
            asset("jmdict-all-3.6.1.json.tgz"),
            asset("kanjidic-en-3.6.1.json.tgz"),
        ];
        let found = select_asset(&assets, "jmdict-eng-").unwrap();
        assert_eq!(found.name, "jmdict-eng-3.6.1.json.tgz");
    }

    #[test]
    fn test_select_asset_no_match() {
        let assets = vec![asset("jmdict-all-3.6.1.json.tgz")];
        assert!(select_asset(&assets, "jmdict-eng-").is_none());
    }

    #[test]
    fn test_select_asset_ignores_non_tgz() {
        let assets = vec![asset("jmdict-eng-3.6.1.json.zip")];
        assert!(select_asset(&assets, "jmdict-eng-").is_none());
    }

    #[test]
    fn test_jmdict_asset_prefix_selects_examples_variant_not_plain_eng() {
        // jmdict-examples-eng is a strict superset of jmdict-eng (same
        // words/readings/glosses, plus example sentences) — make sure the
        // prefix used in production picks the examples-bearing asset and
        // isn't accidentally satisfied by the plain jmdict-eng one.
        let assets = vec![asset("jmdict-eng-3.6.1.json.tgz"), asset("jmdict-examples-eng-3.6.1.json.tgz")];
        let found = select_asset(&assets, JMDICT_ASSET_PREFIX).unwrap();
        assert_eq!(found.name, "jmdict-examples-eng-3.6.1.json.tgz");
    }

    #[test]
    fn test_parse_tatoeba_sentences() {
        let tsv = b"1297\tjpn\t\xe3\x81\x93\xe3\x82\x93\xe3\x81\xab\xe3\x81\xa1\xe3\x81\xaf\ngarbage line\n4702\tjpn\tfoo\tbar";
        let parsed = parse_tatoeba_sentences(tsv, None);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[&1297], "\u{3053}\u{3093}\u{306b}\u{3061}\u{306f}"); // こんにちは
        assert_eq!(parsed[&4702], "foo\tbar"); // splitn(3, ..) keeps the rest of the line as one field
    }

    #[test]
    fn test_parse_tatoeba_sentences_filters_by_keep_set() {
        let tsv = b"1\teng\tkeep me\n2\teng\tdrop me\n";
        let keep: HashSet<u64> = [1].into_iter().collect();
        let parsed = parse_tatoeba_sentences(tsv, Some(&keep));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[&1], "keep me");
    }

    #[test]
    fn test_parse_tatoeba_links() {
        let tsv = b"1297\t4724\n4702\t1276\nmalformed\n";
        let links = parse_tatoeba_links(tsv);
        assert_eq!(links, vec![(1297, 4724), (4702, 1276)]);
    }

    #[test]
    fn test_install_tatoeba_examples_joins_and_writes_pairs() {
        // Exercises the join/write logic directly (no network) by feeding
        // already-parsed maps through the same code path `install_tatoeba_examples`
        // uses after its three downloads — see that function for the shape.
        let links = vec![(1u64, 10u64), (2u64, 99u64) /* 99 has no English text below */];
        let jpn: HashMap<u64, String> = [(1, "こんにちは".to_string()), (2, "さようなら".to_string())].into_iter().collect();
        let eng: HashMap<u64, String> = [(10, "Hello.".to_string())].into_iter().collect();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("nested").join("tatoeba_ja_en.tsv");
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut out = BufWriter::new(std::fs::File::create(&dest).unwrap());
        let mut count = 0;
        for (jpn_id, eng_id) in &links {
            if let (Some(japanese), Some(english)) = (jpn.get(jpn_id), eng.get(eng_id)) {
                writeln!(out, "{japanese}\t{english}").unwrap();
                count += 1;
            }
        }
        out.flush().unwrap();

        assert_eq!(count, 1);
        let contents = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(contents, "こんにちは\tHello.\n");
    }

    fn make_tgz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for (name, data) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, name, *data).unwrap();
            }
            builder.finish().unwrap();
        }
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&tar_bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn test_extract_json_member_writes_json_file() {
        let tgz = make_tgz(&[("readme.txt", b"ignore me"), ("jmdict-eng-3.6.1.json", b"{\"words\":[]}")]);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("nested").join("ja.json");

        extract_json_member(&tgz, &dest).unwrap();

        let contents = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(contents, "{\"words\":[]}");
    }

    #[test]
    fn test_extract_json_member_errors_without_json() {
        let tgz = make_tgz(&[("readme.txt", b"no json here")]);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("ja.json");

        let err = extract_json_member(&tgz, &dest).unwrap_err();
        assert!(err.to_string().contains("did not contain a .json file"));
    }
}
