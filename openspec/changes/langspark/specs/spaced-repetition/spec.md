# Spaced Repetition Capability

## ADDED Requirements

### Requirement: Implement SM-2 algorithm

The system SHALL implement the SM-2 spaced repetition algorithm for scheduling card reviews based on user performance ratings.

#### Scenario: User reviews new card first time
When a user reviews a new card and rates "Good", the system sets interval to 1 day, repetitions to 1, and schedules next review in 1 day.

#### Scenario: User reviews card second time
When a user reviews a card with 1 repetition and rates "Good", the system sets interval to 6 days, increments repetitions to 2, and schedules next review in 6 days.

#### Scenario: User reviews card with ease factor adjustment
When a user reviews a card with ease_factor=2.5 and rates "Easy", the system increases ease_factor by 0.15, calculates new interval as previous_interval * new_ease_factor, and schedules next review.

#### Scenario: User fails to recall card
When a user rates "Again", the system resets repetitions to 0, decreases ease_factor by 0.20 (minimum 1.3), and schedules next review for 1 day later.

### Requirement: Manage card states

The system SHALL manage vocabulary cards through distinct states (New, Learning, Review) as users progress through their learning journey.

#### Scenario: User adds new word to study
When a user adds "受け取る" to study deck, the system creates new SRS card in "New" state, associates with vocabulary entry, sets initial interval, and adds to review queue.

#### Scenario: Card progresses through learning stages
When a user reviews a "New" card correctly, the system moves to "Learning" state, schedules first review in 1 day. After second correct review, moves to "Review" state. After third, applies SRS interval.

### Requirement: Generate daily review queue

The system SHALL generate a daily review queue containing all cards due for review on the current day, sorted and prioritized for efficient study.

#### Scenario: System generates daily review queue
When generating queue, the system queries all cards due today, sorts by next review date (oldest first), limits to daily review limit, and returns cards with state.

### Requirement: Track statistics

The system SHALL track user progress and statistics including daily streaks, session duration, cards reviewed, and retention rates.

#### Scenario: User completes daily review
When a user completes reviews, the system increments daily streak, records session duration, updates cards reviewed count, and calculates retention rate.

### Requirement: Performance

The system SHALL maintain high performance even with large vocabulary collections.

#### Scenario: System calculates schedules for many cards
With 10,000 SRS cards, scheduling calculation completes in under 100ms, loading daily queue in under 200ms, with SRS state memory under 10MB.
