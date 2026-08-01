//! Audio processing module
//!
//! Handles audio capture, playback, TTS, and speech recognition.

/// Manages audio operations including recording, playback, and TTS/ASR coordination
pub struct AudioManager;

/// Scores pronunciation by comparing recognized speech to expected text
pub struct PronunciationScorer;

/// Handles speech recognition using qwen3_asr_rs
pub struct SpeechRecognizer;

/// Trait for text-to-speech engines
pub trait TtsBackend {
    /// Synthesize text to speech
    fn synthesize(&self, text: &str) -> Result<Vec<u8>, anyhow::Error>;
}
