# Kanji Lookup Capability

## ADDED Requirements

### Requirement: Kanji data model

The system SHALL store kanji characters with meanings, readings, stroke count, and radical information.

#### Scenario: User looks up kanji character
When a user searches for "受", the system retrieves character, meanings ["receive", "accept", "take"], on_readings ["ジュ"], kun_readings ["う.ける", "う.け", "-う.け"], stroke_count (11), and radical information.

### Requirement: Kanji lookup operations

The system SHALL support searching kanji by character, reading, meaning, radical, and stroke count.

#### Scenario: User searches kanji by character
When a user searches for "受", the system returns exact match with all associated data and displays kanji prominently.

#### Scenario: User searches kanji by reading
When a user searches for on-reading "ジュ", the system returns all kanji with "ジュ" in on_readings (受, 授, 住) sorted by relevance.

#### Scenario: User searches kanji by meaning
When a user searches for meaning "receive", the system returns all kanji with "receive" in meanings with highlighted matches.

### Requirement: Kanji display

The system SHALL display kanji information in a user-friendly format.

#### Scenario: User views kanji detail
When a user views "受" details, the system displays large rendering, all on readings, all kun readings, all meanings, stroke count, and radical information.

### Requirement: Performance

The system SHALL provide fast kanji lookup.

#### Scenario: User performs kanji lookup
For any kanji lookup, the system returns results in under 100ms without visible lag.
