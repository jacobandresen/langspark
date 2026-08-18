//! Paragraph translation: offline Japanese→English machine translation via
//! Helsinki-NLP's OPUS-MT Marian model (`Helsinki-NLP/opus-mt-ja-en`),
//! running locally through `candle` — deliberately not `rust-bert`, which
//! would pull in a second, conflicting `tch`/libtorch version alongside the
//! one `qwen3-asr-rs` already pins for speech recognition (see `asr.rs`).
//! `candle` has no libtorch dependency at all, so there's no conflict.
//!
//! The model has no ready-made ja-en preset in `candle_transformers`, but
//! everything needed to build one is present: `Helsinki-NLP/opus-mt-ja-en`'s
//! own `config.json` deserializes directly into
//! [`candle_transformers::models::marian::Config`], and its SentencePiece
//! tokenizer files (`source.spm`/`target.spm`) are loaded directly via the
//! pure-Rust `sentencepiece-rs` crate rather than needing the "fast
//! tokenizer" JSON conversion candle's own example script uses. Its
//! upstream weights (`pytorch_model.bin`) do need a one-time conversion to
//! the `model.safetensors` this loads from below — see
//! `installer::TRANSLATION_MODEL_WEIGHTS_URL`'s doc comment for why, and
//! why that conversion happens once ourselves rather than at install time.
//!
//! One real subtlety this needed working out empirically (garbage
//! translations until fixed): a raw SentencePiece model's own internal
//! piece numbering (what `sentencepiece-rs`'s `encode_to_ids`/`decode_ids`
//! use) is **not** the id space the model's embedding matrix was trained
//! against. HuggingFace's `MarianTokenizer` assigns its own ids via a
//! separate `vocab.json` (verified against the real file: raw
//! `source.spm` numbers `<unk>`/`<s>`/`</s>` as 0/1/2 with real pieces
//! starting at 3, while `vocab.json` has no `<s>` entry at all and orders
//! everything differently — e.g. `▁の` is id 3 in one and id 4 in the
//! other). So this always tokenizes to *piece strings* first
//! (`encode`/`decode_pieces`, not `encode_to_ids`/`decode_ids`), and
//! translates piece string ↔ model id itself through `vocab.json`.

use anyhow::{Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::marian;
use sentencepiece_rs::SentencePieceProcessor;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// Piece string ↔ model vocabulary id, both directions — built from
/// `vocab.json`, not from either `.spm` file's own internal numbering (see
/// this module's doc comment for why those differ). Shared between source
/// and target: `Helsinki-NLP/opus-mt-ja-en` publishes one `vocab.json` for
/// both (matching its `share_encoder_decoder_embeddings` config flag).
struct Vocab {
    piece_to_id: HashMap<String, u32>,
    id_to_piece: Vec<String>,
}

impl Vocab {
    fn load(path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(path).context("failed to read vocab.json")?;
        let piece_to_id: HashMap<String, u32> =
            serde_json::from_str(&json).context("failed to parse vocab.json")?;

        // Sized by the *highest id present* + 1, not by the entry count —
        // those coincide for a real, gap-free `vocab.json` (ids 0..vocab_size
        // contiguously), but this test-fixture-caught bug on the first pass:
        // sizing by entry count silently truncates `id_to_piece` whenever a
        // fixture (or any future vocab with genuine gaps) doesn't happen to
        // use every id in [0, entry count).
        let max_id = piece_to_id.values().copied().max().unwrap_or(0);
        let mut id_to_piece = vec![String::new(); max_id as usize + 1];
        for (piece, &id) in &piece_to_id {
            if let Some(slot) = id_to_piece.get_mut(id as usize) {
                *slot = piece.clone();
            }
        }
        Ok(Self { piece_to_id, id_to_piece })
    }

    fn id_of(&self, piece: &str) -> Option<u32> {
        self.piece_to_id.get(piece).copied()
    }

    fn piece_of(&self, id: u32) -> Option<&str> {
        self.id_to_piece.get(id as usize).map(String::as_str)
    }
}

/// Loads once (see `load`, called lazily and kept resident for the rest of
/// the session by the caller — see `langspark-gui`'s `AppState`, since
/// reloading ~300MB of weights per translation would be far too slow) and
/// answers `translate` calls after that.
pub struct Translator {
    model: Mutex<marian::MTModel>,
    source_tokenizer: SentencePieceProcessor,
    target_tokenizer: SentencePieceProcessor,
    vocab: Vocab,
    config: marian::Config,
    device: Device,
}

/// Seed for `LogitsProcessor`'s RNG — irrelevant here since `translate`
/// always samples with `temperature: None` (greedy/argmax decoding, the
/// right choice for translation: deterministic and repeatable, unlike
/// creative text generation where sampling diversity matters).
const GREEDY_SEED: u64 = 0;

/// How strongly `translate`'s decode loop discourages repeating an
/// already-generated token — see its `apply_repeat_penalty` call. `1.1`
/// matches the default candle's own text-generation examples
/// (llama/mistral/phi/...) all use.
const REPEAT_PENALTY: f32 = 1.1;

/// Tokens `translate`'s decode loop must generate before it's allowed to
/// stop. Without this, the model sometimes samples end-of-sequence/pad as
/// literally the *first* token for a sentence it finds difficult — confirmed
/// on a real paragraph once `REPEAT_PENALTY` was added: the same input that
/// used to degenerate into a repetition loop instead produced an empty
/// translation. A short forced minimum trades a possibly-imperfect partial
/// translation for that empty one.
const MIN_NEW_TOKENS: usize = 3;

impl Translator {
    /// Load the model from `model_dir` (expects `config.json`,
    /// `pytorch_model.bin`, `source.spm`, `target.spm` — see
    /// `installer::install_translation_model`). CPU-only: this model is
    /// small enough (~300MB, 6+6 Marian layers) that CPU inference for a
    /// single paragraph takes a few seconds, not requiring a GPU feature.
    pub fn load(model_dir: &Path) -> Result<Self> {
        let config_json = std::fs::read_to_string(model_dir.join("config.json"))
            .context("failed to read translation model config.json")?;
        let config: marian::Config =
            serde_json::from_str(&config_json).context("failed to parse translation model config.json")?;

        let device = Device::Cpu;
        // `installer::install_translation_model` converts the model's raw
        // `pytorch_model.bin` (which candle's pickle reader can't load — see
        // that function's doc comment for why) to this safetensors file at
        // install time, once.
        let weights_path = model_dir.join("model.safetensors");
        // SAFETY: standard candle mmap-loading caveat — safe as long as
        // nothing else truncates/rewrites this file while it's mapped, which
        // nothing in this app does once `install_translation_model` has
        // finished writing it.
        let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[&weights_path], DType::F32, &device) }
            .context("failed to load translation model weights")?;
        let model = marian::MTModel::new(&config, vb).context("failed to build translation model")?;

        let source_tokenizer = SentencePieceProcessor::open(model_dir.join("source.spm"))
            .context("failed to load source.spm tokenizer")?;
        let target_tokenizer = SentencePieceProcessor::open(model_dir.join("target.spm"))
            .context("failed to load target.spm tokenizer")?;
        let vocab = Vocab::load(&model_dir.join("vocab.json"))?;

        Ok(Self { model: Mutex::new(model), source_tokenizer, target_tokenizer, vocab, config, device })
    }

    /// Translate `japanese` to English. Greedy (temperature-0) decoding,
    /// stopping at the model's end-of-sequence token or
    /// `max_position_embeddings` tokens, whichever comes first — mirrors
    /// `candle`'s own `examples/marian-mt` reference implementation's
    /// generation loop exactly.
    pub fn translate(&self, japanese: &str) -> Result<String> {
        // The decoder's self-attention KV cache carries state across calls
        // (that's what makes incremental decoding fast) — without resetting
        // it here, a second translation would decode as a continuation of
        // the first one's cached keys/values instead of starting fresh.
        let mut model = self.model.lock().expect("translator model mutex poisoned");
        model.reset_kv_cache();

        let unk_id = self.vocab.id_of("<unk>").unwrap_or(0);
        let max_len = self.config.max_position_embeddings;
        let mut source_ids: Vec<u32> = self
            .source_tokenizer
            .encode(japanese)
            .context("failed to tokenize paragraph for translation")?
            .iter()
            .map(|piece| self.vocab.id_of(piece).unwrap_or(unk_id))
            .collect();
        source_ids.truncate(max_len.saturating_sub(1));
        source_ids.push(self.config.eos_token_id);

        let input = Tensor::new(source_ids.as_slice(), &self.device)?.unsqueeze(0)?;
        let encoder_xs = model.encoder().forward(&input, 0)?;

        let mut logits_processor = LogitsProcessor::new(GREEDY_SEED, None, None);
        let mut token_ids: Vec<u32> = vec![self.config.decoder_start_token_id];
        for index in 0..max_len {
            let context_size = if index >= 1 { 1 } else { token_ids.len() };
            let start_pos = token_ids.len() - context_size;
            let input_ids = Tensor::new(&token_ids[start_pos..], &self.device)?.unsqueeze(0)?;
            let logits = model.decode(&input_ids, &encoder_xs, start_pos)?;
            let logits = logits.squeeze(0)?;
            let logits = logits.get(logits.dim(0)? - 1)?;
            // Greedy (argmax) decoding has no built-in defense against
            // getting stuck repeating the same phrase verbatim — confirmed
            // on real paragraphs, where a handful out of a small sample
            // degenerated into "I've seen<pad> I've seen<pad> ..." or, worse,
            // nothing but `<pad>` hundreds of times over. Penalizing tokens
            // already generated this call (not persisted across calls —
            // `apply_repeat_penalty` only sees `token_ids`, reset per
            // `translate`) makes repeating a token less attractive each time
            // it recurs, which breaks the loop before it can run away. `1.1`
            // matches candle's own text-generation examples' default.
            let logits = candle_transformers::utils::apply_repeat_penalty(&logits, REPEAT_PENALTY, &token_ids[1..])?;
            // Enforce `MIN_NEW_TOKENS` by making end-of-sequence/pad
            // impossible to sample (not just discouraged, unlike the repeat
            // penalty above) until then.
            let logits = if token_ids.len() - 1 < MIN_NEW_TOKENS {
                mask_tokens(
                    &logits,
                    &[self.config.eos_token_id, self.config.forced_eos_token_id, self.config.pad_token_id],
                )?
            } else {
                logits
            };
            let token = logits_processor.sample(&logits)?;
            token_ids.push(token);
            // `pad_token_id` stops generation the same as end-of-sequence
            // would: a well-formed translation never legitimately contains
            // its own decoder-start/pad token mid-sequence (Marian reuses
            // one id for both — see this module's doc comment), so sampling
            // it here is itself a sign generation has gone off the rails,
            // confirmed by the reproduction above. Without this, the token
            // was previously left to decode to the literal string "<pad>",
            // which is also invalid Pango markup — see langspark-gui's
            // `books::sentence_popup` fix for the crash that caused.
            if token == self.config.eos_token_id
                || token == self.config.forced_eos_token_id
                || token == self.config.pad_token_id
            {
                break;
            }
        }

        let output_pieces: Vec<&str> = token_ids[1..] // skip the seed decoder_start_token_id
            .iter()
            .take_while(|&&id| {
                id != self.config.eos_token_id && id != self.config.forced_eos_token_id && id != self.config.pad_token_id
            })
            .filter_map(|&id| self.vocab.piece_of(id))
            .collect();
        self.target_tokenizer.decode_pieces(&output_pieces).context("failed to decode translated text")
    }
}

/// Force each of `ids`' logits to `-infinity` so `LogitsProcessor::sample`'s
/// argmax can never select them — used by `translate` to enforce
/// `MIN_NEW_TOKENS`.
fn mask_tokens(logits: &Tensor, ids: &[u32]) -> Result<Tensor> {
    let device = logits.device();
    let mut logits = logits.to_dtype(DType::F32)?.to_vec1::<f32>()?;
    for &id in ids {
        if let Some(logit) = logits.get_mut(id as usize) {
            *logit = f32::NEG_INFINITY;
        }
    }
    let len = logits.len();
    Tensor::from_vec(logits, len, device).context("failed to rebuild logits tensor after masking")
}

/// On-disk cache of translated paragraphs, keyed by a hash of the source
/// text — mirrors `audio::AudioCache` exactly, for the same reason: without
/// it, re-opening the same paragraph's popup would re-run several seconds
/// of CPU inference every time instead of once per paragraph per install.
pub struct TranslationCache {
    cache_dir: std::path::PathBuf,
}

impl TranslationCache {
    pub fn new(cache_dir: impl Into<std::path::PathBuf>) -> Self {
        Self { cache_dir: cache_dir.into() }
    }

    fn cache_path(&self, japanese: &str) -> std::path::PathBuf {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        japanese.hash(&mut hasher);
        self.cache_dir.join(format!("{:x}.txt", hasher.finish()))
    }

    /// The cached translation for `japanese`, if present.
    pub fn get(&self, japanese: &str) -> Option<String> {
        std::fs::read_to_string(self.cache_path(japanese)).ok()
    }

    /// Store `english` as the translation for `japanese`.
    pub fn put(&self, japanese: &str, english: &str) -> Result<()> {
        std::fs::create_dir_all(&self.cache_dir).context("failed to create translation cache directory")?;
        std::fs::write(self.cache_path(japanese), english).context("failed to write cached translation")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A handful of real entries from `Helsinki-NLP/opus-mt-ja-en`'s actual
    /// `vocab.json`, at their real ids — including the "gap" this format
    /// requires `Vocab::load` to handle: only 5 of these ids are given, but
    /// `id_to_piece` must still return `None`/empty (not panic or shift
    /// everything down) for every id in between that a real 60716-entry file
    /// would otherwise fill in.
    const VOCAB_FIXTURE: &str = r#"{
        "</s>": 0,
        "<unk>": 1,
        ".": 2,
        "▁の": 4,
        "▁the": 5
    }"#;

    #[test]
    fn test_vocab_load_maps_both_directions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vocab.json");
        std::fs::write(&path, VOCAB_FIXTURE).unwrap();

        let vocab = Vocab::load(&path).unwrap();
        assert_eq!(vocab.id_of("</s>"), Some(0));
        assert_eq!(vocab.id_of("▁の"), Some(4));
        assert_eq!(vocab.id_of("not in vocab"), None);
        assert_eq!(vocab.piece_of(0), Some("</s>"));
        assert_eq!(vocab.piece_of(5), Some("▁the"));
        assert_eq!(vocab.piece_of(3), Some("")); // gap id, present but unused
    }

    /// Verified against a live download of `Helsinki-NLP/opus-mt-ja-en`'s
    /// real `config.json` during development — this fixture is that file's
    /// actual field values, confirming they deserialize cleanly into
    /// `marian::Config` with no hand-translation needed.
    const OPUS_MT_JA_EN_CONFIG: &str = r#"{
        "activation_function": "swish",
        "d_model": 512,
        "decoder_attention_heads": 8,
        "decoder_ffn_dim": 2048,
        "decoder_layers": 6,
        "decoder_start_token_id": 60715,
        "decoder_vocab_size": 60716,
        "encoder_attention_heads": 8,
        "encoder_ffn_dim": 2048,
        "encoder_layers": 6,
        "eos_token_id": 0,
        "forced_eos_token_id": 0,
        "is_encoder_decoder": true,
        "max_position_embeddings": 512,
        "pad_token_id": 60715,
        "scale_embedding": true,
        "share_encoder_decoder_embeddings": true,
        "use_cache": true,
        "vocab_size": 60716
    }"#;

    #[test]
    fn test_opus_mt_ja_en_config_parses() {
        let config: marian::Config = serde_json::from_str(OPUS_MT_JA_EN_CONFIG).unwrap();
        assert_eq!(config.vocab_size, 60716);
        assert_eq!(config.decoder_vocab_size, Some(60716));
        assert_eq!(config.d_model, 512);
        assert_eq!(config.encoder_layers, 6);
        assert_eq!(config.decoder_layers, 6);
        assert_eq!(config.decoder_start_token_id, 60715);
        assert_eq!(config.eos_token_id, 0);
    }

    #[test]
    fn test_translation_cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = TranslationCache::new(dir.path());

        assert!(cache.get("浅草の仁王門の中に吊った。").is_none());
        cache.put("浅草の仁王門の中に吊った。", "Hung inside the Niō-mon gate of Asakusa.").unwrap();
        assert_eq!(cache.get("浅草の仁王門の中に吊った。").unwrap(), "Hung inside the Niō-mon gate of Asakusa.");
    }

    #[test]
    fn test_translation_cache_distinguishes_different_text() {
        let dir = tempfile::tempdir().unwrap();
        let cache = TranslationCache::new(dir.path());

        cache.put("一つ目の文。", "The first sentence.").unwrap();
        cache.put("二つ目の文。", "The second sentence.").unwrap();
        assert_eq!(cache.get("一つ目の文。").unwrap(), "The first sentence.");
        assert_eq!(cache.get("二つ目の文。").unwrap(), "The second sentence.");
    }
}
