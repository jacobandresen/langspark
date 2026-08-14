//! Paragraph read-aloud/translation popover: shown when the reader
//! (`books::reader`) 's per-paragraph speaker icon is clicked. Styled to
//! match `books::popup`/`vocabulary::dialog` — same margins, same
//! boxed-list-row structure, same `subtitle_lines` cap so a long paragraph's
//! translation can't balloon the popup the way an uncapped one did before
//! (see `books::popup`'s Meaning row for the original fix this mirrors).

use adw::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};
use std::rc::Rc;

/// Callbacks the popup needs from the host application.
pub struct SentencePopupCallbacks {
    /// Speak the paragraph aloud. `None` if no TTS backend is available —
    /// the Play button is omitted entirely, matching `books::popup`'s and
    /// `vocabulary::dialog`'s convention.
    pub speak: Option<Rc<dyn Fn(String)>>,
    /// Translate the paragraph in the background: request, on_done(english),
    /// on_error(message) — see `books::reader::ReaderCallbacks::translate_paragraph`.
    pub translate: Rc<dyn Fn(String, Box<dyn Fn(String)>, Box<dyn Fn(String)>)>,
}

/// A paragraph translation runs longer than a single word's meanings, so
/// this gets a taller cap than `books::popup`'s Meaning row (`3`) — still
/// bounded, so a very long paragraph can't push the Play button off screen.
const TRANSLATION_SUBTITLE_LINES: i32 = 5;

/// Build the popover for `japanese` (a whole paragraph's plain text).
/// Kicks off translation immediately (the caller — `reader.rs`'s icon click
/// handler — calls this right when the icon is clicked, so there's no
/// separate "start" step). Caller is responsible for parenting it
/// (`.set_parent(...)`) and calling `.popup()`.
pub fn build(japanese: &str, callbacks: &SentencePopupCallbacks) -> gtk4::Popover {
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.set_size_request(320, -1);

    // `title-4` rather than `books::popup`'s `title-1` — a whole wrapped
    // paragraph doesn't suit as large a headline as a single short word does.
    let text_label =
        Label::builder().label(japanese).css_classes(["title-4"]).xalign(0.0).wrap(true).max_width_chars(36).build();
    root.append(&text_label);

    let translation_row =
        adw::ActionRow::builder().title("Translation").subtitle("Translating\u{2026}").subtitle_lines(TRANSLATION_SUBTITLE_LINES).build();
    let list = gtk4::ListBox::builder().css_classes(["boxed-list"]).build();
    list.append(&translation_row);
    root.append(&list);

    let action_box = GtkBox::new(Orientation::Horizontal, 8);
    action_box.set_margin_top(4);
    if let Some(speak) = &callbacks.speak {
        let speak = speak.clone();
        let text = japanese.to_string();
        // Unlike `books::popup`'s Play button, this one *is*
        // `suggested-action` — nothing else in this popup competes for the
        // accent color, matching `vocabulary::dialog`'s convention of Play
        // being primary when there's no other action around it.
        let play_btn = gtk4::Button::builder().label("\u{25b6} Play").css_classes(["suggested-action"]).build();
        play_btn.connect_clicked(move |_| speak(text.clone()));
        action_box.append(&play_btn);
    }
    root.append(&action_box);

    let popover = gtk4::Popover::builder().child(&root).autohide(true).build();

    let translate = callbacks.translate.clone();
    let done_row = translation_row.clone();
    let error_row = translation_row.clone();
    translate(
        japanese.to_string(),
        Box::new(move |english: String| done_row.set_subtitle(&english)),
        Box::new(move |message: String| error_row.set_subtitle(&message)),
    );

    popover
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn noop_callbacks() -> SentencePopupCallbacks {
        SentencePopupCallbacks {
            speak: Some(Rc::new(|_| {})),
            translate: Rc::new(|_, on_done, _| on_done("translated text".to_string())),
        }
    }

    // Widget-construction is exercised by the consolidated smoke test in
    // `main.rs` (`gtk_smoke` module) rather than here — see
    // `vocabulary::dialog::tests` for why.
}
