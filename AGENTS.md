# AGENTS.md

This file contains instructions for agents working on the LangSpark project.

## Implementation Guidelines

### Modular Architecture

The implementation shall follow a modular design where each component has a single, well-defined responsibility. Modules shall be loosely coupled, with clear interfaces that minimize dependencies between them. This approach enables easier testing, maintenance, and future extension.

The Rust workspace structure (`jv-core` and `jv-gui`) shall be preserved, with `jv-core` containing all business logic independent of the UI, and `jv-gui` handling presentation and user interaction exclusively.

### Testability

Each module shall be designed for testability from the outset. This means:

- Pure functions where possible, with side effects isolated at module boundaries
- Dependency injection for external services (database, audio, TTS, ASR)
- Clear separation of concerns to enable unit testing in isolation

## Testing Requirements

### Unit Tests

Every module shall have comprehensive unit tests covering:

- All public functions and methods
- Edge cases and error conditions
- Input validation and boundary conditions
- State transitions and invariants

Unit tests shall execute quickly and without external dependencies. Use mocking or test doubles for external services.

### Integration Tests

Integration tests shall verify that modules work correctly together. These tests may involve:

- Multiple modules interacting through their public interfaces
- Database operations with a test database instance
- Audio processing pipelines
- End-to-end workflows such as: adding a word, scheduling reviews, completing pronunciation practice

Integration tests shall be marked separately from unit tests and may have longer execution times.

### Test Coverage

Aim for high test coverage, particularly for:

- Core SRS algorithm calculations
- Dictionary querying and parsing
- Audio capture and processing
- Speech recognition integration
- Database operations and data persistence

Coverage gaps shall be identified and addressed before merging significant changes.

## Data Usage

### Free and Open Data

All data used by LangSpark shall be freely available under permissive licenses. This includes:

- Dictionary datasets (JMdict, Kanjidic, SpanDict)
- TTS models (VOICEVOX, Piper)
- ASR models (qwen3_asr_rs)
- Any other language resources

### License References

Every time data is used, loaded, or referenced in the codebase, there shall be a clear reference to its license. This includes:

- **Module-level documentation**: Each module that uses external data shall document the data sources and their licenses in the module docstring
- **Function-level comments**: Functions that directly access data files shall include a comment referencing the license
- **Download/installation code**: Any code that fetches or installs data shall include license information in comments or documentation

Example format:
```rust
// Data source: JMdict (https://www.edrdg.org/jmdict/readme.html)
// License: Creative Commons Attribution-ShareAlike License (CC BY-SA)
// See: https://creativecommons.org/licenses/by-sa/2.5/
```

### Data Directory Structure

Language-specific data shall be organized in the `data/` directory with subdirectories for each language. Each language directory shall contain a `LICENSE` or `README` file documenting the data sources and their licenses.

```
data/
├── ja/
│   ├── dictionaries/
│   │   ├── jmdict.json
│   │   └── kanjidic.json
│   ├── tts/
│   └── README.md          # Contains license information
├── es/
│   ├── dictionaries/
│   │   └── spandict.json
│   ├── tts/
│   └── README.md          # Contains license information
└── README.md              # Global data license overview
```

## Code Quality

- Follow Rust best practices and idiomatic patterns
- Use consistent naming conventions throughout
- Document all public APIs with doc comments
- Handle errors gracefully with appropriate error types
- Avoid unsafe code unless absolutely necessary
- Keep dependencies minimal and well-justified

## Before Committing

- Run `cargo test` to ensure all tests pass
- Run `cargo clippy` to catch linting issues
- Run `cargo fmt` to ensure consistent formatting
- Verify no license references are missing for data usage
