# Architecture

## Workspace layout

Two crates, following breadbin's core/GUI split:

- **`langspark-core`** — all business logic, no GTK dependency. Data models,
  SQLite repositories, the SM-2 spaced-repetition backend, dictionary
  loaders, audio capture/playback/caching, TTS/ASR backends, and
  pronunciation scoring. Fully unit-testable without a display.
- **`langspark-gui`** — GTK4/libadwaita UI. Tabs, dialogs, and widgets are
  plain functions/structs that take `langspark-core` data types and return
  `gtk4::Widget`s; they don't know how that data was loaded.

## Data flow

```
SQLite (~/.local/share/langspark/langspark.db)
        │
        ▼
langspark-core repositories (SqliteVocabularyRepository, SqliteKanjiRepository, ...)
        │
        ▼
langspark-gui::state::AppState::load_tab_data()   — single load, one round trip
        │
        ▼
app::build_main_window()   — feeds each tab's build_tab(&data) function
        │
        ▼
vocabulary/kanji/review/statistics tab widgets
```

Writes flow the other way: a UI action (e.g. rating a review card) calls a
callback that runs the write on a background thread via
`task::run_blocking`, then reports success/failure back to the main thread
via `task::spawn_on_main` — GTK widgets aren't `Send`, so anything touching
them has to happen back on the GLib main context. See `review::ReviewSession`
and `app::build_main_window`'s `on_review` callback for the concrete example.

**Why a `Mutex` around the SQLite connection:** `rusqlite::Connection` is
`Send` but not `Sync`. Repositories hold `Arc<Database>` so they can be
shared with the background thread pool; `Arc<T>` is only `Send` if
`T: Send + Sync`. `Database` wraps its `Connection` in a `Mutex` specifically
so this holds — without it, no repository could be used from
`task::run_blocking` at all (this was a real bug caught while wiring
section 24's integration, not a hypothetical).

## Language-awareness

`langspark_core::LanguageManager` holds the active language and answers
"does this language support kanji," "what's its TTS voice," etc.
Every repository query that's language-scoped takes the active language code
as a parameter — there's no global "current language" state inside
`langspark-core` itself, keeping it usable from a hypothetical second UI
(TUI, web) without carrying GUI assumptions along.

Real-time language switching mid-session is an explicit non-goal (see
`openspec/changes/langspark/design.md`): switching in Preferences takes
effect on the next launch, not live.

## TTS/ASR backend choices (and why they're not what the original proposal said)

- **Japanese TTS**: the proposal called for the `voicevox_core` crate, which
  isn't published to crates.io, and the one alternative (`voicevox-rs`) fails
  to compile due to an upstream bug. `tts::VoicevoxTts` instead speaks
  directly to a locally-running [VOICEVOX Engine](https://voicevox.hiroshiba.jp/)
  over its HTTP API with `ureq` — functionally equivalent for an offline-first
  app, since VOICEVOX Engine is the thing distributing the actual voice models.
- **Spanish TTS**: `piper-rs`, built with `default-features = false` — the
  default `compile-espeak-intonations` feature's build script fails in this
  environment; the espeak-based phonemizer still works without it.
- **ASR** (`qwen3_asr_rs`): real, but its only backends need either a system
  libtorch install (`tch`) or Apple Silicon (`mlx`). It's an optional Cargo
  feature (`asr`) on `langspark-core` so the default build never needs
  either; without the feature, `SpeechRecognizer::transcribe` returns a clear
  error instead of the crate failing to compile.

## Testing GTK widgets

GTK can only be initialized from one OS thread per process, but Rust's test
harness spawns a fresh thread per `#[test]` function. Multiple tests each
calling `gtk4::init()` reliably crash the *second* one with "Attempted to
initialize GTK from two different threads." `langspark-gui`'s widget
construction is therefore smoke-tested from a single consolidated test
(`main.rs`'s `gtk_smoke` module) that builds one instance of every
tab/dialog/widget; everything else uses plain pure-logic unit tests.
