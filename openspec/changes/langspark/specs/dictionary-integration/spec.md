# Dictionary Integration Capability

## ADDED Requirements

### Requirement: Load language-specific dictionaries

The system SHALL load appropriate dictionary files based on the active language.

#### Scenario: Application loads Japanese dictionaries on startup
When the application starts with Japanese active, the system loads JMdict JSON, Kanjidic JSON, validates formats, builds search indexes, and displays progress.

#### Scenario: Application loads Spanish dictionary on startup
When the application starts with Spanish active, the system loads Spanish dictionary JSON, validates format, builds search indexes, and displays progress.

### Requirement: Parse dictionary entries

The system SHALL correctly parse dictionary file formats and extract all relevant information.

#### Scenario: System parses JMdict entry
When parsing JMdict, the system extracts entry ID, kanji elements, reading elements, sense elements with meanings, part of speech, and JLPT level if available.

#### Scenario: System parses Kanjidic entry
When parsing Kanjidic, the system extracts kanji character, Unicode code point, on_yomi, kun_yomi, meanings, stroke count, and radical information.

### Requirement: Search dictionary

The system SHALL provide fast and accurate dictionary search functionality.

#### Scenario: User performs text search
When a user searches, the system queries pre-built indexes, returns results in under 100ms, supports prefix matching and fuzzy matching.

### Requirement: Performance

The system SHALL load dictionaries quickly and use memory efficiently.

#### Scenario: Initial dictionary load
On first run, JMdict/Kanjidic load completes in under 2 seconds, search index build in under 5 seconds, with memory usage under 100MB.
