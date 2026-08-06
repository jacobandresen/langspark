//! Text-to-speech: a language-agnostic `TtsBackend` trait plus the two
//! concrete engines LangSpark uses — VOICEVOX (Japanese) and Piper (Spanish
//! and other languages).

use anyhow::{Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Trait for text-to-speech engines. Implementations return WAV-encoded audio.
pub trait TtsBackend {
    /// Synthesize text to speech, returning WAV bytes.
    fn synthesize(&self, text: &str) -> Result<Vec<u8>>;
}

/// A `TtsBackend` that always fails, used when a language's voice model
/// hasn't been installed yet so the rest of the app can degrade gracefully
/// instead of crashing (see design.md "Offline Model Download" risk).
pub struct UnavailableTts {
    reason: String,
}

impl UnavailableTts {
    pub fn new(reason: impl Into<String>) -> Self {
        Self { reason: reason.into() }
    }
}

impl TtsBackend for UnavailableTts {
    fn synthesize(&self, _text: &str) -> Result<Vec<u8>> {
        anyhow::bail!("TTS unavailable: {}", self.reason)
    }
}

// ---------------------------------------------------------------------
// VOICEVOX (Japanese)
// ---------------------------------------------------------------------

/// Client for a locally-running VOICEVOX Engine (<https://voicevox.hiroshiba.jp/>),
/// which exposes synthesis over a local HTTP API. We talk to it directly with
/// `ureq` rather than a wrapper crate: `voicevox_core` isn't published to
/// crates.io, and the one `voicevox-rs` release on crates.io fails to compile
/// (ambiguous `Deserialize` import in its own source).
pub struct VoicevoxTts {
    /// Base URL of the running VOICEVOX Engine, e.g. "http://127.0.0.1:50021"
    base_url: String,
    /// VOICEVOX speaker/style ID (e.g. Zundamon's normal style)
    speaker_id: u32,
}

impl VoicevoxTts {
    pub fn new(base_url: impl Into<String>, speaker_id: u32) -> Self {
        Self { base_url: base_url.into(), speaker_id }
    }

    /// Default local engine address on VOICEVOX's default port.
    pub fn default_local(speaker_id: u32) -> Self {
        Self::new("http://127.0.0.1:50021", speaker_id)
    }
}

impl TtsBackend for VoicevoxTts {
    fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        // Step 1: build an audio query from the text.
        let query: serde_json::Value = ureq::post(&format!("{}/audio_query", self.base_url))
            .query("text", text)
            .query("speaker", &self.speaker_id.to_string())
            .call()
            .context("VOICEVOX Engine audio_query request failed (is it running?)")?
            .into_json()
            .context("failed to parse VOICEVOX audio_query response")?;

        // Step 2: synthesize audio from the query.
        let response = ureq::post(&format!("{}/synthesis", self.base_url))
            .query("speaker", &self.speaker_id.to_string())
            .send_json(query)
            .context("VOICEVOX Engine synthesis request failed")?;

        let mut wav_bytes = Vec::new();
        response
            .into_reader()
            .read_to_end(&mut wav_bytes)
            .context("failed to read VOICEVOX synthesis response body")?;
        Ok(wav_bytes)
    }
}

// ---------------------------------------------------------------------
// Piper (Spanish and other languages)
// ---------------------------------------------------------------------

/// Wraps a `piper-rs` ONNX voice model for offline synthesis.
pub struct PiperTts {
    piper: std::sync::Mutex<piper_rs::Piper>,
    sample_rate: u32,
}

impl PiperTts {
    /// Load a Piper voice from its `.onnx` model and `.onnx.json` config files.
    pub fn load(model_path: &Path, config_path: &Path) -> Result<Self> {
        let piper = piper_rs::Piper::new(model_path, config_path)
            .map_err(|e| anyhow::anyhow!("failed to load Piper model: {e}"))?;
        // Sample rate isn't exposed after construction, so re-read it from the config file.
        let config_json = std::fs::read_to_string(config_path).context("failed to read Piper config")?;
        let config: serde_json::Value =
            serde_json::from_str(&config_json).context("failed to parse Piper config")?;
        let sample_rate = config["audio"]["sample_rate"].as_u64().unwrap_or(22_050) as u32;

        Ok(Self { piper: std::sync::Mutex::new(piper), sample_rate })
    }
}

impl TtsBackend for PiperTts {
    fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        let mut piper = self.piper.lock().map_err(|_| anyhow::anyhow!("Piper model lock poisoned"))?;
        let (samples, sample_rate) = piper
            .create(text, false, None, None, None, None)
            .map_err(|e| anyhow::anyhow!("Piper synthesis failed: {e}"))?;
        let sample_rate = if sample_rate > 0 { sample_rate } else { self.sample_rate };
        crate::audio::encode_wav(&samples, sample_rate)
    }
}

// ---------------------------------------------------------------------
// Voice selection configuration
// ---------------------------------------------------------------------

/// Which TTS voice to use for a language, and where its files live (Piper only).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VoiceConfig {
    pub language: String,
    pub voice_id: String,
    /// Piper only: path to the `.onnx` model file
    pub model_path: Option<PathBuf>,
    /// Piper only: path to the `.onnx.json` config file
    pub config_path: Option<PathBuf>,
}

impl VoiceConfig {
    pub fn voicevox(voice_id: &str) -> Self {
        Self {
            language: "ja".to_string(),
            voice_id: voice_id.to_string(),
            model_path: None,
            config_path: None,
        }
    }

    pub fn piper(language: &str, voice_id: &str, model_path: PathBuf, config_path: PathBuf) -> Self {
        Self {
            language: language.to_string(),
            voice_id: voice_id.to_string(),
            model_path: Some(model_path),
            config_path: Some(config_path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unavailable_tts_reports_reason() {
        let tts = UnavailableTts::new("Spanish voice model not downloaded");
        let err = tts.synthesize("hola").unwrap_err();
        assert!(err.to_string().contains("Spanish voice model not downloaded"));
    }

    #[test]
    fn test_voice_config_constructors() {
        let ja = VoiceConfig::voicevox("zundamon");
        assert_eq!(ja.language, "ja");
        assert!(ja.model_path.is_none());

        let es = VoiceConfig::piper("es", "es_es-mls-medium", "model.onnx".into(), "model.onnx.json".into());
        assert_eq!(es.language, "es");
        assert!(es.model_path.is_some());
    }
}
