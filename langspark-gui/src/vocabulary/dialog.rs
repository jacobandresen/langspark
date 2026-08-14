//! Vocabulary detail dialog: large word display, meaning/POS/level, example
//! sentence, audio playback, delete.
//!
//! Practice-navigation, add-to-deck, and edit were removed rather than left
//! as inert buttons: none of them has a real backing UI yet (a way to jump
//! the pronunciation tab to a specific word, a deck picker, an edit form).
//! Re-add them once that UI exists.

use adw::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};
use langspark_core::{ExampleSentence, VocabularyEntry};
use std::rc::Rc;

/// Callbacks for the actions available from the detail dialog.
pub struct VocabularyDialogCallbacks {
    /// Speak arbitrary text aloud — used both for the main word Play button
    /// (word/reading) and each example sentence's own Play button (its
    /// Japanese text). `None` if no TTS backend is available for the active
    /// language — Play buttons are omitted entirely rather than shown
    /// disabled with no explanation.
    pub speak: Option<Rc<dyn Fn(String)>>,
    /// Delete this entry. The dialog closes immediately (optimistic UI); if
    /// the underlying delete fails, the caller is responsible for surfacing
    /// that separately (e.g. a toast) since the dialog is already gone.
    pub on_delete: Box<dyn Fn() + 'static>,
}

/// Build a vocabulary detail dialog for `entry`, showing `examples` (empty
/// if none are available — see `ExampleSentence`). Caller is responsible for
/// calling `.present(Some(parent))` on the returned dialog.
pub fn build(entry: &VocabularyEntry, examples: &[ExampleSentence], callbacks: VocabularyDialogCallbacks) -> adw::Dialog {
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let word = Label::builder().label(&entry.word).css_classes(["title-1"]).build();
    root.append(&word);

    // Phonetic guide: furigana would render above the kanji in a real
    // implementation; without a furigana-capable widget we show the reading
    // beneath the word instead.
    if let Some(reading) = &entry.reading {
        root.append(&Label::builder().label(reading).css_classes(["title-4", "dim-label"]).build());
    }

    // `subtitle_lines` caps (and ellipsizes) how tall this row can grow — a
    // word with a long meaning otherwise keeps wrapping indefinitely (see
    // the matching fix in `books::popup`, which this dialog is meant to
    // look and behave the same as).
    let meaning_row = adw::ActionRow::builder().title("Meaning").subtitle(&entry.meaning).subtitle_lines(3).build();
    let list = gtk4::ListBox::builder().css_classes(["boxed-list"]).margin_top(12).build();
    list.append(&meaning_row);

    if let Some(pos) = &entry.part_of_speech {
        list.append(&adw::ActionRow::builder().title("Part of speech").subtitle(pos).build());
    }
    if let Some(level) = &entry.level {
        list.append(&adw::ActionRow::builder().title("Level").subtitle(level).build());
    }
    root.append(&list);

    let example_title =
        if examples.len() > 1 { format!("Example sentences ({})", examples.len()) } else { "Example sentence".to_string() };
    let example_row = adw::ExpanderRow::builder().title(example_title).build();
    if examples.is_empty() {
        example_row.add_row(&adw::ActionRow::builder().title("No example available for this word").build());
    } else {
        for example in examples {
            let row = adw::ActionRow::builder().title(&example.japanese).subtitle(&example.english).build();
            if let Some(speak) = &callbacks.speak {
                let speak = speak.clone();
                let japanese = example.japanese.clone();
                let play_btn = gtk4::Button::builder()
                    .icon_name("media-playback-start-symbolic")
                    .valign(gtk4::Align::Center)
                    .tooltip_text("Play this sentence")
                    .build();
                play_btn.connect_clicked(move |_| speak(japanese.clone()));
                row.add_suffix(&play_btn);
            }
            example_row.add_row(&row);
        }
    }
    let example_list = gtk4::ListBox::builder().css_classes(["boxed-list"]).margin_top(8).build();
    example_list.append(&example_row);
    root.append(&example_list);

    let action_box = GtkBox::new(Orientation::Horizontal, 8);
    action_box.set_margin_top(12);
    if let Some(speak) = &callbacks.speak {
        let speak = speak.clone();
        let word_text = entry.reading.clone().unwrap_or_else(|| entry.word.clone());
        let play_btn = gtk4::Button::builder().label("▶ Play").css_classes(["suggested-action"]).build();
        play_btn.connect_clicked(move |_| speak(word_text.clone()));
        action_box.append(&play_btn);
    }
    let delete_btn = gtk4::Button::builder().label("Delete").css_classes(["destructive-action"]).build();
    action_box.append(&delete_btn);
    root.append(&action_box);

    let dialog = adw::Dialog::builder().title(&entry.word).content_width(420).content_height(420).child(&root).build();

    delete_btn.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            (callbacks.on_delete)();
            dialog.close();
        }
    ));

    dialog
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample_entry() -> VocabularyEntry {
        VocabularyEntry {
            id: Some(1),
            word: "受け取る".to_string(),
            reading: Some("うけとる".to_string()),
            meaning: "to receive".to_string(),
            language: "ja".to_string(),
            level: Some("N4".to_string()),
            part_of_speech: Some("verb".to_string()),
            tags: None,
            created_at: None,
            updated_at: None,
        }
    }

    pub(crate) fn noop_callbacks() -> VocabularyDialogCallbacks {
        VocabularyDialogCallbacks { speak: Some(Rc::new(|_| {})), on_delete: Box::new(|| {}) }
    }

    // Widget-construction is exercised by the consolidated smoke test in
    // `main.rs` (`gtk_smoke` module) rather than here: GTK can only be
    // initialized from one OS thread per process, but the `#[test]` harness
    // runs every test function on its own thread, so per-module GTK tests
    // reliably crash with "Attempted to initialize GTK from two different
    // threads" as soon as more than one such test exists in the binary.
}
