//! Review tab: spaced-repetition review queue. Shows one card at a time
//! (front, then flip to reveal the back), rating buttons that drive the SM-2
//! backend, a progress indicator, and keyboard shortcuts for rating.

use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation};
use langspark_core::{
    KanjiEntry, SrsBackend, SrsCard, VocabularyEntry, SM2Backend, RATING_AGAIN, RATING_EASY, RATING_GOOD,
    RATING_HARD,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// What to show as a card's front/back text — supplied by the caller since
/// the card content (word/reading/meaning) lives in `VocabularyEntry`/`KanjiEntry`,
/// not on `SrsCard` itself.
#[derive(Debug, Clone)]
pub struct ReviewCardContent {
    pub front: String,
    pub back: String,
}

/// One entry in a review session: the SRS scheduling card plus its display content.
#[derive(Debug, Clone)]
pub struct ReviewItem {
    pub card: SrsCard,
    pub content: ReviewCardContent,
}

/// Runs a review session over a queue of cards, calling `on_review` with
/// (index, rating) each time the user rates a card so the caller can persist
/// the SM-2 update (e.g. via `SqliteSrsRepository::update_after_review`).
pub struct ReviewSession {
    pub root: gtk4::Widget,
    queue: Rc<RefCell<Vec<ReviewItem>>>,
    index: Rc<Cell<usize>>,
}

impl ReviewSession {
    pub fn new(queue: Vec<ReviewItem>, on_review: impl Fn(usize, u32) + 'static) -> Self {
        let root = Box::new(Orientation::Vertical, 12);
        root.set_margin_top(12);
        root.set_margin_bottom(12);
        root.set_margin_start(12);
        root.set_margin_end(12);

        let progress_label = Label::builder().css_classes(["caption"]).build();

        let card_text = Label::builder()
            .css_classes(["title-1"])
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .margin_top(24)
            .margin_bottom(24)
            .build();

        let show_answer = gtk4::Button::builder().label("Show Answer").halign(gtk4::Align::Center).build();

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

        root.append(&progress_label);
        root.append(&card_text);
        root.append(&show_answer);
        root.append(&rating_box);

        let queue = Rc::new(RefCell::new(queue));
        let index = Rc::new(Cell::new(0usize));
        let showing_answer = Rc::new(Cell::new(false));
        let on_review = Rc::new(on_review);

        let refresh = {
            let queue = queue.clone();
            let index = index.clone();
            let showing_answer = showing_answer.clone();
            let progress_label = progress_label.clone();
            let card_text = card_text.clone();
            let show_answer_btn = show_answer.clone();
            let rating_box = rating_box.clone();
            move || {
                let q = queue.borrow();
                let total = q.len();
                let i = index.get();
                showing_answer.set(false);
                rating_box.set_visible(false);
                show_answer_btn.set_visible(true);

                if i >= total {
                    progress_label.set_label(&format!("{total} of {total} — done!"));
                    card_text.set_label("Review complete");
                    show_answer_btn.set_visible(false);
                } else {
                    progress_label.set_label(&format!("{} of {total}", i + 1));
                    card_text.set_label(&q[i].content.front);
                }
            }
        };
        refresh();

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
                    {
                        let mut q = queue.borrow_mut();
                        if let Some(item) = q.get_mut(i) {
                            SM2Backend.update_card(&mut item.card, rating);
                        }
                    }
                    on_review(i, rating);
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

        Self { root: root.upcast(), queue, index }
    }

    /// Number of cards remaining (not yet rated) in the session.
    pub fn remaining(&self) -> usize {
        self.queue.borrow().len().saturating_sub(self.index.get())
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
                })
            } else if let Some(kanji_id) = card.kanji_id {
                kanji.iter().find(|k| k.id == Some(kanji_id)).map(|k| ReviewCardContent {
                    front: k.character.clone(),
                    back: k.meanings.clone(),
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
            content: ReviewCardContent { front: front.to_string(), back: "back".to_string() },
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
