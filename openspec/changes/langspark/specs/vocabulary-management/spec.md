# Vocabulary Management Capability

## ADDED Requirements

### Requirement: Store vocabulary entries

The system SHALL store vocabulary entries with word, reading, meaning, part_of_speech, language, and proficiency_level fields.

#### Scenario: User adds a Japanese vocabulary word
When a user adds "受け取る", the system stores word, reading ("うけとる"), meaning, part_of_speech, language ("ja"), and proficiency_level ("N4").

#### Scenario: User adds a Spanish vocabulary word  
When a user adds "recibir", the system stores word, reading (phonetic), meaning, part_of_speech, language ("es"), and proficiency_level ("B1").

### Requirement: CRUD operations

The system SHALL support create, read, update, and delete operations for vocabulary entries.

#### Scenario: User creates vocabulary entry
When a user creates an entry, the system generates unique ID, validates required fields, stores in database, and returns created entry.

#### Scenario: User reads vocabulary entry
When a user requests an entry by ID, the system queries database and returns complete entry with all fields.

#### Scenario: User updates vocabulary entry
When a user updates an entry, the system validates data, updates database, updates updated_at timestamp, and returns updated entry.

#### Scenario: User deletes vocabulary entry
When a user deletes an entry, the system confirms, removes from database, cascade deletes SRS cards, and returns success.

### Requirement: Search and filter vocabulary

The system SHALL support searching and filtering vocabulary entries by word, reading, meaning, language, and proficiency level.

#### Scenario: User searches by word
When a user searches for "食べる", the system returns all entries where word contains "食べる" with fuzzy matching support.

#### Scenario: User filters by language and level
When a user filters by language="ja" and proficiency_level="N4", the system returns only Japanese N4 entries.

### Requirement: Organize vocabulary

The system SHALL support organizing vocabulary into user-defined tags and categories.

#### Scenario: User organizes by topic tags
When a user adds tag "food" to entries, the system stores tags, allows filtering by tag, and displays grouped by tag.

### Requirement: Import and export

The system SHALL support exporting vocabulary to JSON format.

#### Scenario: User exports vocabulary to JSON
When a user exports vocabulary, the system generates JSON with all entries and all fields, with optional language/tag filtering.

### Requirement: Performance

The system SHALL provide fast search and filtering for large vocabulary databases.

#### Scenario: User searches large database
With 10,000 entries, search queries complete in under 500ms, filtering in under 300ms, paginated results in under 100ms.
