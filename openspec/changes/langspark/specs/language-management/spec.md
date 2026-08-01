# Language Management Capability

## ADDED Requirements

### Requirement: Support multiple languages

The system SHALL support Japanese and Spanish as primary languages, with ability to add more languages in the future.

#### Scenario: User selects Japanese language
When a user selects Japanese, the system sets active language to "ja", loads JMdict/Kanjidic, configures VOICEVOX TTS, configures qwen3_asr_rs for Japanese, and displays Japanese-specific UI (Kanji tab).

#### Scenario: User selects Spanish language
When a user selects Spanish, the system sets active language to "es", loads Spanish dictionary, configures Piper TTS with Spanish model, configures qwen3_asr_rs for Spanish, and hides Japanese-specific UI (Kanji tab).

### Requirement: Language switching

The system SHALL allow users to switch between installed languages at any time, with persistence across sessions.

#### Scenario: User switches from Japanese to Spanish
When a user switches from Japanese to Spanish, the system persists selection, unloads Japanese data, loads Spanish data, clears Japanese caches, updates UI, and preserves Japanese data in database.

#### Scenario: User views available languages
When a user opens language selection, the system displays all supported languages, shows installation status, shows active language, and provides install options.

### Requirement: Language installation

The system SHALL support downloading and installing language-specific data files (dictionaries, TTS models, ASR models).

#### Scenario: User installs Japanese language data
When a user installs Japanese, the system downloads JMdict JSON, Kanjidic JSON, VOICEVOX models, shows progress, validates files, and marks as installed.

#### Scenario: User installs Spanish language data
When a user installs Spanish, the system downloads Spanish dictionary JSON, Piper Spanish model, shows progress, validates files, and marks as installed.

### Requirement: Performance

The system SHALL provide fast language switching and loading.

#### Scenario: User switches between installed languages
With Japanese and Spanish installed, language switching completes in under 2 seconds, dictionary loading in under 3 seconds, and UI updates within 500ms.
