//! Speech recognition.
//!
//! Real transcription uses `qwen3-asr-rs`, gated behind the `asr` Cargo
//! feature because its only backends are `tch` (needs a native libtorch
//! install) and `mlx` (Apple Silicon only) — neither is guaranteed to be
//! present, so the default build must not require either. Without the
//! feature, [`SpeechRecognizer`] reports a clear "unavailable" error instead
//! of failing to compile, per design.md's graceful-degradation goal.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Result of transcribing an audio clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    pub text: String,
    /// qwen3-asr-rs doesn't report a numeric confidence score, so this is
    /// reserved for a backend that does (or a future heuristic).
    pub confidence: Option<f32>,
    pub language: String,
}

/// Handles speech recognition using qwen3_asr_rs (Japanese and Spanish, among
/// 30+ languages the model supports).
pub struct SpeechRecognizer {
    language: String,
    #[cfg(feature = "asr")]
    inference: qwen3_asr_rs::inference::AsrInference,
}

impl SpeechRecognizer {
    /// Load the ASR model for a language from a directory containing
    /// `config.json`, `model.safetensors`, and `tokenizer.json`. Requires the
    /// `asr` feature and a working libtorch install; without it, `transcribe`
    /// always errors.
    #[cfg(feature = "asr")]
    pub fn new(language: &str, model_dir: &Path) -> Result<Self> {
        use qwen3_asr_rs::tensor::Device;
        let inference = qwen3_asr_rs::inference::AsrInference::load(model_dir, Device::Cpu)
            .map_err(|e| anyhow::anyhow!("failed to load qwen3 ASR model: {e}"))?;
        Ok(Self { language: language.to_string(), inference })
    }

    #[cfg(not(feature = "asr"))]
    pub fn new(language: &str, _model_dir: &Path) -> Result<Self> {
        Ok(Self { language: language.to_string() })
    }

    /// The language this recognizer was configured for
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Transcribe a WAV file at `audio_path` to text.
    #[cfg(feature = "asr")]
    pub fn transcribe(&self, audio_path: &Path) -> Result<TranscriptionResult> {
        let path_str = audio_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("audio path is not valid UTF-8"))?;
        let output = self
            .inference
            .transcribe(path_str, Some(&self.language))
            .map_err(|e| anyhow::anyhow!("qwen3 ASR transcription failed: {e}"))?;
        Ok(TranscriptionResult {
            text: strip_asr_text_marker(&output.text),
            confidence: None,
            language: self.language.clone(),
        })
    }

    #[cfg(not(feature = "asr"))]
    pub fn transcribe(&self, _audio_path: &Path) -> Result<TranscriptionResult> {
        anyhow::bail!(
            "speech recognition is unavailable: langspark-core was built without the `asr` feature \
             (requires a libtorch install; see qwen3-asr-rs docs)"
        )
    }
}

/// Strip a leading `<asr_text>` marker from qwen3-asr-rs's output.
///
/// `qwen3_asr_rs::AsrInference::transcribe`'s own output parser only strips
/// this marker when *auto-detecting* the language; when a language is
/// force-specified (as `SpeechRecognizer::transcribe` does, passing
/// `Some(&self.language)`), its `parse_asr_output` takes an early-return
/// path that skips the strip — so the marker otherwise leaks into the
/// returned text and would silently wreck downstream text comparison (e.g.
/// `pronunciation::score_pronunciation`, which expects plain recognized
/// text). This works around that rather than patching the upstream crate.
#[cfg(feature = "asr")]
fn strip_asr_text_marker(text: &str) -> String {
    text.rsplit_once("<asr_text>").map(|(_, after)| after).unwrap_or(text).trim().to_string()
}

#[cfg(all(test, feature = "asr"))]
mod asr_feature_tests {
    use super::*;

    #[test]
    fn test_strip_asr_text_marker() {
        assert_eq!(strip_asr_text_marker("<asr_text>受け取る。"), "受け取る。");
        assert_eq!(strip_asr_text_marker("受け取る。"), "受け取る。");
        assert_eq!(strip_asr_text_marker("  <asr_text>  hola  "), "hola");
    }
}

/// Normalize recognized/expected text for comparison, per language script.
/// Japanese: strip whitespace so kana output with/without spaces still matches.
/// Spanish: lowercase and fold accents (handled by `pronunciation::normalize_text`).
pub fn normalize_for_recognition(text: &str, language: &str) -> String {
    crate::pronunciation::normalize_text(text, language)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transcribe_without_asr_feature_reports_clear_error() {
        #[cfg(not(feature = "asr"))]
        {
            let recognizer = SpeechRecognizer::new("ja", Path::new("/nonexistent")).unwrap();
            let err = recognizer.transcribe(Path::new("/tmp/does-not-exist.wav")).unwrap_err();
            assert!(err.to_string().contains("asr"));
        }
    }

    #[test]
    fn test_normalize_for_recognition() {
        assert_eq!(normalize_for_recognition("Recibí", "es"), "recibi");
        assert_eq!(normalize_for_recognition("うけとる ", "ja"), "うけとる");
    }
}
