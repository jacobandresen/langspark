//! Word lookup popover: shown when the reader (`books::reader`) resolves a
//! click to a dictionary entry. Deliberately styled to match
//! `vocabulary::dialog`'s full detail dialog exactly (same typography, same
//! boxed-list rows) so a word looked up while reading feels like the same
//! piece of UI as one opened from the Vocabulary tab — just in a `Popover`
//! rather than a modal `Dialog`, since interrupting reading for every word
//! looked up would defeat the point of an inline reader. Example sentences
//! are the one thing left out, to keep it from growing as tall as the full
//! dialog while anchored mid-page.

use adw::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};
use langspark_core::{VocabEntry, VocabularyEntry};
use std::rc::Rc;

/// Callbacks the popup needs from the host application.
pub struct PopupCallbacks {
    /// Speak the word aloud. `None` if no TTS backend is available for the
    /// active language — the Play button is omitted entirely rather than
    /// shown disabled, matching `vocabulary::dialog`'s convention.
    pub speak: Option<Rc<dyn Fn(String)>>,
    /// Save this dictionary entry into the user's vocabulary. Same shape as
    /// `vocabulary::lookup::AddWordCallbacks::persist` — reuses the exact
    /// same write path (`app.rs`'s `build_persist_callback`), so a word
    /// added from a book gets an immediately-due SRS card just like one
    /// added from the dictionary search dialog.
    pub add_to_vocabulary: Rc<dyn Fn(VocabEntry, Box<dyn Fn(VocabularyEntry)>, Box<dyn Fn()>)>,
}

/// Build the popover for `entry`. Caller is responsible for parenting it
/// (`.set_parent(...)`), pointing it at the click location
/// (`.set_pointing_to(...)`), and calling `.popup()`.
pub fn build(entry: &VocabEntry, callbacks: &PopupCallbacks) -> gtk4::Popover {
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.set_size_request(280, -1);

    // Same word/reading typography as `vocabulary::dialog::build`.
    let word = Label::builder().label(&entry.word).css_classes(["title-1"]).xalign(0.0).build();
    root.append(&word);

    if let Some(reading) = &entry.reading {
        if reading != &entry.word {
            root.append(&Label::builder().label(reading).css_classes(["title-4", "dim-label"]).xalign(0.0).build());
        }
    }

    // Same boxed-list-of-ActionRows structure as the dialog (Meaning, then
    // Part of speech/Level if present), not a plain label — this is the
    // biggest single thing that made the two popups look unrelated before.
    // `subtitle_lines` caps (and ellipsizes) how tall this row can grow —
    // without it, a word with many JMdict senses joined by "; " makes the
    // popup balloon well past the click point it's anchored to.
    let meaning_row =
        adw::ActionRow::builder().title("Meaning").subtitle(entry.meanings.join("; ")).subtitle_lines(3).build();
    let list = gtk4::ListBox::builder().css_classes(["boxed-list"]).build();
    list.append(&meaning_row);

    if !entry.part_of_speech.is_empty() {
        list.append(&adw::ActionRow::builder().title("Part of speech").subtitle(entry.part_of_speech.join(", ")).build());
    }
    if let Some(level) = &entry.level {
        list.append(&adw::ActionRow::builder().title("Level").subtitle(level).build());
    }
    root.append(&list);

    let action_box = GtkBox::new(Orientation::Horizontal, 8);
    action_box.set_margin_top(4);

    if let Some(speak) = &callbacks.speak {
        let speak = speak.clone();
        let word_text = entry.reading.clone().unwrap_or_else(|| entry.word.clone());
        // Plain (not `suggested-action`) here, unlike the dialog's Play
        // button — Add to Vocabulary is this popup's primary action, and
        // only one button per view should carry the accent color (see
        // Review/Pronunciation's single "suggested-action" rating/Play
        // button for the same convention).
        let play_btn = gtk4::Button::builder().label("▶ Play").build();
        play_btn.connect_clicked(move |_| speak(word_text.clone()));
        action_box.append(&play_btn);
    }

    let add_btn =
        gtk4::Button::builder().label("Add to Vocabulary").css_classes(["suggested-action"]).hexpand(true).build();
    action_box.append(&add_btn);
    root.append(&action_box);

    let add = callbacks.add_to_vocabulary.clone();
    let dict_entry = entry.clone();
    add_btn.connect_clicked(move |btn| {
        btn.set_sensitive(false);
        btn.set_label("Adding\u{2026}");
        let done_btn = btn.clone();
        let error_btn = btn.clone();
        add(
            dict_entry.clone(),
            Box::new(move |_saved| done_btn.set_label("Added")),
            Box::new(move || {
                error_btn.set_label("Add to Vocabulary");
                error_btn.set_sensitive(true);
            }),
        );
    });

    gtk4::Popover::builder().child(&root).autohide(true).build()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample_entry() -> VocabEntry {
        VocabEntry {
            id: "1".to_string(),
            word: "受け取る".to_string(),
            reading: Some("うけとる".to_string()),
            meanings: vec!["to receive".to_string()],
            part_of_speech: vec!["verb".to_string()],
            level: None,
            language: "ja".to_string(),
            examples: Vec::new(),
        }
    }

    pub(crate) fn noop_callbacks() -> PopupCallbacks {
        PopupCallbacks { speak: Some(Rc::new(|_| {})), add_to_vocabulary: Rc::new(|_, _, _| {}) }
    }

    // Widget-construction is exercised by the consolidated smoke test in
    // `main.rs` (`gtk_smoke` module) rather than here — see
    // `vocabulary::dialog::tests` for why.
}
