//! Application module
//!
//! Contains the main window, application state, and coordination between UI and core.

use crate::config::Settings;
use crate::state::AppState;
use crate::{diagnostics, pronunciation, review, vocabulary};
use adw::prelude::*;
use adw::{
    Application as AdwApplication, ApplicationWindow as AdwApplicationWindow, HeaderBar, ToastOverlay, ToolbarView,
    ViewStack, ViewSwitcherTitle,
};
use gio::SimpleAction;
use gtk4::Box as GtkBox;
use langspark_core::{Language, TtsBackend};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Main application window and its toast overlay (for `diagnostics::show_error_toast`).
///
/// The Vocabulary tab is populated from `state` (loaded synchronously at
/// startup — SQLite reads of this size are fast enough not to warrant the
/// complexity of an async loading placeholder). The review tab persists each
/// rating asynchronously via `task::run_blocking` so a slow disk doesn't
/// stall the UI thread. The pronunciation tab's TTS/ASR callbacks report
/// "unavailable" until real backends are configured in Preferences, per the
/// graceful-degradation approach in `tts::UnavailableTts`.
///
/// The Kanji and Statistics tabs are temporarily dropped from the switcher
/// (not deleted — `kanji.rs`/`statistics.rs` and their data loading are
/// untouched, just unwired here) until they're revisited.
pub fn build_main_window(
    app: &AdwApplication,
    active_language: Language,
    settings: Rc<RefCell<Settings>>,
    state: Arc<AppState>,
) -> (AdwApplicationWindow, ToastOverlay) {
    let window = AdwApplicationWindow::builder()
        .application(app)
        .title("LangSpark")
        .default_width(900)
        .default_height(700)
        .build();

    let toast_overlay = ToastOverlay::new();

    let tab_data = match state.load_tab_data() {
        Ok(data) => data,
        Err(e) => {
            diagnostics::show_error_toast(&toast_overlay, &format!("Failed to load your data: {e}"));
            crate::state::TabData {
                vocabulary: Vec::new(),
                kanji: Vec::new(),
                due_cards: Vec::new(),
            }
        }
    };

    let view_stack = ViewStack::new();
    view_stack.set_vexpand(true);

    let vocab_widget = vocabulary::build_tab(
        &tab_data.vocabulary,
        vocabulary::VocabTabCallbacks {
            add_word: dictionary_add_word_callbacks(&state, active_language, &settings, &toast_overlay),
            on_play: vocab_play_callback(active_language, &settings.borrow(), &toast_overlay),
            delete: vocab_delete_callback(&state, &toast_overlay),
            example_lookup: example_lookup_callback(&state, active_language),
        },
    );
    let vocab_page = view_stack.add_titled(&vocab_widget, Some("vocabulary"), "Vocabulary");
    vocab_page.set_icon_name(Some("accessories-dictionary-symbolic"));

    let review_items = review::build_items_from_cards(&tab_data.due_cards, &tab_data.vocabulary, &tab_data.kanji);
    // Captured alongside the queue so `on_review`'s index can be mapped back
    // to the database row id `SqliteSrsRepository::update_after_review` needs.
    let review_card_ids: Vec<Option<i64>> = review_items.iter().map(|item| item.card.id).collect();
    let review_play_callback = vocab_play_callback(active_language, &settings.borrow(), &toast_overlay);
    let review_session = review::ReviewSession::new(
        review_items,
        glib::clone!(
            #[strong]
            state,
            #[strong]
            settings,
            #[weak]
            toast_overlay,
            move |index, rating| {
                let Some(Some(card_id)) = review_card_ids.get(index).copied() else {
                    return;
                };
                let algorithm = settings.borrow().srs_algorithm.clone();
                let state = state.clone();
                crate::task::spawn_on_main(async move {
                    let result = crate::task::run_blocking(move || {
                        state.srs_repo.update_after_review_with_algorithm(card_id, rating, &algorithm)
                    })
                    .await;
                    if let Err(e) = result {
                        diagnostics::show_error_toast(&toast_overlay, &format!("Failed to save review: {e}"));
                    }
                });
            }
        ),
        review_play_callback,
    );
    let review_page = view_stack.add_titled(&review_session.root, Some("review"), "Review");
    review_page.set_icon_name(Some("view-refresh-symbolic"));

    if asr_model_installed(active_language) {
        let practice_words: Vec<pronunciation::PracticeWord> = tab_data
            .vocabulary
            .iter()
            .map(|entry| pronunciation::PracticeWord { text: entry.word.clone(), reading: entry.reading.clone() })
            .collect();
        let pronunciation_tab = pronunciation::PronunciationTab::new(
            practice_words,
            pronunciation_callbacks(active_language, &settings.borrow()),
        );
        let pronunciation_page =
            view_stack.add_titled(&pronunciation_tab.widget, Some("pronunciation"), "Pronunciation");
        pronunciation_page.set_icon_name(Some("audio-input-microphone-symbolic"));
    }

    // Header: view switcher as the title, language indicator, app menu
    let switcher_title = ViewSwitcherTitle::builder().stack(&view_stack).title("LangSpark").build();

    let language_indicator = gtk4::Label::builder()
        .label(format!("{} {}", active_language.display_name(), active_language.code().to_uppercase()))
        .css_classes(["langspark-language-indicator"])
        .build();

    let menu = gio::Menu::new();
    menu.append(Some("Preferences"), Some("app.preferences"));
    menu.append(Some("Help"), Some("app.help"));
    menu.append(Some("About LangSpark"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    let menu_button = gtk4::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).build();

    let header = HeaderBar::builder().title_widget(&switcher_title).build();
    header.pack_start(&language_indicator);
    header.pack_end(&menu_button);

    let content = GtkBox::new(gtk4::Orientation::Vertical, 0);
    content.set_vexpand(true);
    content.append(&view_stack);

    let toolbar_view = ToolbarView::builder().content(&content).build();
    toolbar_view.add_top_bar(&header);

    toast_overlay.set_child(Some(&toolbar_view));
    window.set_content(Some(&toast_overlay));

    register_app_actions(app, &window, settings);

    (window, toast_overlay)
}

/// Register app-level actions (`app.preferences`, `app.about`, `app.quit`)
/// backing the header menu, per task 11.6/11.7.
fn register_app_actions(app: &AdwApplication, window: &AdwApplicationWindow, settings: Rc<RefCell<Settings>>) {
    let quit_action = SimpleAction::new("quit", None);
    quit_action.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| app.quit()
    ));
    app.add_action(&quit_action);

    let about_action = SimpleAction::new("about", None);
    about_action.connect_activate(glib::clone!(
        #[weak]
        window,
        move |_, _| {
            let about = adw::AboutWindow::builder()
                .transient_for(&window)
                .application_name("LangSpark")
                .application_icon("org.langspark.LangSpark")
                .version(env!("CARGO_PKG_VERSION"))
                .developer_name("Jacob Andresen")
                .license_type(gtk4::License::Gpl30)
                .comments("Offline-first vocabulary and pronunciation practice")
                .build();
            about.present();
        }
    ));
    app.add_action(&about_action);

    let help_action = SimpleAction::new("help", None);
    help_action.connect_activate(glib::clone!(
        #[weak]
        window,
        move |_, _| {
            let body = gtk4::Label::builder()
                .label(HELP_TEXT)
                .wrap(true)
                .xalign(0.0)
                .margin_top(16)
                .margin_bottom(16)
                .margin_start(16)
                .margin_end(16)
                .build();
            let scroller = gtk4::ScrolledWindow::builder().child(&body).build();
            let dialog = adw::Dialog::builder()
                .title("Help")
                .content_width(420)
                .content_height(480)
                .child(&scroller)
                .build();
            dialog.present(Some(&window));
        }
    ));
    app.add_action(&help_action);

    let preferences_action = SimpleAction::new("preferences", None);
    preferences_action.connect_activate(glib::clone!(
        #[weak]
        window,
        #[strong]
        settings,
        move |_, _| {
            let dialog = crate::preferences::build(settings.clone(), move |updated| {
                if let Some(dirs) = crate::config::AppDirs::new() {
                    if let Err(e) = updated.save(&dirs.config_file()) {
                        log::warn!("failed to save settings: {e}");
                    }
                }
            });
            dialog.present(Some(&window));
        }
    ));
    app.add_action(&preferences_action);
}

/// Build the vocabulary detail dialog's example-sentence lookup from the
/// dictionary loaded into `state`, or `None` if no dictionary is installed
/// for `active_language` (in which case the dialog just always shows "no
/// example available" — see `vocabulary/dialog.rs`).
fn example_lookup_callback(
    state: &Arc<AppState>,
    active_language: Language,
) -> Option<Rc<dyn Fn(&str) -> Vec<langspark_core::ExampleSentence>>> {
    let code = active_language.code();
    if !state.dictionary.is_loaded(code) {
        return None;
    }
    let state = state.clone();
    Some(Rc::new(move |word: &str| state.dictionary.examples_for(code, word)))
}

/// Build the "Add Word" callbacks for the vocabulary tab from the dictionary
/// loaded into `state` (see `AppState::open`), or `None` if no dictionary is
/// installed for `active_language` — the "Add Word" button then stays hidden
/// until one is installed from Preferences > Language Installation.
fn dictionary_add_word_callbacks(
    state: &Arc<AppState>,
    active_language: Language,
    settings: &Rc<RefCell<Settings>>,
    toast_overlay: &ToastOverlay,
) -> Option<vocabulary::AddWordCallbacks> {
    let code = active_language.code();
    if !state.dictionary.is_loaded(code) {
        return None;
    }

    let search: Rc<dyn Fn(&str) -> Vec<langspark_core::VocabEntry>> = {
        let state = state.clone();
        Rc::new(move |query: &str| state.dictionary.search(code, query).into_iter().cloned().collect())
    };

    let persist: Rc<
        dyn Fn(langspark_core::VocabEntry, Box<dyn Fn(langspark_core::VocabularyEntry)>, Box<dyn Fn()>),
    > = Rc::new(glib::clone!(
        #[strong]
        state,
        #[strong]
        settings,
        #[weak]
        toast_overlay,
        move |dict_entry: langspark_core::VocabEntry,
              on_done: Box<dyn Fn(langspark_core::VocabularyEntry)>,
              on_error: Box<dyn Fn()>| {
            let new_entry = langspark_core::VocabularyEntry {
                id: None,
                word: dict_entry.word.clone(),
                reading: dict_entry.reading.clone(),
                meaning: dict_entry.meanings.join("; "),
                language: dict_entry.language.clone(),
                level: dict_entry.level.clone(),
                part_of_speech: dict_entry.part_of_speech.first().cloned(),
                tags: None,
                created_at: None,
                updated_at: None,
            };
            // Read live (the user may have changed it in Preferences since
            // this callback was built) rather than capturing it once at startup.
            let starting_ease_factor = settings.borrow().starting_ease_factor;
            let state = state.clone();
            crate::task::spawn_on_main(async move {
                let to_insert = new_entry.clone();
                let result = crate::task::run_blocking(move || -> anyhow::Result<i64> {
                    let vocab_id = state.vocabulary_repo.create(&to_insert)?;
                    // Newly-added words are immediately due for review: also
                    // create the SRS card that puts them in the Review tab's
                    // queue (see `SrsCard::is_due_today`, true when unreviewed).
                    let mut card = langspark_core::SrsCard::new("vocabulary", &to_insert.language);
                    card.vocab_id = Some(vocab_id);
                    card.ease_factor = starting_ease_factor;
                    state.srs_repo.create(&card)?;
                    Ok(vocab_id)
                })
                .await;
                match result {
                    Ok(id) => {
                        let mut persisted = new_entry;
                        persisted.id = Some(id);
                        on_done(persisted);
                    }
                    Err(e) => {
                        diagnostics::show_error_toast(&toast_overlay, &format!("Failed to add word: {e}"));
                        on_error();
                    }
                }
            });
        }
    ));

    Some(vocabulary::AddWordCallbacks { search, persist })
}

const HELP_TEXT: &str = "\
Vocabulary — Browse entries grouped by level. Click \"Show All\" to see \
every entry in a section as a grid.

Review — Cards due today appear one at a time. Click \"Show Answer\" to reveal \
the back, then rate how well you remembered it (Again/Hard/Good/Easy). Your \
rating adjusts when the card comes up next.

Pronunciation — Pick a word, click Play to hear a reference pronunciation, \
then Record to attempt it yourself. You'll get a score and feedback. Only \
appears once a speech recognition model is installed (see README.md); also \
requires a TTS backend configured in Preferences to hear the reference \
pronunciation.

Preferences — Change the active language (takes effect on restart), TTS \
voices, SRS algorithm, and audio devices.";

fn unavailable_pronunciation_callbacks(
    active_language: Language,
    device_name: Option<String>,
) -> pronunciation::PronunciationCallbacks {
    let code = active_language.code();
    pronunciation::PronunciationCallbacks {
        synthesize: Box::new(|_| anyhow::bail!("no TTS backend configured yet — set one up in Preferences")),
        record: build_record(device_name),
        record_duration: RECORDING_DURATION,
        play: Box::new(|_| anyhow::bail!("no audio output backend configured yet")),
        score: Box::new(move |r, e| langspark_core::score_pronunciation(r, e, code)),
        transcribe: build_transcribe(active_language),
    }
}

/// Whether a speech recognition model directory exists at
/// `AppDirs::asr_model_dir` for `active_language` — gates whether the
/// Pronunciation tab is shown at all, since without a model, Record would
/// only ever produce `build_transcribe`'s "no speech recognition model
/// installed" error. Doesn't check the directory's contents (`config.json`,
/// `model.safetensors`, `tokenizer.json`) or whether langspark-core was
/// built with the `asr` Cargo feature — either of those still surfaces as a
/// runtime error from `build_transcribe` if the model is otherwise incomplete.
fn asr_model_installed(active_language: Language) -> bool {
    crate::config::AppDirs::new()
        .map(|d| d.asr_model_dir(active_language.code()))
        .is_some_and(|dir| dir.exists())
}

/// Build the `transcribe` callback: runs `SpeechRecognizer` (see `asr.rs`)
/// against a `qwen3` ASR model directory expected at
/// `AppDirs::asr_model_dir`. There's no installer for that model yet (unlike
/// the Japanese dictionary), so this only produces real transcriptions once
/// one has been placed there by hand *and* langspark-core was built with the
/// `asr` Cargo feature (needs a native libtorch install) — `SpeechRecognizer`
/// itself reports a clear "unavailable" error otherwise in both cases, so no
/// feature-gating is needed here.
fn build_transcribe(active_language: Language) -> Box<dyn Fn(&[f32], u32) -> anyhow::Result<String> + Send + Sync> {
    let code = active_language.code().to_string();
    let model_dir = crate::config::AppDirs::new().map(|d| d.asr_model_dir(&code));

    Box::new(move |samples: &[f32], sample_rate: u32| {
        let dir = model_dir.clone().ok_or_else(|| anyhow::anyhow!("couldn't determine the ASR model directory"))?;
        if !dir.exists() {
            anyhow::bail!(
                "no speech recognition model installed at {} (needs config.json, model.safetensors, \
                 tokenizer.json from a qwen3 ASR model)",
                dir.display()
            );
        }

        let recognizer = langspark_core::SpeechRecognizer::new(&code, &dir)?;

        // SpeechRecognizer::transcribe reads a WAV file path rather than raw
        // samples, so round-trip through a scratch file.
        let wav = langspark_core::audio::encode_wav(samples, sample_rate)?;
        let tmp_path = std::env::temp_dir().join(format!("langspark-asr-{}-{}.wav", std::process::id(), code));
        std::fs::write(&tmp_path, &wav)?;
        let result = recognizer.transcribe(&tmp_path);
        let _ = std::fs::remove_file(&tmp_path);

        Ok(result?.text)
    })
}

/// How long the Pronunciation tab's "Record" button captures for. There's
/// only a single Record button (no separate Stop) — see `pronunciation/mod.rs`.
const RECORDING_DURATION: std::time::Duration = std::time::Duration::from_secs(3);

/// Build the `record` callback shared by every language: capture
/// `RECORDING_DURATION` of audio via `cpal` (`AudioRecorder`), from
/// `device_name` if set (the `audio_input_device` Preference) or the system
/// default input device otherwise. Doesn't depend on TTS/dictionary
/// availability, so it's wired the same way regardless of which language is
/// active.
fn build_record(device_name: Option<String>) -> Box<dyn Fn() -> anyhow::Result<(Vec<f32>, u32)> + Send + Sync> {
    Box::new(move || {
        // `AudioRecorder` (and the `cpal::Stream` it owns) isn't `Send`, but
        // that's fine here: it's created, used, and dropped entirely within
        // this closure's single call to `task::run_blocking`, never crossing
        // a thread boundary itself — only this closure (which captures a
        // plain `String`) needs to be `Send`.
        let recorder = langspark_core::AudioRecorder::start_with_device(device_name.as_deref())?;
        std::thread::sleep(RECORDING_DURATION);
        Ok(recorder.stop())
    })
}

/// If a native VOICEVOX Engine is installed (see
/// `langspark_core::install_voicevox_engine`, wired to Preferences' "Install"
/// button in the Data Sources page) and nothing is already listening on its
/// port — a Docker-based engine (`scripts/setup-voicevox.sh`) or a manual
/// launch, say — start it as a detached background process, so Japanese TTS
/// works without the user manually starting anything each session.
/// Best-effort: failures are only logged, not surfaced to the user, since
/// VOICEVOX is optional and its absence is already reported through
/// `build_synthesize`'s normal "couldn't reach VOICEVOX Engine" error path.
pub fn spawn_voicevox_engine_if_installed(dirs: Option<&crate::config::AppDirs>) {
    let Some(dirs) = dirs else { return };
    let run_path = dirs.voicevox_engine_dir().join("run");
    if !run_path.exists() {
        return;
    }

    let addr: std::net::SocketAddr = "127.0.0.1:50021".parse().expect("valid socket address literal");
    if std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).is_ok() {
        return; // something (this engine from a previous launch, Docker, ...) is already listening
    }

    match std::process::Command::new(&run_path)
        .args(["--host", "127.0.0.1", "--port", "50021"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => log::info!("started VOICEVOX Engine from {}", run_path.display()),
        Err(e) => log::warn!("failed to start VOICEVOX Engine at {}: {e}", run_path.display()),
    }
}

/// Resolve a VOICEVOX speaker ID from the free-text `tts_voice_ja` setting: a
/// bare number is used directly (full control over any installed speaker),
/// otherwise a few well-known default VOICEVOX character names resolve to
/// their public speaker IDs (stable across VOICEVOX Engine installs),
/// falling back to Zundamon Normal — the project's documented default voice
/// (see `language.rs`'s `default_tts_voice`).
fn resolve_voicevox_speaker_id(voice: &str) -> u32 {
    const ZUNDAMON_NORMAL: u32 = 3;
    let voice = voice.trim();
    if let Ok(id) = voice.parse::<u32>() {
        return id;
    }
    match voice.to_lowercase().as_str() {
        "shikoku metan" | "metan" | "四国めたん" => 2,
        "zundamon" | "ずんだもん" => ZUNDAMON_NORMAL,
        "kasukabe tsumugi" | "tsumugi" | "春日部つむぎ" => 8,
        _ => ZUNDAMON_NORMAL,
    }
}

/// Build a `synthesize(text) -> WAV bytes` closure for `active_language`
/// (currently always Japanese, spoken through VOICEVOX), shared between the
/// pronunciation tab's Play button and the vocabulary detail dialog's Play
/// button (`vocab_play_callback`). Synthesized audio is cached to disk so
/// repeat playback of the same word doesn't re-synthesize it.
fn build_synthesize(
    active_language: Language,
    settings: &Settings,
) -> Option<Box<dyn Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync>> {
    let code = active_language.code();
    let cache_dir = crate::config::AppDirs::new().map(|d| d.audio_cache_dir());
    let speaker_id = resolve_voicevox_speaker_id(&settings.tts_voice_ja);
    let voice_label = settings.tts_voice_ja.clone();

    Some(Box::new(move |text: &str| {
        if let Some(dir) = &cache_dir {
            if let Some(cached) = langspark_core::AudioCache::new(dir.clone()).get(code, &voice_label, text) {
                return Ok(cached);
            }
        }
        let wav = langspark_core::VoicevoxTts::default_local(speaker_id).synthesize(text)?;
        if let Some(dir) = &cache_dir {
            if let Err(e) = langspark_core::AudioCache::new(dir.clone()).put(code, &voice_label, text, &wav) {
                log::warn!("failed to cache synthesized audio: {e}");
            }
        }
        Ok(wav)
    }))
}

/// Build the pronunciation tab's playback callbacks. Japanese speaks through
/// a locally-running VOICEVOX Engine (default `http://127.0.0.1:50021` — the
/// user must have it running separately; there's no bundled offline engine).
/// Recording/transcription (speech recognition) are wired the same way
/// regardless of language — see `build_record`/`build_transcribe`.
fn pronunciation_callbacks(active_language: Language, settings: &Settings) -> pronunciation::PronunciationCallbacks {
    let code = active_language.code();
    let Some(synthesize) = build_synthesize(active_language, settings) else {
        return unavailable_pronunciation_callbacks(active_language, settings.audio_input_device.clone());
    };

    let play_cache_dir = crate::config::AppDirs::new().map(|d| d.audio_cache_dir());

    pronunciation::PronunciationCallbacks {
        synthesize,
        record: build_record(settings.audio_input_device.clone()),
        record_duration: RECORDING_DURATION,
        play: Box::new(move |wav: Vec<u8>| {
            let dir = play_cache_dir.clone().unwrap_or_else(std::env::temp_dir);
            langspark_core::AudioManager::new(dir).play(wav)
        }),
        score: Box::new(move |r, e| langspark_core::score_pronunciation(r, e, code)),
        transcribe: build_transcribe(active_language),
    }
}

/// Build the vocabulary detail dialog's Play callback: synthesize + play
/// `text` in the background, surfacing failures as a toast instead of
/// silently doing nothing. `None` (button stays disabled) if no TTS backend
/// is available for `active_language` — see `build_synthesize`.
fn vocab_play_callback(
    active_language: Language,
    settings: &Settings,
    toast_overlay: &ToastOverlay,
) -> Option<Rc<dyn Fn(String)>> {
    let synthesize = build_synthesize(active_language, settings)?;
    let synthesize = Arc::new(synthesize);
    let play_cache_dir = crate::config::AppDirs::new().map(|d| d.audio_cache_dir());

    Some(Rc::new(glib::clone!(
        #[weak]
        toast_overlay,
        move |text: String| {
            let synthesize = synthesize.clone();
            let play_cache_dir = play_cache_dir.clone();
            crate::task::spawn_on_main(async move {
                let result = crate::task::run_blocking(move || -> anyhow::Result<()> {
                    let wav = synthesize(&text)?;
                    let dir = play_cache_dir.clone().unwrap_or_else(std::env::temp_dir);
                    langspark_core::AudioManager::new(dir).play(wav)
                })
                .await;
                if let Err(e) = result {
                    diagnostics::show_error_toast(&toast_overlay, &format!("Couldn't play pronunciation: {e}"));
                }
            });
        }
    )))
}

/// Build the vocabulary detail dialog's Delete callback: deletes the entry
/// (and any SRS cards referencing it — see `SqliteVocabularyRepository::delete`)
/// in the background. The dialog itself closes optimistically before this
/// completes (see `vocabulary/dialog.rs`), so failures are surfaced only via
/// a toast, not by reopening the dialog.
fn vocab_delete_callback(
    state: &Arc<AppState>,
    toast_overlay: &ToastOverlay,
) -> Rc<dyn Fn(i64, Box<dyn Fn()>, Box<dyn Fn()>)> {
    Rc::new(glib::clone!(
        #[strong]
        state,
        #[weak]
        toast_overlay,
        move |id: i64, on_done: Box<dyn Fn()>, on_error: Box<dyn Fn()>| {
            let state = state.clone();
            crate::task::spawn_on_main(async move {
                let result = crate::task::run_blocking(move || state.vocabulary_repo.delete(id)).await;
                match result {
                    Ok(()) => on_done(),
                    Err(e) => {
                        diagnostics::show_error_toast(&toast_overlay, &format!("Failed to delete word: {e}"));
                        on_error();
                    }
                }
            });
        }
    ))
}
