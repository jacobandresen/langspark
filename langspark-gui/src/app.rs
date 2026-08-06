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
use langspark_core::Language;
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

    let vocab_page =
        view_stack.add_titled(&vocabulary::build_tab(&tab_data.vocabulary), Some("vocabulary"), "Vocabulary");
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

    let pronunciation_tab = pronunciation::PronunciationTab::new(Vec::new(), unavailable_pronunciation_callbacks());
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

    let toast_overlay = ToastOverlay::new();
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

fn unavailable_pronunciation_callbacks() -> pronunciation::PronunciationCallbacks {
    pronunciation::PronunciationCallbacks {
        synthesize: Box::new(|_| anyhow::bail!("no TTS backend configured yet — set one up in Preferences")),
        record: Box::new(|| anyhow::bail!("no microphone recording backend configured yet")),
        play: Box::new(|_| anyhow::bail!("no audio output backend configured yet")),
        score: Box::new(|r, e| langspark_core::score_pronunciation(r, e, "ja")),
        transcribe: Box::new(|_, _| anyhow::bail!("speech recognition is unavailable (see the `asr` feature)")),
    }
}
