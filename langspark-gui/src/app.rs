//! Application module
//!
//! Contains the main window, application state, and coordination between UI and core.

use crate::config::Settings;
use crate::state::AppState;
use crate::{books, diagnostics, pronunciation, review, vocabulary};
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
use std::sync::{Arc, Mutex};

/// A minimal splash window shown the instant the app activates, before
/// `AppState::open` (database + dictionary loading — the slow part of
/// startup, easily a few seconds for a full JMdict install) has even
/// started. Without this, GTK's main loop stays blocked on that call and
/// nothing appears on screen at all, which reads as the app having failed to
/// launch. Callers spawn the heavy work in the background (see
/// `task::run_blocking` in `main.rs`) and swap this out for
/// `build_main_window`'s real window once it resolves.
pub fn build_loading_window(app: &AdwApplication) -> AdwApplicationWindow {
    let window = AdwApplicationWindow::builder()
        .application(app)
        .title("LangSpark")
        .default_width(360)
        .default_height(240)
        .resizable(false)
        .build();

    let logo = gtk4::Image::from_icon_name("org.langspark.LangSpark");
    logo.set_pixel_size(96);

    let spinner = gtk4::Spinner::builder().width_request(32).height_request(32).build();
    spinner.start();

    let label = gtk4::Label::builder().label("Loading LangSpark\u{2026}").css_classes(["title-3"]).build();

    let content = GtkBox::new(gtk4::Orientation::Vertical, 12);
    content.set_valign(gtk4::Align::Center);
    content.set_halign(gtk4::Align::Center);
    content.set_vexpand(true);
    content.append(&logo);
    content.append(&spinner);
    content.append(&label);

    let toolbar_view = ToolbarView::builder().content(&content).build();
    toolbar_view.add_top_bar(&HeaderBar::builder().show_title(false).build());

    window.set_content(Some(&toolbar_view));
    window
}

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

    // Shared with every "Play" button built below (and updated live from
    // Preferences — see `register_app_actions`) so a voice/speed change
    // takes effect on the next Play without restarting the app; see
    // `VoiceSettings`.
    let voice_settings = Arc::new(Mutex::new(VoiceSettings {
        voice: settings.borrow().tts_voice_ja.clone(),
        speed: settings.borrow().tts_speed,
    }));

    // `review_session` is *built* before the Vocabulary tab (which it would
    // otherwise seem to follow, view-switcher-order-wise) because
    // `dictionary_add_word_callbacks` needs `review_session.append` — a
    // newly-added word's immediately-due `SrsCard` (see
    // `build_persist_callback`) should show up in the Review queue live, the
    // same way `vocab_tab.append` makes it show up in the Vocabulary tab
    // live. Its `add_titled` call is deferred below, after Vocabulary's, so
    // this dependency doesn't also reorder the view switcher itself.
    let review_items = review::build_items_from_cards(&tab_data.due_cards, &tab_data.vocabulary, &tab_data.kanji);
    let review_play_callback = vocab_play_callback(active_language, voice_settings.clone(), &toast_overlay);
    let review_session = review::ReviewSession::new(
        review_items,
        glib::clone!(
            #[strong]
            state,
            #[strong]
            settings,
            #[weak]
            toast_overlay,
            move |card_id, rating| {
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

    let vocab_tab = vocabulary::build_tab(
        &tab_data.vocabulary,
        vocabulary::VocabTabCallbacks {
            add_word: dictionary_add_word_callbacks(
                &state,
                active_language,
                &settings,
                &toast_overlay,
                &review_session.append,
            ),
            on_play: vocab_play_callback(active_language, voice_settings.clone(), &toast_overlay),
            delete: vocab_delete_callback(&state, &toast_overlay),
            example_lookup: example_lookup_callback(&state, active_language),
        },
    );
    let vocab_page = view_stack.add_titled(&vocab_tab.widget, Some("vocabulary"), "Vocabulary");
    vocab_page.set_icon_name(Some("accessories-dictionary-symbolic"));

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
            pronunciation_callbacks(&state, active_language, &settings.borrow(), voice_settings.clone()),
        );
        let pronunciation_page =
            view_stack.add_titled(&pronunciation_tab.widget, Some("pronunciation"), "Pronunciation");
        pronunciation_page.set_icon_name(Some("audio-input-microphone-symbolic"));
    }

    if let Some(callbacks) = books_tab_callbacks(
        &state,
        active_language,
        &settings,
        voice_settings.clone(),
        &toast_overlay,
        &vocab_tab.append,
        &review_session.append,
    ) {
        let catalog = load_installed_book_catalog(&settings.borrow());
        if !catalog.is_empty() {
            let books_widget = books::build_tab(&catalog, callbacks);
            let books_page = view_stack.add_titled(&books_widget, Some("books"), "Books");
            books_page.set_icon_name(Some("x-office-document-symbolic"));
        }
    }

    // Header: view switcher as the title, app menu
    let switcher_title = ViewSwitcherTitle::builder().stack(&view_stack).title("LangSpark").build();

    let menu = gio::Menu::new();
    menu.append(Some("Preferences"), Some("app.preferences"));
    menu.append(Some("Help"), Some("app.help"));
    menu.append(Some("About LangSpark"), Some("app.about"));
    menu.append(Some("Quit"), Some("app.quit"));
    let menu_button = gtk4::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).build();

    let header = HeaderBar::builder().title_widget(&switcher_title).build();
    header.pack_end(&menu_button);

    let content = GtkBox::new(gtk4::Orientation::Vertical, 0);
    content.set_vexpand(true);
    content.append(&view_stack);

    let toolbar_view = ToolbarView::builder().content(&content).build();
    toolbar_view.add_top_bar(&header);

    toast_overlay.set_child(Some(&toolbar_view));
    window.set_content(Some(&toast_overlay));

    register_app_actions(app, &window, settings, voice_settings);

    (window, toast_overlay)
}

/// Register app-level actions (`app.preferences`, `app.about`, `app.quit`)
/// backing the header menu.
fn register_app_actions(
    app: &AdwApplication,
    window: &AdwApplicationWindow,
    settings: Rc<RefCell<Settings>>,
    voice_settings: Arc<Mutex<VoiceSettings>>,
) {
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
        #[strong]
        voice_settings,
        move |_, _| {
            let voice_settings = voice_settings.clone();
            let dialog = crate::preferences::build(settings.clone(), move |updated| {
                // Keep the background-thread-visible TTS mirror in step with
                // every settings change — see `VoiceSettings`'s doc comment
                // for why `Settings` itself (`Rc<RefCell<_>>`) can't be read
                // directly from where synthesis actually runs.
                *voice_settings.lock().expect("voice settings mutex poisoned") =
                    VoiceSettings { voice: updated.tts_voice_ja.clone(), speed: updated.tts_speed };
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
    review_append: &Rc<dyn Fn(review::ReviewItem)>,
) -> Option<vocabulary::AddWordCallbacks> {
    let code = active_language.code();
    if !state.dictionary.is_loaded(code) {
        return None;
    }

    let search: Rc<dyn Fn(&str) -> Vec<langspark_core::VocabEntry>> = {
        let state = state.clone();
        Rc::new(move |query: &str| state.dictionary.search(code, query).into_iter().cloned().collect())
    };

    Some(vocabulary::AddWordCallbacks {
        search,
        persist: build_persist_callback(state, settings, toast_overlay, review_append),
    })
}

/// Build the "persist a dictionary entry into the user's vocabulary" write
/// path: shared by `dictionary_add_word_callbacks` (the Vocabulary tab's
/// "Add Word" dialog) and `books::popup`'s "Add to Vocabulary" button, both
/// of which start from a `langspark_core::VocabEntry` (a dictionary lookup
/// result, not yet saved) and need the exact same
/// `vocabulary_repo.create` + immediately-due `SrsCard` behavior — a word
/// added while reading should show up in Review exactly like one added from
/// the dictionary search dialog. That includes *live*, not just after a
/// database reload: `review_append` (see `review::ReviewSession::append`)
/// is called with the freshly-created card so it's reviewable immediately —
/// the Vocabulary tab's own equivalent immediacy (`vocabulary::VocabTab::append`)
/// is layered on separately by each caller, since (unlike Review) only one
/// of the two callers here needs it wired in (see `persist_and_refresh_vocab_tab`).
fn build_persist_callback(
    state: &Arc<AppState>,
    settings: &Rc<RefCell<Settings>>,
    toast_overlay: &ToastOverlay,
    review_append: &Rc<dyn Fn(review::ReviewItem)>,
) -> Rc<dyn Fn(langspark_core::VocabEntry, Box<dyn Fn(langspark_core::VocabularyEntry)>, Box<dyn Fn()>)> {
    Rc::new(glib::clone!(
        #[strong]
        state,
        #[strong]
        settings,
        #[weak]
        toast_overlay,
        #[strong]
        review_append,
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
            let review_append = review_append.clone();
            crate::task::spawn_on_main(async move {
                let to_insert = new_entry.clone();
                let result = crate::task::run_blocking(move || -> anyhow::Result<(i64, langspark_core::SrsCard)> {
                    let vocab_id = state.vocabulary_repo.create(&to_insert)?;
                    // Newly-added words are immediately due for review: also
                    // create the SRS card that puts them in the Review tab's
                    // queue (see `SrsCard::is_due_today`, true when unreviewed).
                    let mut card = langspark_core::SrsCard::new("vocabulary", &to_insert.language);
                    card.vocab_id = Some(vocab_id);
                    card.ease_factor = starting_ease_factor;
                    card.id = Some(state.srs_repo.create(&card)?);
                    Ok((vocab_id, card))
                })
                .await;
                match result {
                    Ok((id, card)) => {
                        let mut persisted = new_entry;
                        persisted.id = Some(id);
                        // Reuses `build_items_from_cards`' own front/back/speak_text
                        // construction (a 1-card, 1-vocab-entry slice) rather than
                        // duplicating it here, so the two can't drift apart.
                        if let Some(item) =
                            review::build_items_from_cards(std::slice::from_ref(&card), std::slice::from_ref(&persisted), &[])
                                .into_iter()
                                .next()
                        {
                            review_append(item);
                        }
                        on_done(persisted);
                    }
                    Err(e) => {
                        diagnostics::show_error_toast(&toast_overlay, &format!("Failed to add word: {e}"));
                        on_error();
                    }
                }
            });
        }
    ))
}

/// Wrap a `persist` callback (see `build_persist_callback`) so that, on top
/// of whatever the caller's own `on_done` does (e.g. a popup's button
/// switching to "Added"), the newly-saved entry is also pushed into the
/// Vocabulary tab's live list via `vocabulary::VocabTab::append`. Without
/// this, a word added from outside the Vocabulary tab (currently: the Books
/// reader's popup) is correctly written to the database but doesn't show up
/// in the Vocabulary tab until the app is restarted and reloads it from
/// disk — the tab's list is otherwise only ever mutated by its own "Add
/// Word" dialog (see `vocabulary::build_tab`'s doc comment).
fn persist_and_refresh_vocab_tab(
    persist: Rc<dyn Fn(langspark_core::VocabEntry, Box<dyn Fn(langspark_core::VocabularyEntry)>, Box<dyn Fn()>)>,
    vocab_append: Rc<dyn Fn(langspark_core::VocabularyEntry)>,
) -> Rc<dyn Fn(langspark_core::VocabEntry, Box<dyn Fn(langspark_core::VocabularyEntry)>, Box<dyn Fn()>)> {
    Rc::new(move |dict_entry, on_done: Box<dyn Fn(langspark_core::VocabularyEntry)>, on_error: Box<dyn Fn()>| {
        let vocab_append = vocab_append.clone();
        persist(
            dict_entry,
            Box::new(move |saved: langspark_core::VocabularyEntry| {
                vocab_append(saved.clone());
                on_done(saved);
            }),
            on_error,
        );
    })
}

/// Resolve where the book catalog and cached book text live: the
/// `books_data_dir` Preference override if set, otherwise the default XDG
/// books directory — the same override pattern `dictionary_data_dir` uses
/// (see `preferences.rs`).
fn effective_books_dir(settings: &Settings) -> Option<std::path::PathBuf> {
    settings.books_data_dir.clone().or_else(|| crate::config::AppDirs::new().map(|d| d.books_dir()))
}

/// Load the installed Aozora Bunko catalog from `<books_dir>/catalog.json`
/// (see `langspark_core::install_aozora_catalog`), or an empty list if none
/// is installed yet — in which case the Books tab simply isn't shown, the
/// same graceful-degradation approach `asr_model_installed` uses for the
/// Pronunciation tab.
fn load_installed_book_catalog(settings: &Settings) -> Vec<langspark_core::BookCatalogEntry> {
    let Some(dir) = effective_books_dir(settings) else { return Vec::new() };
    let Ok(json) = std::fs::read_to_string(dir.join("catalog.json")) else { return Vec::new() };
    langspark_core::load_book_catalog(&json).unwrap_or_default()
}

/// Build the Books tab's callbacks, or `None` if the active language has no
/// dictionary loaded — word lookups while reading need one just as much as
/// the Vocabulary tab's "Add Word" dialog does (see
/// `dictionary_add_word_callbacks`).
fn books_tab_callbacks(
    state: &Arc<AppState>,
    active_language: Language,
    settings: &Rc<RefCell<Settings>>,
    voice_settings: Arc<Mutex<VoiceSettings>>,
    toast_overlay: &ToastOverlay,
    vocab_append: &Rc<dyn Fn(langspark_core::VocabularyEntry)>,
    review_append: &Rc<dyn Fn(review::ReviewItem)>,
) -> Option<books::BooksTabCallbacks> {
    let code = active_language.code();
    if !state.dictionary.is_loaded(code) {
        return None;
    }
    let books_dir = effective_books_dir(&settings.borrow())?;

    let open_book: Rc<
        dyn Fn(langspark_core::BookCatalogEntry, Box<dyn Fn(langspark_core::BookText)>, Box<dyn Fn(String)>),
    > = Rc::new(glib::clone!(
        #[weak]
        toast_overlay,
        move |entry: langspark_core::BookCatalogEntry,
              on_done: Box<dyn Fn(langspark_core::BookText)>,
              on_error: Box<dyn Fn(String)>| {
            let books_dir = books_dir.clone();
            let title = entry.title.clone();
            crate::task::spawn_on_main(async move {
                let result =
                    crate::task::run_blocking(move || langspark_core::fetch_book(&entry, &books_dir, &|_, _| {})).await;
                match result {
                    Ok(book) => on_done(book),
                    Err(e) => {
                        let message = format!("Couldn't open '{title}': {e}");
                        diagnostics::show_error_toast(&toast_overlay, &message);
                        on_error(message);
                    }
                }
            });
        }
    ));

    let lookup: Rc<dyn Fn(&str, usize) -> Option<(usize, usize, langspark_core::VocabEntry)>> = {
        let state = state.clone();
        Rc::new(move |text: &str, char_index: usize| {
            let (start, end, entry) = state.dictionary.word_at(code, text, char_index)?;
            Some((start, end, entry.clone()))
        })
    };

    let reader = books::reader::ReaderCallbacks {
        lookup,
        speak: vocab_play_callback(active_language, voice_settings, toast_overlay),
        add_to_vocabulary: persist_and_refresh_vocab_tab(
            build_persist_callback(state, settings, toast_overlay, review_append),
            vocab_append.clone(),
        ),
        translate_paragraph: translate_paragraph_callback(state, toast_overlay),
    };

    Some(books::BooksTabCallbacks { open_book, reader })
}

/// Build the `translate_paragraph` callback (see
/// `books::reader::ReaderCallbacks`). Always returns a callable closure,
/// unlike `speak`/`add_to_vocabulary`'s `Option`-gating — when no
/// translation model is installed, it still exists, it just always reports
/// that through `on_error`, so the paragraph popup stays available and
/// explains what to do rather than the icon silently disappearing.
fn translate_paragraph_callback(
    state: &Arc<AppState>,
    toast_overlay: &ToastOverlay,
) -> Rc<dyn Fn(String, Box<dyn Fn(String)>, Box<dyn Fn(String)>)> {
    let model_dir = crate::config::AppDirs::new().map(|d| d.translation_model_dir());
    let cache_dir = crate::config::AppDirs::new().map(|d| d.translation_cache_dir());
    let state = state.clone();

    Rc::new(glib::clone!(
        #[weak]
        toast_overlay,
        move |japanese: String, on_done: Box<dyn Fn(String)>, on_error: Box<dyn Fn(String)>| {
            let Some(model_dir) = model_dir.clone().filter(|d| d.join("model.safetensors").exists()) else {
                on_error("Translation model not installed \u{2014} install it in Preferences.".to_string());
                return;
            };
            if let Some(cache) = &cache_dir {
                if let Some(cached) = langspark_core::TranslationCache::new(cache.clone()).get(&japanese) {
                    on_done(cached);
                    return;
                }
            }

            let state = state.clone();
            let cache_dir = cache_dir.clone();
            let japanese_for_cache = japanese.clone();
            crate::task::spawn_on_main(async move {
                let result = crate::task::run_blocking(move || -> anyhow::Result<String> {
                    let translator = state.translator.get_or_init(|| match langspark_core::Translator::load(&model_dir) {
                        Ok(t) => Some(Mutex::new(t)),
                        Err(e) => {
                            log::warn!("failed to load translation model: {e}");
                            None
                        }
                    });
                    let Some(translator) = translator else {
                        anyhow::bail!("translation model failed to load (see logs)");
                    };
                    let translator = translator.lock().expect("translator mutex poisoned");
                    translator.translate(&japanese)
                })
                .await;
                match result {
                    Ok(english) => {
                        if let Some(dir) = &cache_dir {
                            if let Err(e) = langspark_core::TranslationCache::new(dir.clone()).put(&japanese_for_cache, &english) {
                                log::warn!("failed to cache translation: {e}");
                            }
                        }
                        on_done(english);
                    }
                    Err(e) => {
                        let message = format!("Couldn't translate paragraph: {e}");
                        diagnostics::show_error_toast(&toast_overlay, &message);
                        on_error(message);
                    }
                }
            });
        }
    ))
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

Books — Read Aozora Bunko classics grouped by genre. Click any word in the \
text for its reading and meaning, hear it pronounced, and add it straight to \
your vocabulary deck. Only appears once a dictionary and the book catalog \
are installed (see Preferences).

Preferences — Change the active language (takes effect on restart), TTS \
voices, SRS algorithm, and audio devices.";

fn unavailable_pronunciation_callbacks(
    state: &Arc<AppState>,
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
        transcribe: build_transcribe(state, active_language),
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
fn build_transcribe(state: &Arc<AppState>, active_language: Language) -> Box<dyn Fn(&[f32], u32) -> anyhow::Result<String> + Send + Sync> {
    let code = active_language.code().to_string();
    let model_dir = crate::config::AppDirs::new().map(|d| d.asr_model_dir(&code));
    let state = state.clone();

    Box::new(move |samples: &[f32], sample_rate: u32| {
        let dir = model_dir.clone().ok_or_else(|| anyhow::anyhow!("couldn't determine the ASR model directory"))?;
        if !dir.exists() {
            anyhow::bail!(
                "no speech recognition model installed at {} (needs config.json, model.safetensors, \
                 tokenizer.json from a qwen3 ASR model)",
                dir.display()
            );
        }

        // Loaded once per session and kept resident (see `AppState::recognizer`)
        // rather than reloading the model's weights from disk on every call —
        // reloading here previously dominated pronunciation-practice latency
        // far more than the actual transcription inference did.
        let recognizer = state.recognizer.get_or_init(|| match langspark_core::SpeechRecognizer::new(&code, &dir) {
            Ok(r) => Some(Mutex::new(r)),
            Err(e) => {
                log::warn!("failed to load ASR model: {e}");
                None
            }
        });
        let Some(recognizer) = recognizer else {
            anyhow::bail!("speech recognition model failed to load (see logs)");
        };
        let recognizer = recognizer.lock().expect("recognizer mutex poisoned");

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
    let run_path = dirs.voicevox_engine_dir().join(langspark_core::voicevox_run_executable_name());
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

/// VOICEVOX speakers this app has a stable, well-known speaker ID for out of
/// the box — `(settings key, display name, speaker id)`. These IDs are
/// stable across VOICEVOX Engine installs (a handful of "default" characters
/// bundled with every install), unlike the engine's fuller speaker roster,
/// which would need querying its own `/speakers` endpoint to enumerate — not
/// attempted here since the engine may not even be running yet when
/// Preferences is opened. Shared between `resolve_voicevox_speaker_id` below
/// and the Japanese voice `ComboRow` in `preferences.rs`, so the select list
/// and the resolver can't drift apart.
pub(crate) const VOICEVOX_SPEAKERS: &[(&str, &str, u32)] = &[
    ("zundamon", "Zundamon (ずんだもん)", 3),
    ("metan", "Shikoku Metan (四国めたん)", 2),
    ("tsumugi", "Kasukabe Tsumugi (春日部つむぎ)", 8),
];

/// Resolve a VOICEVOX speaker ID from the `tts_voice_ja` setting: one of
/// `VOICEVOX_SPEAKERS`' keys (the only values the Preferences `ComboRow` ever
/// writes), or a bare number for full control over any other installed
/// speaker (not selectable from the UI, but still honored for anyone editing
/// config.toml by hand) — falling back to Zundamon Normal, the project's
/// documented default voice (see `language.rs`'s `default_tts_voice`).
fn resolve_voicevox_speaker_id(voice: &str) -> u32 {
    const ZUNDAMON_NORMAL: u32 = 3;
    let voice = voice.trim();
    if let Ok(id) = voice.parse::<u32>() {
        return id;
    }
    let voice = voice.to_lowercase();
    VOICEVOX_SPEAKERS.iter().find(|(key, _, _)| *key == voice).map(|&(_, _, id)| id).unwrap_or(ZUNDAMON_NORMAL)
}

/// Map the user-facing "speech speed" preference (1 = slowest, 5 = normal —
/// see `Settings::tts_speed`) onto VOICEVOX's own `speedScale` parameter
/// (1.0 = the engine's native speed). Linear from 0.5 at 1 up to 1.0 at 5, so
/// the existing default (5) reproduces the speed LangSpark always spoke at
/// before this setting existed.
fn tts_speed_to_speed_scale(speed: u8) -> f64 {
    0.5 + (speed.clamp(1, 5) as f64 - 1.0) * 0.125
}

/// Live mirror of the two `Settings` fields controlling Japanese TTS
/// (`tts_voice_ja`, `tts_speed`) that every "Play" button's synthesis needs
/// — kept in sync with `Settings` whenever Preferences saves (see
/// `register_app_actions`'s `on_save`), but held behind a `Mutex` rather than
/// `Settings`' own `Rc<RefCell<_>>`, which isn't `Send`: synthesis always
/// runs on a background thread via `task::run_blocking` (see `task.rs`), so
/// a closure built from a value read out of `Rc<RefCell<Settings>>` once at
/// *startup* — the previous approach — went stale the moment the voice was
/// changed in Preferences, only picking up the new one after restarting the
/// app.
#[derive(Clone)]
struct VoiceSettings {
    voice: String,
    speed: u8,
}

/// Build a `synthesize(text) -> WAV bytes` closure for `active_language`
/// (currently always Japanese, spoken through VOICEVOX), shared between the
/// pronunciation tab's Play button and the vocabulary detail dialog's Play
/// button (`vocab_play_callback`). Synthesized audio is cached to disk so
/// repeat playback of the same word doesn't re-synthesize it — the cache key
/// folds in the speed setting so changing it doesn't serve stale audio at
/// the old speed. Reads `voice_settings` fresh on *every* call, rather than
/// once when this closure is built, so a voice/speed change in Preferences
/// takes effect the very next time Play is pressed — see `VoiceSettings`.
fn build_synthesize(
    active_language: Language,
    voice_settings: Arc<Mutex<VoiceSettings>>,
) -> Option<Box<dyn Fn(&str) -> anyhow::Result<Vec<u8>> + Send + Sync>> {
    let code = active_language.code();
    let cache_dir = crate::config::AppDirs::new().map(|d| d.audio_cache_dir());

    Some(Box::new(move |text: &str| {
        let (speaker_id, speed_scale, voice_label) = {
            let vs = voice_settings.lock().expect("voice settings mutex poisoned");
            (
                resolve_voicevox_speaker_id(&vs.voice),
                tts_speed_to_speed_scale(vs.speed),
                format!("{}_speed{}", vs.voice, vs.speed),
            )
        };
        if let Some(dir) = &cache_dir {
            if let Some(cached) = langspark_core::AudioCache::new(dir.clone()).get(code, &voice_label, text) {
                return Ok(cached);
            }
        }
        let wav = langspark_core::VoicevoxTts::default_local(speaker_id, speed_scale).synthesize(text)?;
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
fn pronunciation_callbacks(
    state: &Arc<AppState>,
    active_language: Language,
    settings: &Settings,
    voice_settings: Arc<Mutex<VoiceSettings>>,
) -> pronunciation::PronunciationCallbacks {
    let code = active_language.code();
    let Some(synthesize) = build_synthesize(active_language, voice_settings) else {
        return unavailable_pronunciation_callbacks(state, active_language, settings.audio_input_device.clone());
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
        transcribe: build_transcribe(state, active_language),
    }
}

/// Build the vocabulary detail dialog's Play callback: synthesize + play
/// `text` in the background, surfacing failures as a toast instead of
/// silently doing nothing. `None` (button stays disabled) if no TTS backend
/// is available for `active_language` — see `build_synthesize`.
fn vocab_play_callback(
    active_language: Language,
    voice_settings: Arc<Mutex<VoiceSettings>>,
    toast_overlay: &ToastOverlay,
) -> Option<Rc<dyn Fn(String)>> {
    let synthesize = build_synthesize(active_language, voice_settings)?;
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
