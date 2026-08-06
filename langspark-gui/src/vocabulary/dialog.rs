//! Vocabulary detail dialog: large word display, meaning/POS/level, example
//! sentence, audio controls, "practice pronunciation", add-to-deck, edit/delete.

use adw::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};
use langspark_core::VocabularyEntry;

/// Callbacks for the actions available from the detail dialog.
pub struct VocabularyDialogCallbacks {
    pub on_play_audio: Box<dyn Fn() + 'static>,
    pub on_practice: Box<dyn Fn() + 'static>,
    pub on_add_to_deck: Box<dyn Fn() + 'static>,
    pub on_edit: Box<dyn Fn() + 'static>,
    pub on_delete: Box<dyn Fn() + 'static>,
}

/// Build a vocabulary detail dialog for `entry`. Caller is responsible for
/// calling `.present(Some(parent))` on the returned dialog.
pub fn build(entry: &VocabularyEntry, callbacks: VocabularyDialogCallbacks) -> adw::Dialog {
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let word = Label::builder().label(&entry.word).css_classes(["title-1"]).build();
    root.append(&word);

    // Phonetic guide: furigana for Japanese would render above the kanji in a
    // real implementation; without a furigana-capable widget we show the
    // reading beneath the word, which also serves Spanish's phonetic guide.
    if let Some(reading) = &entry.reading {
        root.append(&Label::builder().label(reading).css_classes(["title-4", "dim-label"]).build());
    }

    let meaning_row = adw::ActionRow::builder().title("Meaning").subtitle(&entry.meaning).build();
    let list = gtk4::ListBox::builder().css_classes(["boxed-list"]).margin_top(12).build();
    list.append(&meaning_row);

    if let Some(pos) = &entry.part_of_speech {
        list.append(&adw::ActionRow::builder().title("Part of speech").subtitle(pos).build());
    }
    if let Some(level) = &entry.level {
        list.append(&adw::ActionRow::builder().title("Level").subtitle(level).build());
    }
    root.append(&list);

    // Example sentence placeholder (populated once example-sentence data exists)
    let example_row = adw::ExpanderRow::builder().title("Example sentence").build();
    example_row.add_row(&adw::ActionRow::builder().title("No example available yet").build());
    let example_list = gtk4::ListBox::builder().css_classes(["boxed-list"]).margin_top(8).build();
    example_list.append(&example_row);
    root.append(&example_list);

    let action_box = GtkBox::new(Orientation::Horizontal, 8);
    action_box.set_margin_top(12);
    let play_btn = gtk4::Button::builder().label("▶ Play").build();
    let practice_btn = gtk4::Button::builder().label("Practice Pronunciation").css_classes(["suggested-action"]).build();
    let add_deck_btn = gtk4::Button::builder().label("Add to Deck").build();
    action_box.append(&play_btn);
    action_box.append(&practice_btn);
    action_box.append(&add_deck_btn);
    root.append(&action_box);

    let edit_box = GtkBox::new(Orientation::Horizontal, 8);
    let edit_btn = gtk4::Button::builder().label("Edit").build();
    let delete_btn = gtk4::Button::builder().label("Delete").css_classes(["destructive-action"]).build();
    edit_box.append(&edit_btn);
    edit_box.append(&delete_btn);
    root.append(&edit_box);

    play_btn.connect_clicked(move |_| (callbacks.on_play_audio)());
    practice_btn.connect_clicked(move |_| (callbacks.on_practice)());
    add_deck_btn.connect_clicked(move |_| (callbacks.on_add_to_deck)());
    edit_btn.connect_clicked(move |_| (callbacks.on_edit)());
    delete_btn.connect_clicked(move |_| (callbacks.on_delete)());

    adw::Dialog::builder().title(&entry.word).content_width(420).content_height(480).child(&root).build()
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
        VocabularyDialogCallbacks {
            on_play_audio: Box::new(|| {}),
            on_practice: Box::new(|| {}),
            on_add_to_deck: Box::new(|| {}),
            on_edit: Box::new(|| {}),
            on_delete: Box::new(|| {}),
        }
    }

    // Widget-construction is exercised by the consolidated smoke test in
    // `main.rs` (`gtk_smoke` module) rather than here: GTK can only be
    // initialized from one OS thread per process, but the `#[test]` harness
    // runs every test function on its own thread, so per-module GTK tests
    // reliably crash with "Attempted to initialize GTK from two different
    // threads" as soon as more than one such test exists in the binary.
}
