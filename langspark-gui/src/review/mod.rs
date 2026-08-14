//! Review tab: spaced-repetition review queue. Shows one card at a time
//! (front, then flip to reveal the back), rating buttons that report each
//! rating via `on_review` — the caller (`app.rs::build_main_window`'s
//! `ReviewSession::new` call) persists it through the user's chosen SM-2/
//! FSRS backend — a progress indicator, and keyboard shortcuts for rating.

use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation};
use langspark_core::{KanjiEntry, SrsCard, VocabularyEntry, RATING_AGAIN, RATING_EASY, RATING_GOOD, RATING_HARD};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What to show as a card's front/back text — supplied by the caller since
/// the card content (word/reading/meaning) lives in `VocabularyEntry`/`KanjiEntry`,
/// not on `SrsCard` itself.
#[derive(Debug, Clone)]
pub struct ReviewCardContent {
    pub front: String,
    pub back: String,
    /// What to pass to the Play button's TTS callback — the reading for
    /// vocabulary (so kanji with irregular readings are spoken correctly
    /// rather than guessed from the characters) or a kanji's own reading,
    /// falling back to `front` when no reading is available.
    pub speak_text: String,
}

/// One entry in a review session: the SRS scheduling card plus its display content.
#[derive(Debug, Clone)]
pub struct ReviewItem {
    pub card: SrsCard,
    pub content: ReviewCardContent,
}

/// Runs a review session over a queue of cards, calling `on_review` with
/// (the rated card's database id, rating) each time the user rates a card so
/// the caller can persist the SM-2 update (e.g. via
/// `SqliteSrsRepository::update_after_review`) — the id comes from the
/// queue's own `ReviewItem::card.id` at rating time rather than a
/// caller-maintained id-per-position list, so it stays correct even as
/// `append` grows the queue past whatever length it started at.
pub struct ReviewSession {
    pub root: gtk4::Widget,
    /// Push a newly-created card onto the live queue and refresh the visible
    /// card — including flipping a session that had already reached "done"
    /// back to showing it. Wired from `app.rs::build_persist_callback` so a
    /// word just added to the vocabulary (which always creates an
    /// immediately-due `SrsCard`, see that function's doc comment) becomes
    /// reviewable without restarting the app, the same way `vocabulary::VocabTab::append`
    /// makes it show up in the Vocabulary tab immediately.
    pub append: Rc<dyn Fn(ReviewItem)>,
}

impl ReviewSession {
    /// `on_play`, if given, speaks a card's `speak_text` through TTS when the
    /// Play button is clicked — `None` (matching `vocabulary::dialog`'s
    /// pattern) omits the button entirely rather than showing a disabled one,
    /// since no language currently reviewed lacks a TTS backend for long.
    pub fn new(queue: Vec<ReviewItem>, on_review: impl Fn(i64, u32) + 'static, on_play: Option<Rc<dyn Fn(String)>>) -> Self {
        let root = Box::new(Orientation::Vertical, 8);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(12);
        root.set_margin_end(12);

        let progress_bar = gtk4::ProgressBar::builder().show_text(false).build();
        let progress_label = Label::builder().css_classes(["caption", "dim-label"]).build();
        root.append(&progress_bar);
        root.append(&progress_label);

        // Vertically centers everything below the progress bar in whatever
        // space is left — without this, a tall window leaves the card
        // pinned at the top with a large dead gap underneath it.
        let center_wrapper = Box::new(Orientation::Vertical, 20);
        center_wrapper.set_valign(gtk4::Align::Center);
        center_wrapper.set_vexpand(true);
        root.append(&center_wrapper);

        let card_text = Label::builder()
            .css_classes(["title-1"])
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .build();

        let play_btn = on_play.is_some().then(|| {
            gtk4::Button::builder()
                .icon_name("media-playback-start-symbolic")
                .halign(gtk4::Align::Center)
                .tooltip_text("Play the word")
                .margin_top(12)
                .build()
        });

        let card_frame = Box::new(Orientation::Vertical, 4);
        card_frame.set_halign(gtk4::Align::Center);
        card_frame.set_css_classes(&["langspark-review-card"]);
        card_frame.append(&card_text);
        if let Some(btn) = &play_btn {
            card_frame.append(btn);
        }
        center_wrapper.append(&card_frame);

        let show_answer = gtk4::Button::builder().label("Show Answer").halign(gtk4::Align::Center).build();
        center_wrapper.append(&show_answer);

        let rating_box = Box::new(Orientation::Horizontal, 8);
        rating_box.set_halign(gtk4::Align::Center);
        rating_box.set_visible(false);
        let again_btn = gtk4::Button::builder().label("Again").css_classes(["destructive-action"]).build();
        let hard_btn = gtk4::Button::builder().label("Hard").build();
        let good_btn = gtk4::Button::builder().label("Good").css_classes(["suggested-action"]).build();
        let easy_btn = gtk4::Button::builder().label("Easy").build();
        for btn in [&again_btn, &hard_btn, &good_btn, &easy_btn] {
            rating_box.append(btn);
        }
        center_wrapper.append(&rating_box);

        let shortcut_hint = Label::builder()
            .label("Space to reveal \u{00b7} 1\u{2013}4 to rate")
            .css_classes(["caption", "dim-label"])
            .halign(gtk4::Align::Center)
            .build();
        center_wrapper.append(&shortcut_hint);

        let queue = Rc::new(RefCell::new(queue));
        let index = Rc::new(Cell::new(0usize));
        let showing_answer = Rc::new(Cell::new(false));
        let on_review = Rc::new(on_review);

        let refresh = {
            let queue = queue.clone();
            let index = index.clone();
            let showing_answer = showing_answer.clone();
            let progress_bar = progress_bar.clone();
            let progress_label = progress_label.clone();
            let card_text = card_text.clone();
            let show_answer_btn = show_answer.clone();
            let rating_box = rating_box.clone();
            let play_btn = play_btn.clone();
            let shortcut_hint = shortcut_hint.clone();
            move || {
                let q = queue.borrow();
                let total = q.len();
                let i = index.get();
                showing_answer.set(false);
                rating_box.set_visible(false);
                show_answer_btn.set_visible(true);
                progress_bar.set_fraction(if total == 0 { 0.0 } else { i.min(total) as f64 / total as f64 });

                if i >= total {
                    progress_label.set_label(&format!("{total} of {total} \u{2014} done!"));
                    card_text.set_label("Review complete");
                    show_answer_btn.set_visible(false);
                    shortcut_hint.set_visible(false);
                    if let Some(btn) = &play_btn {
                        btn.set_visible(false);
                    }
                } else {
                    progress_label.set_label(&format!("{} of {total}", i + 1));
                    card_text.set_label(&q[i].content.front);
                    shortcut_hint.set_visible(true);
                    if let Some(btn) = &play_btn {
                        btn.set_visible(true);
                    }
                }
            }
        };
        refresh();

        let append: Rc<dyn Fn(ReviewItem)> = Rc::new(glib::clone!(
            #[strong]
            queue,
            #[strong]
            refresh,
            move |item: ReviewItem| {
                queue.borrow_mut().push(item);
                // Re-evaluates the "done" check against the now-larger
                // queue, so a session already on the "done" screen flips
                // back to showing the newly-added card instead of staying
                // stuck there until the app restarts.
                refresh();
            }
        ));

        show_answer.connect_clicked(glib::clone!(
            #[weak]
            card_text,
            #[weak]
            rating_box,
            #[strong]
            queue,
            #[strong]
            index,
            #[strong]
            showing_answer,
            move |btn| {
                let q = queue.borrow();
                let i = index.get();
                if let Some(item) = q.get(i) {
                    card_text.set_label(&item.content.back);
                    rating_box.set_visible(true);
                    btn.set_visible(false);
                    showing_answer.set(true);
                }
            }
        ));

        if let (Some(play_btn), Some(on_play)) = (&play_btn, on_play) {
            play_btn.connect_clicked(glib::clone!(
                #[strong]
                queue,
                #[strong]
                index,
                move |_| {
                    let q = queue.borrow();
                    if let Some(item) = q.get(index.get()) {
                        on_play(item.content.speak_text.clone());
                    }
                }
            ));
        }

        for (btn, rating) in [
            (&again_btn, RATING_AGAIN),
            (&hard_btn, RATING_HARD),
            (&good_btn, RATING_GOOD),
            (&easy_btn, RATING_EASY),
        ] {
            btn.connect_clicked(glib::clone!(
                #[strong]
                queue,
                #[strong]
                index,
                #[strong]
                on_review,
                #[strong]
                refresh,
                move |_| {
                    let i = index.get();
                    if let Some(card_id) = queue.borrow().get(i).and_then(|item| item.card.id) {
                        on_review(card_id, rating);
                    }
                    index.set(i + 1);
                    refresh();
                }
            ));
        }

        // Keyboard shortcuts: Space reveals the answer, 1-4 rate it (Again/Hard/Good/Easy).
        let key_controller = gtk4::EventControllerKey::new();
        key_controller.connect_key_pressed(glib::clone!(
            #[weak]
            show_answer,
            #[weak]
            rating_box,
            #[weak]
            again_btn,
            #[weak]
            hard_btn,
            #[weak]
            good_btn,
            #[weak]
            easy_btn,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                if show_answer.get_visible() && key == gtk4::gdk::Key::space {
                    show_answer.emit_clicked();
                    return glib::Propagation::Stop;
                }
                if rating_box.get_visible() {
                    let btn = match key {
                        gtk4::gdk::Key::_1 => Some(&again_btn),
                        gtk4::gdk::Key::_2 => Some(&hard_btn),
                        gtk4::gdk::Key::_3 => Some(&good_btn),
                        gtk4::gdk::Key::_4 => Some(&easy_btn),
                        _ => None,
                    };
                    if let Some(btn) = btn {
                        btn.emit_clicked();
                        return glib::Propagation::Stop;
                    }
                }
                glib::Propagation::Proceed
            }
        ));
        root.add_controller(key_controller);
        root.set_focusable(true);

        Self { root: root.upcast(), append }
    }
}

/// The first kun reading if there is one (more useful to speak than on
/// readings alone, since most kanji are reviewed in native Japanese words),
/// otherwise the first on reading — stripped of the `.` okurigana marker
/// (e.g. "う.ける" -> "うける") and comma-separated alternates dictionaries
/// list them with, which TTS engines otherwise mispronounce literally.
fn first_reading(entry: &KanjiEntry) -> Option<String> {
    let readings = entry.kun_readings.as_deref().or(entry.on_readings.as_deref())?;
    let first = readings.split(['、', ',']).next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(first.replace('.', ""))
    }
}

/// Build review items by matching each SRS card to its vocabulary/kanji
/// content via `vocab_id`/`kanji_id`. Cards whose referenced entry can't be
/// found are skipped (shouldn't normally happen — it means the entry was
/// deleted without cascading to its SRS card).
pub fn build_items_from_cards(cards: &[SrsCard], vocab: &[VocabularyEntry], kanji: &[KanjiEntry]) -> Vec<ReviewItem> {
    cards
        .iter()
        .filter_map(|card| {
            let content = if let Some(vocab_id) = card.vocab_id {
                vocab.iter().find(|v| v.id == Some(vocab_id)).map(|v| ReviewCardContent {
                    front: v.word.clone(),
                    back: format!("{}\n{}", v.reading.clone().unwrap_or_default(), v.meaning),
                    speak_text: v.reading.clone().unwrap_or_else(|| v.word.clone()),
                })
            } else if let Some(kanji_id) = card.kanji_id {
                kanji.iter().find(|k| k.id == Some(kanji_id)).map(|k| ReviewCardContent {
                    front: k.character.clone(),
                    back: k.meanings.clone(),
                    speak_text: first_reading(k).unwrap_or_else(|| k.character.clone()),
                })
            } else {
                None
            };
            content.map(|content| ReviewItem { card: card.clone(), content })
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample_item(front: &str) -> ReviewItem {
        ReviewItem {
            card: SrsCard::new("vocabulary", "ja"),
            content: ReviewCardContent {
                front: front.to_string(),
                back: "back".to_string(),
                speak_text: front.to_string(),
            },
        }
    }

    // See vocabulary::dialog::tests for why widget construction isn't tested here.

    #[test]
    fn test_build_items_from_cards_matches_vocab() {
        let mut card = SrsCard::new("vocabulary", "ja");
        card.vocab_id = Some(1);
        let vocab = vec![VocabularyEntry {
            id: Some(1),
            word: "受け取る".to_string(),
            reading: Some("うけとる".to_string()),
            meaning: "to receive".to_string(),
            language: "ja".to_string(),
            level: None,
            part_of_speech: None,
            tags: None,
            created_at: None,
            updated_at: None,
        }];

        let items = build_items_from_cards(&[card], &vocab, &[]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content.front, "受け取る");
        assert!(items[0].content.back.contains("to receive"));
    }

    #[test]
    fn test_build_items_from_cards_skips_missing_entries() {
        let mut card = SrsCard::new("vocabulary", "ja");
        card.vocab_id = Some(999); // no matching entry
        let items = build_items_from_cards(&[card], &[], &[]);
        assert!(items.is_empty());
    }
}
