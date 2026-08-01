# UI Kiosk Capability

## ADDED Requirements

### Requirement: Application window

The system SHALL provide a main application window with custom styling and header bar displaying title and language indicator.

#### Scenario: User launches application
When the application starts, the system creates main window 800x600+, applies custom styling, displays header bar with title and language indicator, and initializes all tabs.

### Requirement: Tab navigation

The system SHALL provide tab-based navigation using GTK4 ViewStack and ViewSwitcher for smooth transitions between different functional areas.

#### Scenario: User switches between tabs
When a user clicks a tab, the system switches using ViewStack, updates ViewSwitcher, loads content if needed, and completes in under 100ms.

#### Scenario: User views Vocabulary tab
When viewing Vocabulary tab, the system displays vocabulary grouped by language-specific levels, shows cards horizontally (max 8 per row), provides "Show All" for large groups, and opens detail dialog on click.

#### Scenario: User views Kanji tab (Japanese only)
When viewing Kanji tab with Japanese active, the system displays kanji grouped by JLPT/grade/radius, shows cards horizontally, provides "Show All", opens detail dialog on click, and hides tab when Spanish is active.

#### Scenario: User views Review tab
When viewing Review tab, the system displays all cards due today, shows card front by default, provides rating buttons (Again, Hard, Good, Easy), shows progress (X of Y), and flips card on click.

#### Scenario: User views Pronunciation tab
When viewing Pronunciation tab, the system displays word selection, provides Play/Record/Stop buttons, displays waveform, shows score and feedback after recording.

#### Scenario: User views Statistics tab
When viewing Statistics tab, the system displays overall progress, daily streak, review history chart, next reviews schedule, and per-deck statistics.

### Requirement: Header bar with language indicator

The system SHALL display the current active language with name, flag emoji, and visual indicator in the header bar across all tabs.

#### Scenario: User sees language indicator
For any active tab, the system displays current language name, flag emoji, and visual indicator of active language.

### Requirement: Detail dialogs

The system SHALL provide modal detail dialogs for vocabulary and kanji entries with comprehensive information and interactive controls.

#### Scenario: User opens vocabulary detail dialog
When clicking a vocabulary card, the system opens modal with word details, large word with phonetic guide, meaning/part_of_speech/level, example sentence, audio controls, pronunciation practice button, and add-to-deck option.

#### Scenario: User opens kanji detail dialog
When clicking a kanji card (Japanese only), the system opens modal with large kanji, all readings (on/kun), all meanings, stroke count, radical, and audio for each reading.

### Requirement: Keyboard shortcuts

The system SHALL support keyboard shortcuts for efficient review and navigation throughout the application.

#### Scenario: User uses keyboard for review
During review, Space reveals answer, keys 1-4 rate card, Escape closes dialogs, Ctrl+P plays, Ctrl+R records, Ctrl+S stops.

### Requirement: Performance

The system SHALL maintain smooth UI performance with responsive interactions and minimal resource usage.

#### Scenario: User interacts with UI
UI remains responsive at 60fps, tab switching under 100ms, card loading smooth, UI memory under 150MB.
