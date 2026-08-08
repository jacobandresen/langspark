# AGENTS.md

This file contains instructions for agents working on the LangSpark project.

## Working from intent, not a spec doc

There is no separate specification document — `README.md` and `ARCHITECTURE.md` describe current behavior and design decisions; the user's own requests drive what gets built next. When a request is ambiguous, ask rather than guessing.

**Do not implement features nobody asked for.** Don't add complexity beyond what the current task requires.

## Implementation Workflow

1. Understand the request and its scope before writing code
2. Write the minimal code to satisfy it
3. Write tests that verify the behavior
4. Verify all existing tests still pass

## Implementation Guidelines

### Human Readability

Code shall be written first and foremost for human understanding. The implementation must prioritize clarity, consistency, and maintainability over cleverness or brevity.

- **Descriptive naming**: Use names that reveal intent. Prefer `calculate_next_review_date()` over `calc_date()`. Avoid abbreviations unless they are widely understood in the domain (e.g., SRS for Spaced Repetition System).
- **Small, focused functions**: Each function shall do one thing and do it well. Aim for functions under 20-30 lines. If a function needs a comment to explain what it does, consider splitting it.
- **Explicit over implicit**: Make behavior obvious. Avoid hidden side effects, complex control flow, and magical values. Use enums instead of integers for state (e.g., `CardState::New` not `0`).
- **Consistent style**: Follow Rust idioms consistently throughout. Use the same pattern for similar problems rather than inventing new approaches each time.
- **Self-documenting code**: Structure code so its purpose is evident without comments. Use clear types, well-named variables, and logical flow. Comments should explain *why*, not *what*.

### Modular Architecture

The implementation shall follow a modular design where each component has a single, well-defined responsibility. Modules shall be loosely coupled, with clear interfaces that minimize dependencies between them. This approach enables easier testing, maintenance, and future extension.

The Rust workspace structure (`langspark-core` and `langspark-gui`) shall be preserved, with `langspark-core` containing all business logic independent of the UI, and `langspark-gui` handling presentation and user interaction exclusively.

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

- Dictionary datasets (JMdict, Kanjidic)
- TTS models (VOICEVOX)
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
└── README.md              # Global data license overview
```

## Code Quality

- Follow Rust best practices and idiomatic patterns
- Use consistent naming conventions throughout
- Document all public APIs with doc comments
- Handle errors gracefully with appropriate error types
- Avoid unsafe code unless absolutely necessary
- Keep dependencies minimal and well-justified

## Documentation Standards

Human-readable code requires human-readable documentation.

- **Module-level docs**: Every module shall have a doc comment explaining its purpose, responsibilities, and how it fits into the larger system. Include examples of typical usage where helpful.
- **Public API docs**: All public functions, structs, enums, and traits shall have doc comments explaining their purpose, parameters, return values, and any invariants.
- **Example code**: Documentation shall include compileable examples where possible, using Rust's `///` doc comment syntax.
- **Change documentation**: Significant changes shall be documented with clear rationale. Use git commit messages that explain the *why* behind changes, not just the *what*.

## Code Review Guidelines

To maintain human readability across the codebase:

- **Review for clarity first**: Does the code clearly express its intent? If not, request improvements before approving.
- **Question abbreviations**: Any non-obvious abbreviation shall be defined in comments or documentation.
- **Enforce consistency**: New code shall match the style and patterns of existing code in the same module.
- **Prefer simplicity**: Reject clever solutions that sacrifice readability. Complex problems deserve simple, clear solutions.
- **Document assumptions**: Code that relies on non-obvious behavior or external constraints shall document those assumptions inline.

## Before Committing

- Run `cargo test` to ensure all tests pass
- Run `cargo clippy` to catch linting issues
- Run `cargo fmt` to ensure consistent formatting
- Verify no license references are missing for data usage
- Update `README.md`/`ARCHITECTURE.md` if the change affects what they describe
