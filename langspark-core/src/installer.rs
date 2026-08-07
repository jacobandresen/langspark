//! Downloads dictionary data from public sources.
//!
//! Only Japanese has an automated installer: `scriptin/jmdict-simplified`
//! publishes JMdict and Kanjidic as `.json.tgz` GitHub release assets, so we
//! can resolve the latest release, download the matching asset, and unpack
//! its single JSON member. Spanish has no equivalently maintained JSON
//! export (see `dictionary.rs`), so there is nothing to automate there.

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::io::Read;
use std::path::Path;

const RELEASES_API: &str = "https://api.github.com/repos/scriptin/jmdict-simplified/releases/latest";

/// Asset name prefix for the English-glossed JMdict word list.
pub const JMDICT_ASSET_PREFIX: &str = "jmdict-eng-";
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

/// Download `url` (a `.json.tgz` asset), reporting progress via
/// `on_progress`, and unpack its JSON member to `dest`.
fn download_and_install(url: &str, dest: &Path, on_progress: &ProgressFn) -> Result<()> {
    let response = ureq::get(url)
        .set("User-Agent", "langspark-dictionary-installer")
        .call()
        .with_context(|| format!("failed to download {url}"))?;

    let total: u64 = response.header("Content-Length").and_then(|s| s.parse().ok()).unwrap_or(0);

    let mut compressed = Vec::new();
    let mut reader = response.into_reader();
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    loop {
        let n = reader.read(&mut buf).context("network error while downloading")?;
        if n == 0 {
            break;
        }
        compressed.extend_from_slice(&buf[..n]);
        downloaded += n as u64;
        on_progress(downloaded, total);
    }

    extract_json_member(&compressed, dest)
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
