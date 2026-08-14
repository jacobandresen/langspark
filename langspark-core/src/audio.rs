//! Audio capture, playback, and caching.
//!
//! TTS (`tts.rs`), ASR (`asr.rs`), and pronunciation scoring (`pronunciation.rs`)
//! are separate modules; this one only deals with raw PCM audio: recording
//! from the microphone, playing WAV bytes back, extracting waveform data for
//! visualization, and caching generated TTS audio on disk.

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Sample rate used throughout LangSpark for recording and generated audio,
/// per the pronunciation-practice spec ("captures audio ... at 44.1kHz").
pub const SAMPLE_RATE: u32 = 44_100;

/// Encode mono f32 PCM samples as a WAV byte buffer.
pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut buffer, spec).context("failed to create WAV writer")?;
        for &sample in samples {
            writer.write_sample(sample).context("failed to write WAV sample")?;
        }
        writer.finalize().context("failed to finalize WAV")?;
    }
    Ok(buffer.into_inner())
}

/// Decode a WAV byte buffer into mono f32 PCM samples (channels beyond the
/// first are dropped) and its sample rate.
pub fn decode_wav(wav_bytes: &[u8]) -> Result<(Vec<f32>, u32)> {
    let cursor = Cursor::new(wav_bytes);
    let mut reader = hound::WavReader::new(cursor).context("failed to read WAV")?;
    let spec = reader.spec();
    let channels = spec.channels as usize;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .step_by(channels.max(1))
            .collect::<std::result::Result<Vec<_>, _>>()?,
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .step_by(channels.max(1))
                .map(|s| s.map(|v| v as f32 / max))
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
    };

    Ok((samples, spec.sample_rate))
}

/// Downsample audio samples to `num_points` peak-amplitude buckets, suitable
/// for drawing a waveform widget without rendering every raw sample.
pub fn extract_waveform(samples: &[f32], num_points: usize) -> Vec<f32> {
    if samples.is_empty() || num_points == 0 {
        return Vec::new();
    }
    let chunk_size = (samples.len() as f64 / num_points as f64).ceil() as usize;
    let chunk_size = chunk_size.max(1);

    samples
        .chunks(chunk_size)
        .map(|chunk| chunk.iter().fold(0.0f32, |max, &s| max.max(s.abs())))
        .collect()
}

/// Records audio from the default input device using CPAL.
///
/// Recording happens on a background CPAL stream that pushes samples into a
/// shared buffer; call `stop` to tear the stream down and retrieve the result.
pub struct AudioRecorder {
    buffer: Arc<Mutex<Vec<f32>>>,
    stream: Option<cpal::Stream>,
    sample_rate: u32,
}

impl AudioRecorder {
    /// Start recording from the system's default input device.
    pub fn start() -> Result<Self> {
        Self::start_with_device(None)
    }

    /// Start recording from `device_name` (matched against
    /// [`list_audio_devices`]'s input names), falling back to the system
    /// default input device if `device_name` is `None` or no longer matches
    /// any connected device (e.g. it was unplugged since being selected in
    /// Preferences).
    pub fn start_with_device(device_name: Option<&str>) -> Result<Self> {
        let host = cpal::default_host();
        let device = device_name
            .and_then(|name| host.input_devices().ok()?.find(|d| d.name().map(|n| n == name).unwrap_or(false)))
            .or_else(|| host.default_input_device())
            .context("no audio input device available")?;
        let config = device
            .default_input_config()
            .context("failed to get default input config")?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buffer_clone = buffer.clone();

        let err_fn = |err| log::error!("audio input stream error: {err}");
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    // `data` is interleaved per-frame (e.g. LRLRLR for a
                    // stereo device) at `channels` samples per frame — the
                    // rest of this module (encoding, scoring, ASR) assumes
                    // mono, so downmix by averaging each frame's channels
                    // rather than storing the raw interleaved stream as if
                    // it were already mono (that would double the sample
                    // count per second of real time and scramble L/R samples
                    // together, making played-back audio garbled and roughly
                    // half-speed).
                    let mut mono = buffer_clone.lock().unwrap();
                    mono.extend(data.chunks(channels.max(1)).map(|frame| frame.iter().sum::<f32>() / frame.len() as f32));
                },
                err_fn,
                None,
            ),
            other => anyhow::bail!("unsupported input sample format: {other:?}"),
        }
        .context("failed to build input stream")?;

        stream.play().context("failed to start input stream")?;

        Ok(Self {
            buffer,
            stream: Some(stream),
            sample_rate,
        })
    }

    /// Stop recording and return the captured samples and their sample rate.
    pub fn stop(mut self) -> (Vec<f32>, u32) {
        // Dropping the stream halts callbacks.
        self.stream.take();
        let samples = std::mem::take(&mut *self.buffer.lock().unwrap());
        (samples, self.sample_rate)
    }
}

/// Plays back WAV audio using `rodio`.
pub struct AudioPlayer {
    _stream: rodio::OutputStream,
    handle: rodio::OutputStreamHandle,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        let (stream, handle) = rodio::OutputStream::try_default().context("failed to open audio output device")?;
        Ok(Self { _stream: stream, handle })
    }

    /// Play WAV bytes and block until playback finishes.
    pub fn play_wav_blocking(&self, wav_bytes: Vec<u8>) -> Result<()> {
        let cursor = Cursor::new(wav_bytes);
        let sink = rodio::Sink::try_new(&self.handle).context("failed to create audio sink")?;
        let source = rodio::Decoder::new(cursor).context("failed to decode WAV audio")?;
        sink.append(source);
        sink.sleep_until_end();
        Ok(())
    }
}

/// Caches generated TTS audio to disk, keyed by language + voice + text, so
/// repeated playback of the same word doesn't re-synthesize it.
pub struct AudioCache {
    cache_dir: PathBuf,
}

impl AudioCache {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self { cache_dir: cache_dir.into() }
    }

    fn cache_key(&self, language: &str, voice: &str, text: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        format!("{language}_{voice}_{:x}.wav", hasher.finish())
    }

    fn cache_path(&self, language: &str, voice: &str, text: &str) -> PathBuf {
        self.cache_dir.join(self.cache_key(language, voice, text))
    }

    /// Return cached WAV bytes for this (language, voice, text), if present.
    pub fn get(&self, language: &str, voice: &str, text: &str) -> Option<Vec<u8>> {
        std::fs::read(self.cache_path(language, voice, text)).ok()
    }

    /// Store WAV bytes for this (language, voice, text) in the cache.
    pub fn put(&self, language: &str, voice: &str, text: &str, wav_bytes: &[u8]) -> Result<()> {
        std::fs::create_dir_all(&self.cache_dir).context("failed to create audio cache directory")?;
        std::fs::write(self.cache_path(language, voice, text), wav_bytes).context("failed to write cached audio")?;
        Ok(())
    }

    /// Remove all cached audio files, returning how many were deleted.
    pub fn clear(&self) -> Result<usize> {
        if !self.cache_dir.exists() {
            return Ok(0);
        }
        let mut removed = 0;
        for entry in std::fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            if entry.path().extension().and_then(|e| e.to_str()) == Some("wav") {
                std::fs::remove_file(entry.path())?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Total size in bytes of everything currently cached.
    pub fn size_bytes(&self) -> u64 {
        if !self.cache_dir.exists() {
            return 0;
        }
        std::fs::read_dir(&self.cache_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }
}

/// Coordinates audio operations (recording, playback, TTS/ASR dispatch) for
/// the active language. Owns the on-disk audio cache; TTS/ASR backends
/// themselves live in `tts.rs`/`asr.rs` and are selected by the caller based
/// on `crate::language::Language`.
pub struct AudioManager {
    cache: AudioCache,
}

impl AudioManager {
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self { cache: AudioCache::new(cache_dir) }
    }

    pub fn cache(&self) -> &AudioCache {
        &self.cache
    }

    /// Play WAV bytes through the default output device, blocking until done.
    pub fn play(&self, wav_bytes: Vec<u8>) -> Result<()> {
        AudioPlayer::new()?.play_wav_blocking(wav_bytes)
    }

    /// Start recording from the microphone.
    pub fn start_recording(&self) -> Result<AudioRecorder> {
        AudioRecorder::start()
    }
}

/// Peak absolute amplitude across `samples` (0.0 for empty input). Used to
/// give a pass/fail read on whether a microphone is actually picking up
/// sound, e.g. for a "Test Mic" button — silence and a disconnected/muted
/// mic both record fine but stay near 0.0.
pub fn peak_level(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |max, &s| max.max(s.abs()))
}

/// Whether the system has usable (input device, output device) audio
/// hardware, for startup dependency checks. Never errors — a missing device
/// is a normal, checkable condition, not a failure.
pub fn audio_devices_available() -> (bool, bool) {
    let host = cpal::default_host();
    (host.default_input_device().is_some(), host.default_output_device().is_some())
}

/// Names of all available (input devices, output devices), for the device
/// picker in Preferences. Devices with unreadable names are skipped.
pub fn list_audio_devices() -> (Vec<String>, Vec<String>) {
    let host = cpal::default_host();
    let inputs = host.input_devices().map(|it| it.filter_map(|d| d.name().ok()).collect()).unwrap_or_default();
    let outputs = host.output_devices().map(|it| it.filter_map(|d| d.name().ok()).collect()).unwrap_or_default();
    (inputs, outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_roundtrip() {
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32 / 1000.0).sin()).collect();
        let wav = encode_wav(&samples, SAMPLE_RATE).unwrap();
        let (decoded, rate) = decode_wav(&wav).unwrap();

        assert_eq!(rate, SAMPLE_RATE);
        assert_eq!(decoded.len(), samples.len());
        for (a, b) in samples.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn test_extract_waveform_downsamples() {
        let samples: Vec<f32> = (0..1000).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
        let waveform = extract_waveform(&samples, 10);
        assert_eq!(waveform.len(), 10);
        // Every bucket should register the peak amplitude of 1.0
        assert!(waveform.iter().all(|&v| (v - 1.0).abs() < 1e-6));
    }

    #[test]
    fn test_extract_waveform_empty() {
        assert!(extract_waveform(&[], 10).is_empty());
        assert!(extract_waveform(&[1.0, 2.0], 0).is_empty());
    }

    #[test]
    fn test_audio_devices_available_does_not_panic() {
        // Hardware presence varies by environment; just confirm the check runs cleanly.
        let (_input, _output) = audio_devices_available();
    }

    #[test]
    fn test_list_audio_devices_does_not_panic() {
        let (_inputs, _outputs) = list_audio_devices();
    }

    #[test]
    fn test_audio_cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AudioCache::new(dir.path());

        assert!(cache.get("ja", "zundamon", "受け取る").is_none());

        let fake_wav = vec![1, 2, 3, 4];
        cache.put("ja", "zundamon", "受け取る", &fake_wav).unwrap();

        let retrieved = cache.get("ja", "zundamon", "受け取る").unwrap();
        assert_eq!(retrieved, fake_wav);

        // Different text is a cache miss
        assert!(cache.get("ja", "zundamon", "食べる").is_none());

        assert!(cache.size_bytes() > 0);
        let removed = cache.clear().unwrap();
        assert_eq!(removed, 1);
        assert!(cache.get("ja", "zundamon", "受け取る").is_none());
    }

    /// Exercises the pieces of the pronunciation-practice audio pipeline that
    /// don't need real hardware/models: synthesized samples get WAV-encoded
    /// and cached (as TTS output would be), retrieved and decoded (as
    /// playback would), downsampled into a waveform (as the UI would for
    /// display), and finally scored against expected text.
    #[test]
    fn test_pronunciation_pipeline_encode_cache_decode_waveform_score() {
        let dir = tempfile::tempdir().unwrap();
        let cache = AudioCache::new(dir.path());

        // "Synthesize": a short sine wave standing in for TTS output.
        let synthesized: Vec<f32> = (0..4410).map(|i| (i as f32 * 0.05).sin() * 0.8).collect();
        let wav = encode_wav(&synthesized, SAMPLE_RATE).unwrap();
        cache.put("ja", "zundamon", "うけとる", &wav).unwrap();

        // "Playback": fetch from cache and decode.
        let cached_wav = cache.get("ja", "zundamon", "うけとる").unwrap();
        let (decoded, rate) = decode_wav(&cached_wav).unwrap();
        assert_eq!(rate, SAMPLE_RATE);
        assert_eq!(decoded.len(), synthesized.len());

        // "Visualize": downsample for the waveform widget.
        let waveform = extract_waveform(&decoded, 50);
        assert_eq!(waveform.len(), 50);
        assert!(waveform.iter().any(|&v| v > 0.0));

        // "Score": compare a (here, perfect) recognized transcript to the expected text.
        let result = crate::pronunciation::score_pronunciation("うけとる", "うけとる", "ja");
        assert!(result.is_correct);
    }
}
