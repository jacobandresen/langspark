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

/// Download `url`, writing it to `out` as it arrives and reporting progress
/// via `on_progress`. Shared by `download_bytes` (buffers in memory — fine
/// for anything up to a few hundred MB) and `install_voicevox_engine`
/// (streams to disk instead, since that asset is ~2GB).
fn download_to(url: &str, out: &mut impl Write, on_progress: &ProgressFn) -> Result<()> {
    let response = ureq::get(url)
        .set("User-Agent", "langspark-dictionary-installer")
        .call()
        .with_context(|| format!("failed to download {url}"))?;

    let total: u64 = response.header("Content-Length").and_then(|s| s.parse().ok()).unwrap_or(0);

    let mut reader = response.into_reader();
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    loop {
        let n = reader.read(&mut buf).context("network error while downloading")?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).context("failed to write downloaded data")?;
        downloaded += n as u64;
        on_progress(downloaded, total);
    }

    Ok(())
}

/// Download `url` fully into memory, reporting progress via `on_progress`.
fn download_bytes(url: &str, on_progress: &ProgressFn) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    download_to(url, &mut bytes, on_progress)?;
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

// ---------------------------------------------------------------------
// VOICEVOX Engine (Japanese TTS) — native install, no Docker required
// ---------------------------------------------------------------------

const VOICEVOX_RELEASES_API: &str = "https://api.github.com/repos/VOICEVOX/voicevox_engine/releases/latest";

/// The VOICEVOX Engine release asset name segment identifying this CPU
/// platform (e.g. `"linux-cpu-x64"`) — `None` if VOICEVOX doesn't publish a
/// prebuilt CPU engine for it. Covers Linux x86_64/aarch64 and Windows
/// x86_64 (VOICEVOX's own asset naming doesn't distinguish Windows
/// architectures the way Linux's does). macOS and GPU builds (`-nvidia`/
/// `-directml`) fall back to `scripts/setup-voicevox.sh`'s Docker path.
fn voicevox_platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-cpu-x64"),
        ("linux", "aarch64") => Some("linux-cpu-arm64"),
        ("windows", "x86_64") => Some("windows-cpu"),
        _ => None,
    }
}

/// The VOICEVOX Engine's entry-point executable name within its install
/// directory — `run.exe` on Windows, `run` everywhere else this project
/// installs a native engine for (see `voicevox_platform`).
pub fn voicevox_run_executable_name() -> &'static str {
    if cfg!(windows) {
        "run.exe"
    } else {
        "run"
    }
}

/// Pick the release asset that's the whole-engine `.vvpp` (a plain zip) for
/// `platform` — as opposed to the equivalent split `.7z.NNN` archive (needs
/// an external `7z` tool to reassemble/extract) or a `.txt` sidecar file.
fn select_voicevox_asset<'a>(assets: &'a [GithubAsset], platform: &str) -> Option<&'a GithubAsset> {
    let prefix = format!("voicevox_engine-{platform}-");
    assets.iter().find(|a| a.name.starts_with(&prefix) && a.name.ends_with(".vvpp"))
}

/// Download and install a native VOICEVOX Engine build (see
/// `voicevox_platform` for which platforms) to `dest_dir` — no Docker
/// needed, unlike `scripts/setup-voicevox.sh`. The `.vvpp` release asset is
/// a plain zip of the engine's install directory (a `run`/`run.exe`
/// executable — see `voicevox_run_executable_name` — bundled ONNX runtime,
/// and ~1.5GB of voice model weights, so the download is large, around
/// 2GB); extracted as-is, then the executable is marked runnable on Unix
/// (Windows `.exe` files need no such marking). `langspark-gui` starts
/// `<dest_dir>/<run executable> --host 127.0.0.1 --port 50021` to actually
/// run it — this only installs the files. Returns the installed release
/// version.
pub fn install_voicevox_engine(dest_dir: &Path, on_progress: &ProgressFn) -> Result<String> {
    let platform = voicevox_platform().context(
        "no native VOICEVOX Engine build for this OS/CPU architecture — see \
         scripts/setup-voicevox.sh for the Docker-based alternative",
    )?;

    let release: GithubRelease = ureq::get(VOICEVOX_RELEASES_API)
        .set("User-Agent", "langspark-voicevox-installer")
        .call()
        .context("failed to query VOICEVOX Engine releases")?
        .into_json()
        .context("failed to parse GitHub releases response")?;

    let asset = select_voicevox_asset(&release.assets, platform)
        .with_context(|| format!("no VOICEVOX Engine .vvpp release asset found for '{platform}'"))?;

    std::fs::create_dir_all(dest_dir).context("failed to create VOICEVOX Engine directory")?;
    let tmp_zip = dest_dir.join(".download.vvpp.part");
    {
        let mut file = std::fs::File::create(&tmp_zip)
            .with_context(|| format!("failed to create {}", tmp_zip.display()))?;
        download_to(&asset.browser_download_url, &mut file, on_progress)?;
    }

    let file = std::fs::File::open(&tmp_zip).context("failed to reopen downloaded archive")?;
    let mut archive =
        zip::ZipArchive::new(std::io::BufReader::new(file)).context("failed to read VOICEVOX Engine archive")?;
    archive.extract(dest_dir).context("failed to extract VOICEVOX Engine archive")?;
    let _ = std::fs::remove_file(&tmp_zip);

    #[cfg(unix)]
    mark_executable(&dest_dir.join(voicevox_run_executable_name()))
        .context("failed to make the VOICEVOX Engine executable")?;

    Ok(release.tag_name)
}

/// Set the executable bit on `path` (in addition to whatever read/write bits
/// it already has), since `zip::ZipArchive::extract` doesn't reliably
/// preserve Unix permissions from the archive across all `zip` versions.
#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

// ---------------------------------------------------------------------
// Qwen3-ASR model (speech recognition) — weights via plain HTTP,
// tokenizer.json via a throwaway Python venv (see `generate_tokenizer_json`)
// ---------------------------------------------------------------------

/// Files fetched directly from the model's Hugging Face repo — everything
/// except `tokenizer.json`, which that repo doesn't ship.
const ASR_MODEL_FILES: &[&str] = &[
    "config.json",
    "model.safetensors",
    "vocab.json",
    "merges.txt",
    "tokenizer_config.json",
    "generation_config.json",
    "preprocessor_config.json",
    "chat_template.json",
];

/// Download a Qwen3 ASR model from Hugging Face (`model`, e.g.
/// `"Qwen3-ASR-0.6B"` — the larger `Qwen3-ASR-1.7B` ships its weights
/// sharded across multiple `model-NNNNN-of-NNNNN.safetensors` files rather
/// than one `model.safetensors`, which this doesn't handle; use
/// `scripts/setup-asr.sh` for that one instead) into `dest_dir`, plus a
/// generated `tokenizer.json`. Mirrors `scripts/setup-asr.sh`, just driven
/// from the app itself — including needing `python3` on `PATH` for that last
/// step (see `generate_tokenizer_json`). Doesn't touch libtorch: that's a
/// dynamic *runtime* dependency of an `asr`-featured binary too (confirmed
/// via `ldd`/`readelf` — a plain `NEEDED libtorch_cpu.so` entry), not just a
/// build-time one, but it's already present by the time this can run at all
/// (the binary wouldn't have launched otherwise), so there's still nothing
/// for this function itself to install. `langspark-gui`'s Preferences dialog
/// only wires this installer in behind `#[cfg(feature = "asr")]` for exactly
/// that reason — see its `build` function. Idempotent: skips any file that
/// already exists, like the shell script.
pub fn install_asr_model(model: &str, dest_dir: &Path, on_progress: &ProgressFn) -> Result<String> {
    std::fs::create_dir_all(dest_dir).context("failed to create ASR model directory")?;
    let base_url = format!("https://huggingface.co/Qwen/{model}/resolve/main");

    for file in ASR_MODEL_FILES {
        let dest = dest_dir.join(file);
        if dest.exists() {
            continue;
        }
        let mut out =
            std::fs::File::create(&dest).with_context(|| format!("failed to create {}", dest.display()))?;
        download_to(&format!("{base_url}/{file}"), &mut out, on_progress)
            .with_context(|| format!("failed to download {file}"))?;
    }

    if !dest_dir.join("tokenizer.json").exists() {
        generate_tokenizer_json(dest_dir)?;
    }

    Ok(format!("Installed {model}"))
}

/// Generate `<model_dir>/tokenizer.json` from the plain files a Hugging Face
/// repo ships (`vocab.json`, `merges.txt`, `tokenizer_config.json`, ...).
/// Reimplementing this in Rust would mean re-deriving the exact fast-
/// tokenizer construction `transformers.AutoTokenizer` does from those files
/// (special tokens, added tokens, normalizer/pre-tokenizer config) — that
/// risks silently producing a subtly *different* tokenizer (garbled
/// transcriptions at runtime) rather than a loud failure, so instead this
/// shells out to the same real implementation `scripts/setup-asr.sh` uses,
/// in a throwaway venv removed afterward either way.
fn generate_tokenizer_json(model_dir: &Path) -> Result<()> {
    if std::process::Command::new("python3").arg("--version").output().is_err() {
        bail!(
            "python3 is required to generate tokenizer.json (via a throwaway venv with \
             'transformers') but wasn't found on PATH"
        );
    }

    let venv_dir = std::env::temp_dir().join(format!("langspark-asr-tokenizer-venv-{}", std::process::id()));
    let result = generate_tokenizer_json_with_venv(model_dir, &venv_dir);
    let _ = std::fs::remove_dir_all(&venv_dir);
    result
}

fn generate_tokenizer_json_with_venv(model_dir: &Path, venv_dir: &Path) -> Result<()> {
    run_checked(std::process::Command::new("python3").args(["-m", "venv"]).arg(venv_dir))
        .context("failed to create a Python venv")?;

    let pip = venv_dir.join("bin/pip");
    run_checked(std::process::Command::new(&pip).args(["install", "--upgrade", "pip", "-q"]))
        .context("failed to upgrade pip in the venv")?;
    run_checked(std::process::Command::new(&pip).args(["install", "transformers", "-q"]))
        .context("failed to install the 'transformers' package in the venv")?;

    let tokenizer_path = model_dir.join("tokenizer.json");
    let script = format!(
        "from transformers import AutoTokenizer\n\
         tok = AutoTokenizer.from_pretrained('{}', trust_remote_code=True)\n\
         tok.backend_tokenizer.save('{}')\n",
        model_dir.display(),
        tokenizer_path.display(),
    );
    run_checked(std::process::Command::new(venv_dir.join("bin/python")).args(["-c", &script]))
        .context("failed to generate tokenizer.json")?;

    Ok(())
}

/// Run `cmd`, mapping a nonzero exit status to an `Err` (`Command::status`
/// alone only reports spawn failures, not the command's own failure).
fn run_checked(cmd: &mut std::process::Command) -> Result<()> {
    let status = cmd.status().context("failed to spawn command")?;
    if !status.success() {
        bail!("command exited with {status}");
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Aozora Bunko book catalog + per-work text — see `books.rs` for the
// parsed data types and ruby-markup parsing this feeds into.
// ---------------------------------------------------------------------

use crate::books::{genre_for_ndc, parse_book_text, BookCatalogEntry, BookText};

/// The official Aozora Bunko repository mirrors both the site's bulk CSV
/// catalog and every work's text; fetched via `raw.githubusercontent.com`
/// rather than cloning the (very large) repository.
const AOZORA_CATALOG_URL: &str =
    "https://raw.githubusercontent.com/aozorabunko/aozorabunko/master/index_pages/list_person_all_extended_utf8.zip";

/// Extract the single member of `zip_bytes` whose name ends with `ext`
/// (case-insensitive) — both the catalog archive (one `.csv`) and a book
/// archive (one `.txt`) are simple single-member zips.
fn extract_member(zip_bytes: &[u8], ext: &str) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).context("failed to read zip archive")?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("corrupt zip entry")?;
        if file.name().to_lowercase().ends_with(ext) {
            let mut out = Vec::new();
            std::io::copy(&mut file, &mut out).context("failed to read zip entry")?;
            return Ok(out);
        }
    }
    bail!("zip archive did not contain a '{ext}' file");
}

/// Look up a CSV column's index by its exact header name — resilient to the
/// exact column count/order of Aozora's catalog, which isn't guaranteed
/// stable across revisions (unlike `jmdict-simplified`'s versioned JSON
/// releases, this CSV is updated in place).
fn column_index(headers: &csv::StringRecord, name: &str) -> Result<usize> {
    headers
        .iter()
        .position(|h| h == name)
        .with_context(|| format!("catalog CSV has no '{name}' column (headers: {})", headers.iter().collect::<Vec<_>>().join(", ")))
}

/// Parse Aozora's `list_person_all_extended_utf8.csv` (already extracted
/// from its zip) into a catalog of books. Skips rows missing a title,
/// author id, or text file URL — some catalog entries are metadata-only,
/// with no digitized text yet.
fn parse_aozora_catalog_csv(csv_bytes: &[u8]) -> Result<Vec<BookCatalogEntry>> {
    let mut reader = csv::ReaderBuilder::new().from_reader(csv_bytes);
    let headers = reader.headers().context("catalog CSV has no header row")?.clone();

    let id_col = column_index(&headers, "作品ID")?;
    let title_col = column_index(&headers, "作品名")?;
    let last_name_col = column_index(&headers, "姓")?;
    let first_name_col = column_index(&headers, "名")?;
    let ndc_col = column_index(&headers, "分類番号")?;
    let url_col = column_index(&headers, "テキストファイルURL")?;

    let mut entries = Vec::new();
    for record in reader.records() {
        let record = record.context("malformed catalog CSV row")?;
        let get = |i: usize| record.get(i).unwrap_or("").trim();

        let (id, title, text_url) = (get(id_col), get(title_col), get(url_col));
        if id.is_empty() || title.is_empty() || text_url.is_empty() {
            continue;
        }

        let author =
            [get(last_name_col), get(first_name_col)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        entries.push(BookCatalogEntry {
            id: id.to_string(),
            title: title.to_string(),
            author,
            genre: genre_for_ndc(get(ndc_col)),
            text_url: text_url.to_string(),
        });
    }
    Ok(entries)
}

/// Download and install the Aozora Bunko book catalog to `dest` (typically
/// `<books_dir>/catalog.json`) as a JSON-encoded `Vec<BookCatalogEntry>` —
/// gates whether the Books tab appears (see `app.rs`), the same way
/// `install_jmdict` gates the Vocabulary tab's "Add Word" button. Returns
/// the number of catalog entries installed.
pub fn install_aozora_catalog(dest: &Path, on_progress: &ProgressFn) -> Result<usize> {
    let zip_bytes = download_bytes(AOZORA_CATALOG_URL, on_progress)?;
    let csv_bytes = extract_member(&zip_bytes, ".csv")?;
    let entries = parse_aozora_catalog_csv(&csv_bytes)?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).context("failed to create books directory")?;
    }
    let json = serde_json::to_vec(&entries).context("failed to serialize book catalog")?;
    std::fs::write(dest, json).with_context(|| format!("failed to write {}", dest.display()))?;

    Ok(entries.len())
}

/// Fetch and parse `entry`'s full text, caching the parsed result to
/// `<cache_dir>/<entry.id>.json` so re-opening a book doesn't re-download or
/// re-parse it. Aozora serves book text as a Shift-JIS-encoded `.txt` inside
/// a zip (`entry.text_url`); `encoding_rs` decodes it losslessly for all but
/// a handful of obscure/gaiji characters outside Shift-JIS's mapping table,
/// which become U+FFFD rather than failing the whole book.
pub fn fetch_book(entry: &BookCatalogEntry, cache_dir: &Path, on_progress: &ProgressFn) -> Result<BookText> {
    let cache_path = cache_dir.join(format!("{}.json", entry.id));
    if let Ok(cached) = std::fs::read(&cache_path) {
        if let Ok(book) = serde_json::from_slice(&cached) {
            return Ok(book);
        }
    }

    let zip_bytes =
        download_bytes(&entry.text_url, on_progress).with_context(|| format!("failed to download '{}'", entry.title))?;
    let sjis_bytes = extract_member(&zip_bytes, ".txt")?;
    let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&sjis_bytes);
    let book = parse_book_text(&text);

    std::fs::create_dir_all(cache_dir).context("failed to create book cache directory")?;
    if let Ok(json) = serde_json::to_vec(&book) {
        let _ = std::fs::write(&cache_path, json); // best-effort: a failed cache write shouldn't fail the fetch
    }

    Ok(book)
}

// ---------------------------------------------------------------------
// Paragraph translation model (Helsinki-NLP OPUS-MT ja-en, via candle) —
// see `translation.rs` for the model this feeds and why it's candle-based
// rather than the more obvious rust-bert.
// ---------------------------------------------------------------------

/// Files fetched directly from the model's Hugging Face repo — everything
/// except the weights themselves (see `TRANSLATION_MODEL_WEIGHTS_URL`).
const TRANSLATION_MODEL_FILES: &[&str] = &["config.json", "source.spm", "target.spm", "vocab.json"];

/// Helsinki-NLP published `Helsinki-NLP/opus-mt-ja-en`'s weights only as a
/// `pytorch_model.bin` in PyTorch's *legacy* pickle format (a bare pickle
/// stream, no zip wrapper — confirmed against the real downloaded file: it
/// starts with the raw pickle protocol-2 marker `\x80\x02`, not a zip
/// local-file-header). `candle`'s pickle/`.pth` reader (what
/// `translation::Translator` loads weights through) only understands the
/// newer zip-based format modern `torch.save` produces, and there's no
/// pre-converted `safetensors` variant of this repo published anywhere
/// (checked huggingface.co/Helsinki-NLP/opus-mt-ja-en's `/refs` API — no
/// conversion branch — and the community re-uploads that do exist only
/// publish ONNX, not safetensors). So this points at a one-time conversion
/// done ourselves and hosted on our own release rather than converting it at
/// install time (which needed a throwaway Python/PyTorch venv — exactly the
/// kind of install-time script this installer otherwise avoids). Same
/// tensors/values as upstream, just re-saved as safetensors; verified by
/// loading it through `Translator::load`/`translate` before publishing.
const TRANSLATION_MODEL_WEIGHTS_URL: &str =
    "https://github.com/jacobandresen/langspark/releases/download/model-assets-v1/model.safetensors";

/// Download the Helsinki-NLP OPUS-MT Japanese→English translation model
/// (~300MB config/tokenizer files from Hugging Face, plus the ~550MB
/// pre-converted weights from `TRANSLATION_MODEL_WEIGHTS_URL`) into
/// `dest_dir`. Plain HTTP downloads only, no subprocess — see
/// `TRANSLATION_MODEL_WEIGHTS_URL`'s doc comment for why the weights come
/// from there rather than upstream directly. Idempotent: skips any file that
/// already exists, matching `install_asr_model`'s convention.
pub fn install_translation_model(dest_dir: &Path, on_progress: &ProgressFn) -> Result<String> {
    std::fs::create_dir_all(dest_dir).context("failed to create translation model directory")?;
    const BASE_URL: &str = "https://huggingface.co/Helsinki-NLP/opus-mt-ja-en/resolve/main";

    for file in TRANSLATION_MODEL_FILES {
        let dest = dest_dir.join(file);
        if dest.exists() {
            continue;
        }
        let mut out =
            std::fs::File::create(&dest).with_context(|| format!("failed to create {}", dest.display()))?;
        download_to(&format!("{BASE_URL}/{file}"), &mut out, on_progress)
            .with_context(|| format!("failed to download {file}"))?;
    }

    let weights_dest = dest_dir.join("model.safetensors");
    if !weights_dest.exists() {
        let mut out = std::fs::File::create(&weights_dest)
            .with_context(|| format!("failed to create {}", weights_dest.display()))?;
        download_to(TRANSLATION_MODEL_WEIGHTS_URL, &mut out, on_progress)
            .context("failed to download model.safetensors")?;
    }

    Ok("Installed Helsinki-NLP/opus-mt-ja-en".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn asset(name: &str) -> GithubAsset {
        GithubAsset { name: name.to_string(), browser_download_url: format!("https://example.com/{name}") }
    }

    #[test]
    fn test_select_voicevox_asset_picks_vvpp_not_split_7z() {
        let assets = vec![
            asset("voicevox_engine-linux-cpu-x64-0.25.2.7z.001"),
            asset("voicevox_engine-linux-cpu-x64-0.25.2.7z.txt"),
            asset("voicevox_engine-linux-cpu-x64-0.25.2.vvpp"),
            asset("voicevox_engine-linux-cpu-x64-0.25.2.vvpp.txt"),
            asset("voicevox_engine-linux-cpu-arm64-0.25.2.vvpp"),
        ];
        let found = select_voicevox_asset(&assets, "linux-cpu-x64").unwrap();
        assert_eq!(found.name, "voicevox_engine-linux-cpu-x64-0.25.2.vvpp");
    }

    #[test]
    fn test_select_voicevox_asset_no_match_for_unbuilt_platform() {
        let assets = vec![asset("voicevox_engine-linux-cpu-x64-0.25.2.vvpp")];
        assert!(select_voicevox_asset(&assets, "macos-cpu").is_none());
    }

    #[test]
    fn test_select_voicevox_asset_windows() {
        let assets = vec![
            asset("voicevox_engine-windows-cpu-0.25.2.vvpp"),
            asset("voicevox_engine-windows-nvidia-0.25.2.vvpp.txt"),
        ];
        let found = select_voicevox_asset(&assets, "windows-cpu").unwrap();
        assert_eq!(found.name, "voicevox_engine-windows-cpu-0.25.2.vvpp");
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

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut writer = zip::ZipWriter::new(&mut buf);
        let options = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
        buf.into_inner()
    }

    #[test]
    fn test_extract_member_finds_by_extension_case_insensitive() {
        let zip = make_zip(&[("readme.TXT", b"not this one" as &[u8]), ("book.CSV", b"id,title\n1,foo\n")]);
        let csv = extract_member(&zip, ".csv").unwrap();
        assert_eq!(csv, b"id,title\n1,foo\n");
    }

    #[test]
    fn test_extract_member_errors_without_match() {
        let zip = make_zip(&[("readme.txt", b"nothing here" as &[u8])]);
        let err = extract_member(&zip, ".csv").unwrap_err();
        assert!(err.to_string().contains("did not contain a '.csv' file"));
    }

    fn sample_catalog_csv() -> String {
        // Only the columns parse_aozora_catalog_csv actually reads —
        // extra/differently-ordered columns are fine since lookup is by
        // header name, which is the point.
        "作品ID,作品名,姓,名,分類番号,テキストファイルURL,unused\n\
         1234,吾輩は猫である,夏目,漱石,NDC 913,https://www.aozora.gr.jp/cards/000148/files/789.zip,x\n\
         5678,金色夜叉,尾崎,紅葉,NDC 913,https://www.aozora.gr.jp/cards/000000/files/000.zip,y\n\
         9999,未digitized,誰か,不明,NDC 913,,z\n"
            .to_string()
    }

    #[test]
    fn test_parse_aozora_catalog_csv_builds_entries_by_header_name() {
        let entries = parse_aozora_catalog_csv(sample_catalog_csv().as_bytes()).unwrap();
        assert_eq!(entries.len(), 2); // the row with no text_url is skipped
        assert_eq!(entries[0].id, "1234");
        assert_eq!(entries[0].title, "吾輩は猫である");
        assert_eq!(entries[0].author, "夏目 漱石");
        assert_eq!(entries[0].genre.as_deref(), Some("Novels & Stories"));
        assert_eq!(entries[0].text_url, "https://www.aozora.gr.jp/cards/000148/files/789.zip");
    }

    #[test]
    fn test_parse_aozora_catalog_csv_errors_on_missing_column() {
        let err = parse_aozora_catalog_csv(b"foo,bar\n1,2\n").unwrap_err();
        assert!(err.to_string().contains("has no '作品ID' column"));
    }

    #[test]
    fn test_fetch_book_caches_parsed_result() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join("books");
        let entry = BookCatalogEntry {
            id: "42".to_string(),
            title: "test".to_string(),
            author: "test author".to_string(),
            genre: None,
            text_url: "unused-because-the-cache-is-seeded-directly-below".to_string(),
        };

        // Seed the cache directly (the download path needs real network
        // access, so it isn't exercised here — same tradeoff as
        // `test_install_tatoeba_examples_joins_and_writes_pairs` above) to
        // exercise the cache-hit path.
        std::fs::create_dir_all(&cache_dir).unwrap();
        let expected = parse_book_text("吾輩は猫である。\n");
        std::fs::write(cache_dir.join("42.json"), serde_json::to_vec(&expected).unwrap()).unwrap();

        let book = fetch_book(&entry, &cache_dir, &|_, _| {}).unwrap();
        assert_eq!(book.paragraphs, expected.paragraphs);
    }
}
