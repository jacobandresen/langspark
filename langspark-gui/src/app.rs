//! Application module
//!
//! Contains the main window, application state, and coordination between UI and core.

use crate::config::Settings;
use crate::state::AppState;
use crate::{diagnostics, kanji, pronunciation, review, statistics, vocabulary};
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
/// Vocabulary/kanji/statistics tabs are populated from `state` (loaded
/// synchronously at startup — SQLite reads of this size are fast enough not
/// to warrant the complexity of an async loading placeholder). The review
/// tab persists each rating asynchronously via `task::run_blocking` so a
/// slow disk doesn't stall the UI thread. The pronunciation tab's TTS/ASR
/// callbacks report "unavailable" until real backends are configured in
/// Preferences, per the graceful-degradation approach in `tts::UnavailableTts`.
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
                stats: langspark_core::ReviewStats::default(),
                deck_stats: Vec::new(),
            }
        }
    };

    let view_stack = ViewStack::new();
    view_stack.set_vexpand(true);

    let add_word_callbacks = dictionary_add_word_callbacks(&state, active_language, &toast_overlay);
    let vocab_widget = vocabulary::build_tab(&tab_data.vocabulary, add_word_callbacks);
    let vocab_page = view_stack.add_titled(&vocab_widget, Some("vocabulary"), "Vocabulary");
    vocab_page.set_icon_name(Some("accessories-dictionary-symbolic"));

    let kanji_widget = kanji::build_tab(&tab_data.kanji);
    let kanji_page = view_stack.add_titled(&kanji_widget, Some("kanji"), "Kanji");
    kanji_page.set_icon_name(Some("font-x-generic-symbolic"));
    kanji_page.set_visible(kanji::is_visible_for(active_language));

    let review_items = review::build_items_from_cards(&tab_data.due_cards, &tab_data.vocabulary, &tab_data.kanji);
    // Captured alongside the queue so `on_review`'s index can be mapped back
    // to the database row id `SqliteSrsRepository::update_after_review` needs.
    let review_card_ids: Vec<Option<i64>> = review_items.iter().map(|item| item.card.id).collect();
    let review_session = review::ReviewSession::new(
        review_items,
        glib::clone!(
            #[strong]
            state,
            #[weak]
            toast_overlay,
            move |index, rating| {
                let Some(Some(card_id)) = review_card_ids.get(index).copied() else {
                    return;
                };
                let state = state.clone();
                crate::task::spawn_on_main(async move {
                    let result = crate::task::run_blocking(move || state.srs_repo.update_after_review(card_id, rating)).await;
                    if let Err(e) = result {
                        diagnostics::show_error_toast(&toast_overlay, &format!("Failed to save review: {e}"));
                    }
                });
            }
        ),
    );
    let review_page = view_stack.add_titled(&review_session.root, Some("review"), "Review");
    review_page.set_icon_name(Some("view-refresh-symbolic"));

    let practice_words: Vec<pronunciation::PracticeWord> = tab_data
        .vocabulary
        .iter()
        .map(|entry| pronunciation::PracticeWord { text: entry.word.clone(), reading: entry.reading.clone() })
        .collect();
    let pronunciation_tab =
        pronunciation::PronunciationTab::new(practice_words, pronunciation_callbacks(active_language, &settings.borrow()));
    let pronunciation_page =
        view_stack.add_titled(&pronunciation_tab.widget, Some("pronunciation"), "Pronunciation");
    pronunciation_page.set_icon_name(Some("audio-input-microphone-symbolic"));

    let stats_widget = statistics::build_tab(&tab_data.stats, &[], &[], &tab_data.deck_stats);
    let stats_page = view_stack.add_titled(&stats_widget, Some("statistics"), "Statistics");
    stats_page.set_icon_name(Some("x-office-spreadsheet-symbolic"));

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

/// Build the "Add Word" callbacks for the vocabulary tab from the dictionary
/// loaded into `state` (see `AppState::open`), or `None` if no dictionary is
/// installed for `active_language` — the "Add Word" button then stays hidden
/// until one is installed from Preferences > Language Installation.
fn dictionary_add_word_callbacks(
    state: &Arc<AppState>,
    active_language: Language,
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
            let state = state.clone();
            crate::task::spawn_on_main(async move {
                let to_insert = new_entry.clone();
                let result = crate::task::run_blocking(move || state.vocabulary_repo.create(&to_insert)).await;
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
Vocabulary & Kanji — Browse entries grouped by level. Click \"Show All\" to see \
every entry in a section as a grid.

Review — Cards due today appear one at a time. Click \"Show Answer\" to reveal \
the back, then rate how well you remembered it (Again/Hard/Good/Easy). Your \
rating adjusts when the card comes up next.

Pronunciation — Pick a word, click Play to hear a reference pronunciation, \
then Record to attempt it yourself. You'll get a score and feedback. \
Requires a TTS backend configured in Preferences.

Statistics — Your review history, streak, retention rate, and per-deck \
progress.

Preferences — Change the active language (takes effect on restart), TTS \
voices, SRS algorithm, theme, and audio devices.";

fn unavailable_pronunciation_callbacks(active_language: Language) -> pronunciation::PronunciationCallbacks {
    let code = active_language.code();
    pronunciation::PronunciationCallbacks {
        synthesize: Box::new(|_| anyhow::bail!("no TTS backend configured yet — set one up in Preferences")),
        record: Box::new(|| anyhow::bail!("no microphone recording backend configured yet")),
        play: Box::new(|_| anyhow::bail!("no audio output backend configured yet")),
        score: Box::new(move |r, e| langspark_core::score_pronunciation(r, e, code)),
        transcribe: Box::new(|_, _| anyhow::bail!("speech recognition is unavailable (see the `asr` feature)")),
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

/// Build the pronunciation tab's playback callbacks. Japanese speaks through
/// a locally-running VOICEVOX Engine (default `http://127.0.0.1:50021` — the
/// user must have it running separately; there's no bundled offline engine),
/// caching synthesized audio to disk so repeat playback of the same word
/// doesn't re-synthesize it. Spanish stays "unavailable": Piper needs a
/// downloaded `.onnx` voice model and there's no installer for one yet (see
/// ROADMAP.md Phase 8), unlike the Japanese dictionary installer in
/// `installer.rs`. Recording/transcription (speech recognition) are a
/// separate, still-unimplemented feature (the `asr` module).
fn pronunciation_callbacks(active_language: Language, settings: &Settings) -> pronunciation::PronunciationCallbacks {
    if active_language != Language::Japanese {
        return unavailable_pronunciation_callbacks(active_language);
    }

    let code = active_language.code();
    let cache_dir = crate::config::AppDirs::new().map(|d| d.audio_cache_dir());
    let speaker_id = resolve_voicevox_speaker_id(&settings.tts_voice_ja);
    let voice_label = settings.tts_voice_ja.clone();

    let synth_cache_dir = cache_dir.clone();
    let synth_voice_label = voice_label.clone();
    let play_cache_dir = cache_dir;

    pronunciation::PronunciationCallbacks {
        synthesize: Box::new(move |text: &str| {
            if let Some(dir) = &synth_cache_dir {
                if let Some(cached) = langspark_core::AudioCache::new(dir.clone()).get(code, &synth_voice_label, text) {
                    return Ok(cached);
                }
            }
            let wav = langspark_core::VoicevoxTts::default_local(speaker_id).synthesize(text)?;
            if let Some(dir) = &synth_cache_dir {
                if let Err(e) = langspark_core::AudioCache::new(dir.clone()).put(code, &synth_voice_label, text, &wav) {
                    log::warn!("failed to cache synthesized audio: {e}");
                }
            }
            Ok(wav)
        }),
        record: Box::new(|| anyhow::bail!("no microphone recording backend configured yet")),
        play: Box::new(move |wav: Vec<u8>| {
            let dir = play_cache_dir.clone().unwrap_or_else(std::env::temp_dir);
            langspark_core::AudioManager::new(dir).play(wav)
        }),
        score: Box::new(move |r, e| langspark_core::score_pronunciation(r, e, code)),
        transcribe: Box::new(|_, _| anyhow::bail!("speech recognition is unavailable (see the `asr` feature)")),
    }
}
