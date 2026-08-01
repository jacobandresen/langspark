# Pronunciation Practice Capability

## ADDED Requirements

### Requirement: Language-aware TTS

The system SHALL provide language-aware text-to-speech synthesis using appropriate engines for each supported language.

#### Scenario: User plays Japanese word pronunciation
When a user clicks play for "受け取る", the system uses VOICEVOX to synthesize "うけとる", plays audio, provides volume control, and caches for future playback.

#### Scenario: User plays Spanish word pronunciation
When a user clicks play for "recibir", the system uses Piper with Spanish model to synthesize "recibir", plays audio, provides volume control, and caches for future playback.

### Requirement: Audio recording

The system SHALL capture audio from the user's microphone for pronunciation practice and evaluation.

#### Scenario: User records pronunciation attempt
When a user records, the system captures audio from microphone at 44.1kHz, displays waveform in real-time, allows stopping, and saves recording for scoring.

### Requirement: Speech recognition

The system SHALL recognize spoken input using language-specific speech recognition models and provide transcription results.

#### Scenario: System recognizes Japanese speech
When user speaks "うけとる", the system uses qwen3_asr_rs with Japanese, transcribes to text, returns "うけとる" or closest match, and provides confidence score.

#### Scenario: System recognizes Spanish speech
When user speaks "recibir", the system uses qwen3_asr_rs with Spanish, transcribes to text, returns "recibir" or closest match, and provides confidence score.

### Requirement: Pronunciation scoring

The system SHALL score user pronunciation by comparing recognized speech to expected text and provide feedback.

#### Scenario: System scores Japanese pronunciation
When user records "うけとる" for "受け取る", the system normalizes texts, compares recognized to expected, calculates match percentage using Levenshtein, generates feedback, and displays score 0-100%.

#### Scenario: System scores Spanish pronunciation
When user records "recibir" for "recibir", the system normalizes texts, compares recognized to expected, calculates match percentage, generates feedback, and displays score 0-100%.

### Requirement: Display feedback

The system SHALL display comprehensive pronunciation feedback including scores, text comparison, and visual aids.

#### Scenario: User sees pronunciation feedback
After recording, the user sees overall score, recognized vs expected text, human-readable feedback, waveform comparison, and option to try again.

### Requirement: Performance

The system SHALL provide responsive and fast pronunciation practice with minimal latency.

#### Scenario: User uses pronunciation practice
TTS generation completes in under 1 second, speech recognition in under 2 seconds, scoring in under 500ms, recording latency under 100ms.
