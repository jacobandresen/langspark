//! Text-to-speech: a `TtsBackend` trait plus the VOICEVOX engine LangSpark
//! uses for Japanese (its only supported language for now).

use anyhow::{Context, Result};
use std::io::Read;

/// Trait for text-to-speech engines. Implementations return WAV-encoded audio.
pub trait TtsBackend {
    /// Synthesize text to speech, returning WAV bytes.
    fn synthesize(&self, text: &str) -> Result<Vec<u8>>;
}

/// A `TtsBackend` that always fails, used when a language's voice model
/// hasn't been installed yet so the rest of the app can degrade gracefully
/// instead of crashing.
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
    /// VOICEVOX `speedScale` applied to the audio query before synthesis:
    /// 1.0 is the engine's native speaking speed, lower values slow it down.
    speed_scale: f64,
}

impl VoicevoxTts {
    pub fn new(base_url: impl Into<String>, speaker_id: u32, speed_scale: f64) -> Self {
        Self { base_url: base_url.into(), speaker_id, speed_scale }
    }

    /// Default local engine address on VOICEVOX's default port.
    pub fn default_local(speaker_id: u32, speed_scale: f64) -> Self {
        Self::new("http://127.0.0.1:50021", speaker_id, speed_scale)
    }
}

impl TtsBackend for VoicevoxTts {
    fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        // Step 1: build an audio query from the text.
        let mut query: serde_json::Value = ureq::post(&format!("{}/audio_query", self.base_url))
            .query("text", text)
            .query("speaker", &self.speaker_id.to_string())
            .call()
            .context("VOICEVOX Engine audio_query request failed (is it running?)")?
            .into_json()
            .context("failed to parse VOICEVOX audio_query response")?;
        query["speedScale"] = serde_json::json!(self.speed_scale);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unavailable_tts_reports_reason() {
        let tts = UnavailableTts::new("voice model not downloaded");
        let err = tts.synthesize("hello").unwrap_err();
        assert!(err.to_string().contains("voice model not downloaded"));
    }
}
