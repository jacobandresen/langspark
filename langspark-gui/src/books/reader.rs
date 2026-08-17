//! Book reader widget: renders a parsed book's paragraphs — furigana drawn
//! above their base text — and turns a click, hover, or keyboard-navigated
//! word into a dictionary lookup + popup (`books::popup`). This is the same
//! click-to-look-up interaction real pop-up dictionary reading tools use,
//! since Japanese text has no spaces to mark word boundaries on its own.
//! Hovering (mouse) or navigating with the arrow keys highlights the exact
//! span that would be looked up, and Enter/Space opens the popup for
//! whichever word currently has that highlight — the same outcome as
//! clicking it.
//!
//! A custom `DrawingArea`-backed widget, in the same spirit as
//! `widgets/waveform.rs`: fixed content size (here, a comfortable reading
//! width) rather than reflowing to the viewport, wrapped in a
//! `ScrolledWindow` by the caller's parent for narrower windows.

use crate::books::{popup, sentence_popup};
use gtk4::pango;
use gtk4::prelude::*;
use gtk4::{gdk, DrawingArea, EventControllerFocus, EventControllerKey, EventControllerMotion, GestureClick};
use langspark_core::{BookText, TextRun, VocabEntry, VocabularyEntry};
use std::cell::RefCell;
use std::rc::Rc;

/// Everything the reader needs from the host application: resolving a
/// click/hover/keyboard position into a dictionary entry, speaking a word
/// aloud, saving one to the vocabulary, and translating a whole paragraph.
/// Injected as callbacks (rather than a direct `DictionaryManager`/
/// `AppState` dependency) so this widget stays a plain function of
/// `langspark-core` data types, like every other tab/widget — see
/// ARCHITECTURE.md's "Data flow" section.
#[derive(Clone)]
pub struct ReaderCallbacks {
    /// Resolve the dictionary entry (if any) *containing* `char_index` into
    /// `paragraph_plain_text` — the exact ruby-stripped text a paragraph was
    /// laid out with, so the offset the reader hit-tests lines up with what
    /// this looks up — along with its span as `(start_char, end_char)`, used
    /// both for highlighting and for keyboard word-to-word navigation. Wraps
    /// `DictionaryManager::word_at`.
    pub lookup: Rc<dyn Fn(&str, usize) -> Option<(usize, usize, VocabEntry)>>,
    /// Speak a word (or, from the paragraph popup, a whole paragraph) aloud.
    /// `None` if no TTS backend is available.
    pub speak: Option<Rc<dyn Fn(String)>>,
    /// Save a looked-up entry to the vocabulary — see `popup::PopupCallbacks`.
    pub add_to_vocabulary: Rc<dyn Fn(VocabEntry, Box<dyn Fn(VocabularyEntry)>, Box<dyn Fn()>)>,
    /// Translate a whole paragraph to English in the background — request,
    /// on_done(english), on_error(message), the same async shape as
    /// `books::BooksTabCallbacks::open_book`. Always callable (unlike
    /// `speak`): when no translation model is installed, the callback still
    /// exists, it just always reports that through `on_error` — see
    /// `app.rs`'s `books_tab_callbacks`.
    pub translate_paragraph: Rc<dyn Fn(String, Box<dyn Fn(String)>, Box<dyn Fn(String)>)>,
}

/// A comfortable reading column width. Fixed rather than reflowed to the
/// viewport — like `Waveform`, this widget doesn't respond to allocation
/// changes; narrower windows get a horizontal scrollbar from the
/// `ScrolledWindow` wrapper instead of the text re-wrapping.
const READER_WIDTH_PX: i32 = 680;
const H_MARGIN: i32 = 20;
/// Width reserved on the left for each paragraph's read-aloud/translate
/// icon (see `BookReader::new`'s `icons` overlay) — text starts after this,
/// at `TEXT_X`, rather than at the bare `H_MARGIN` every other edge uses.
const ICON_GUTTER_PX: i32 = 32;
const TEXT_X: i32 = H_MARGIN + ICON_GUTTER_PX;
const V_MARGIN: i32 = 16;
/// Vertical room reserved above every line for its furigana, and used as the
/// gap Pango leaves between a paragraph's own wrapped lines (`Layout::set_spacing`).
const FURIGANA_GAP_PX: i32 = 18;
const PARAGRAPH_GAP_PX: i32 = 12;
const BASE_FONT_PT: i32 = 13;
const FURIGANA_FONT_PT: i32 = 8;
/// Paragraphs with this few characters or fewer don't get a read-aloud/
/// translate icon — Aozora texts mark scene breaks with their own bare
/// section-number paragraphs ("1", "2", "3", ...; visible between scenes in
/// e.g. Akutagawa's 「浅草公園」), and a speaker icon next to a lone digit
/// is just noise.
const MIN_CHARS_FOR_ICON: usize = 2;

/// A word span currently highlighted: which paragraph, and its
/// `[start_char, end_char)` range within that paragraph's plain text.
type WordSpan = (usize, usize, usize);

/// One ruby (furigana) annotation's position within a paragraph's plain
/// text, ready to be positioned against that paragraph's laid-out
/// `pango::Layout` via `index_to_pos`. `layout` is the furigana text's own
/// shaped layout, built once in `BookReader::new` alongside the base
/// paragraph layout — like that layout, it must not be rebuilt per draw call
/// (see `LaidOutParagraph`'s doc comment); the previous version created a
/// brand new `pango::Layout` per ruby mark on every repaint, which for a
/// whole book's worth of furigana on every hover-triggered redraw was the
/// dominant cost in `draw_func`.
struct RubyMark {
    base_byte_start: i32,
    layout: pango::Layout,
}

/// A single paragraph, already shaped into a `pango::Layout` at
/// `READER_WIDTH_PX` — built once in `BookReader::new` rather than per draw
/// call, since re-shaping a whole book's worth of text on every repaint
/// would scale badly for longer works.
struct LaidOutParagraph {
    /// The ruby-stripped text `layout` was built from — what
    /// `ReaderCallbacks::lookup`'s `char_index` indexes into.
    plain_text: String,
    layout: pango::Layout,
    ruby: Vec<RubyMark>,
    /// Top of this paragraph, in the drawing area's own coordinate space.
    y: i32,
    height: i32,
}

/// Convert a char index into `text` to a byte index — what
/// `pango::Layout::index_to_pos`/`xy_to_index` actually operate on.
fn char_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices().nth(char_index).map(|(b, _)| b).unwrap_or(text.len())
}

/// The on-screen rectangle (in the drawing area's own coordinate space)
/// spanning `[start_char, end_char)` of `p`'s laid-out text — used both to
/// draw a highlight behind a word and to anchor a keyboard-triggered popup
/// at it. Assumes the span fits on a single visual line, which every real
/// dictionary word does at this font size and column width.
fn char_range_rect(p: &LaidOutParagraph, start_char: usize, end_char: usize) -> (f64, f64, f64, f64) {
    let start = p.layout.index_to_pos(char_to_byte(&p.plain_text, start_char) as i32);
    let end = p.layout.index_to_pos(char_to_byte(&p.plain_text, end_char) as i32);
    let x = TEXT_X as f64 + (start.x() / pango::SCALE) as f64;
    let y = p.y as f64 + (start.y() / pango::SCALE) as f64;
    let width = ((end.x() - start.x()) / pango::SCALE) as f64;
    let height = (start.height() / pango::SCALE) as f64;
    (x, y, width.max(4.0), height.max(4.0))
}

/// The `ScrolledWindow` wrapping `area` (see `BookReader::new`), found by
/// walking up its ancestor chain rather than threading a reference through
/// every closure that needs it.
fn ancestor_scrolled_window(area: &DrawingArea) -> Option<gtk4::ScrolledWindow> {
    area.ancestor(gtk4::ScrolledWindow::static_type())?.downcast::<gtk4::ScrolledWindow>().ok()
}

/// Grab keyboard focus on `area` without letting it scroll the reader.
/// `area`'s own allocated size is its full, often very tall, laid-out
/// content height (`ScrolledWindow` gives a taller-than-viewport child its
/// whole natural size, then scrolls a viewport within it) — GTK's default
/// scroll-focused-widget-into-view behavior on `grab_focus` treats that
/// entire allocation as "the focus rectangle" to reveal, which for a
/// multi-page-tall widget collapses to jumping the scroll position toward
/// the far end rather than doing nothing sensible. A mouse click already
/// only ever focuses a word that's currently on screen, so there's nothing
/// to scroll to anyway — save and restore the scroll position around the
/// call to neutralize that default.
fn grab_focus_without_scrolling(area: &DrawingArea) {
    let scrolled = ancestor_scrolled_window(area);
    let saved = scrolled.as_ref().map(|s| s.vadjustment().value());
    area.grab_focus();
    if let (Some(scrolled), Some(value)) = (scrolled, saved) {
        scrolled.vadjustment().set_value(value);
    }
}

/// Scroll the reader just enough to bring `[y, y + height)` (in `area`'s own
/// coordinate space — e.g. from `char_range_rect`) into view, used after
/// keyboard-driven focus changes (Tab into the reader, arrow-key word
/// navigation) where the newly-focused word may be off-screen. Does nothing
/// if it's already visible.
fn scroll_into_view(area: &DrawingArea, y: f64, height: f64) {
    if let Some(scrolled) = ancestor_scrolled_window(area) {
        scrolled.vadjustment().clamp_page(y, y + height);
    }
}

/// Scroll `span` into view, if it resolves to a real paragraph — the
/// `paragraphs.get`/`char_range_rect`/`scroll_into_view` sequence shared by
/// every keyboard-driven focus change (Tab into the reader, arrow-key word
/// navigation).
fn reveal_span(area: &DrawingArea, paragraphs: &[LaidOutParagraph], span: WordSpan) {
    let (para_index, start, end) = span;
    if let Some(p) = paragraphs.get(para_index) {
        let (_, y, _, height) = char_range_rect(p, start, end);
        scroll_into_view(area, y, height);
    }
}

/// Resolve a click/hover point (in the drawing area's own coordinate space)
/// to the word span under it, if any — shared by the mouse click handler and
/// the hover-highlight motion handler.
fn hit_test(
    paragraphs: &[LaidOutParagraph],
    lookup: &dyn Fn(&str, usize) -> Option<(usize, usize, VocabEntry)>,
    x: f64,
    y: f64,
) -> Option<(usize, WordSpan, VocabEntry)> {
    let (para_index, p) = paragraphs.iter().enumerate().find(|(_, p)| (y as i32) >= p.y && (y as i32) < p.y + p.height)?;
    let local_x = ((x as i32 - TEXT_X) * pango::SCALE).max(0);
    let local_y = ((y as i32 - p.y) * pango::SCALE).max(0);
    let (inside, byte_index, _trailing) = p.layout.xy_to_index(local_x, local_y);
    if !inside {
        return None;
    }
    let char_index = p.plain_text.get(..byte_index as usize).map(|s| s.chars().count()).unwrap_or(0);
    let (start, end, entry) = lookup(&p.plain_text, char_index)?;
    Some((para_index, (para_index, start, end), entry))
}

/// Find the next recognized word span after `from` (exclusive — `from`'s own
/// `end_char`, or the very start of the book if `None`), scanning forward
/// paragraph by paragraph. Pure and GTK-free (operates on plain paragraph
/// texts, not `LaidOutParagraph`, which needs a live GTK display to build)
/// so it's unit-testable without one — see `BookReader::new` for the one
/// live caller.
fn next_word_span(paragraphs: &[&str], lookup: &dyn Fn(&str, usize) -> Option<(usize, usize, VocabEntry)>, from: Option<WordSpan>) -> Option<WordSpan> {
    let (start_para, start_char) = from.map(|(p, _, end)| (p, end)).unwrap_or((0, 0));
    for (para_index, text) in paragraphs.iter().enumerate().skip(start_para) {
        let boundary = if para_index == start_para { start_char } else { 0 };
        let len = text.chars().count();
        let mut char_index = boundary;
        while char_index < len {
            match lookup(text, char_index) {
                Some((start, end, _)) if start >= boundary => return Some((para_index, start, end)),
                Some((_, end, _)) => char_index = end, // overlaps `from` itself — skip past it
                None => char_index += 1,
            }
        }
    }
    None
}

/// Find the previous recognized word span before `from` (exclusive — its own
/// `start_char`, or the very end of the book if `None`), scanning backward
/// paragraph by paragraph. See `next_word_span` for why this is pure/GTK-free.
fn prev_word_span(paragraphs: &[&str], lookup: &dyn Fn(&str, usize) -> Option<(usize, usize, VocabEntry)>, from: Option<WordSpan>) -> Option<WordSpan> {
    let last_para = paragraphs.len().checked_sub(1)?;
    let (end_para, upper_bound) = from.map(|(p, start, _)| (p, start)).unwrap_or((last_para, paragraphs[last_para].chars().count()));
    for para_index in (0..=end_para).rev() {
        let text = paragraphs[para_index];
        let upper = if para_index == end_para { upper_bound } else { text.chars().count() };
        let mut char_index = upper;
        while char_index > 0 {
            char_index -= 1;
            if let Some((start, end, _)) = lookup(text, char_index) {
                if end <= upper {
                    return Some((para_index, start, end));
                }
                char_index = start; // overlaps the boundary — skip back past it
            }
        }
    }
    None
}

pub struct BookReader {
    pub widget: gtk4::Widget,
}

impl BookReader {
    pub fn new(book: &BookText, callbacks: ReaderCallbacks) -> Self {
        let area = DrawingArea::builder()
            .css_classes(["langspark-book-reader"])
            .cursor(&gdk::Cursor::from_name("text", None).expect("\"text\" is a well-known cursor name"))
            .focusable(true)
            .build();

        let mut base_font = pango::FontDescription::new();
        base_font.set_size(BASE_FONT_PT * pango::SCALE);
        let mut furigana_font = pango::FontDescription::new();
        furigana_font.set_size(FURIGANA_FONT_PT * pango::SCALE);

        let content_width_px = READER_WIDTH_PX - H_MARGIN - TEXT_X;
        let mut paragraphs = Vec::with_capacity(book.paragraphs.len());
        let mut y_cursor = V_MARGIN;

        for paragraph in &book.paragraphs {
            let mut plain_text = String::new();
            let mut ruby = Vec::new();
            for run in &paragraph.runs {
                match run {
                    TextRun::Plain(s) => plain_text.push_str(s),
                    TextRun::Ruby { base, reading } => {
                        let base_byte_start = plain_text.len() as i32;
                        plain_text.push_str(base);
                        let furigana_layout = area.create_pango_layout(Some(reading));
                        furigana_layout.set_font_description(Some(&furigana_font));
                        ruby.push(RubyMark { base_byte_start, layout: furigana_layout });
                    }
                }
            }
            if plain_text.is_empty() {
                continue;
            }

            let layout = area.create_pango_layout(Some(&plain_text));
            layout.set_font_description(Some(&base_font));
            layout.set_width(content_width_px * pango::SCALE);
            layout.set_wrap(pango::WrapMode::WordChar);
            layout.set_spacing(FURIGANA_GAP_PX * pango::SCALE);

            let (_layout_width, layout_height) = layout.pixel_size();
            let y = y_cursor + FURIGANA_GAP_PX; // headroom for furigana above this paragraph's first line
            paragraphs.push(LaidOutParagraph { plain_text, layout, ruby, y, height: layout_height });
            y_cursor = y + layout_height + PARAGRAPH_GAP_PX;
        }

        area.set_content_width(READER_WIDTH_PX);
        area.set_content_height(y_cursor.max(1));
        // Breathing room from the tab edges, matching every other tab's
        // outer content margin — set on the area itself (not the
        // `ScrolledWindow` below) so the `.langspark-book-reader` card
        // background floats inset while the scrollbar stays flush against
        // the window edge.
        area.set_margin_top(12);
        area.set_margin_bottom(12);
        area.set_margin_start(12);
        area.set_margin_end(12);
        area.set_halign(gtk4::Align::Center);

        let paragraphs = Rc::new(paragraphs);
        // Mouse-hover and keyboard-navigated word highlights are tracked
        // separately (rather than one shared "current word" slot) so moving
        // the mouse away from text doesn't erase where keyboard navigation
        // had gotten to, and vice versa. `draw_func` below prefers `hover`
        // when both are set, matching how a mouse click would win over
        // whatever was last keyboard-focused.
        let hover: Rc<RefCell<Option<WordSpan>>> = Rc::new(RefCell::new(None));
        let keyboard_focus: Rc<RefCell<Option<WordSpan>>> = Rc::new(RefCell::new(None));

        area.set_draw_func(glib::clone!(
            #[strong]
            paragraphs,
            #[strong]
            hover,
            #[strong]
            keyboard_focus,
            move |area, cr, _width, _height| {
                let fg = area.color();
                let highlight = hover.borrow().or(*keyboard_focus.borrow());
                let is_keyboard = hover.borrow().is_none() && keyboard_focus.borrow().is_some();

                if let Some((para_index, start, end)) = highlight {
                    if let Some(p) = paragraphs.get(para_index) {
                        let (x, y, width, height) = char_range_rect(p, start, end);
                        // Tinted off the current foreground color (not a
                        // fixed color) so this stays theme-correct in both
                        // light and dark mode automatically. Keyboard focus
                        // additionally gets an outline — a plain fill alone
                        // reads as "hovering" either way, and the user
                        // should be able to tell "the keyboard will act on
                        // this" apart from "the mouse happens to be here".
                        cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, fg.alpha() as f64 * 0.18);
                        rounded_rect(cr, x - 2.0, y - 1.0, width + 4.0, height + 2.0, 4.0);
                        let _ = cr.fill();
                        if is_keyboard {
                            cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, fg.alpha() as f64 * 0.6);
                            cr.set_line_width(1.5);
                            rounded_rect(cr, x - 2.0, y - 1.0, width + 4.0, height + 2.0, 4.0);
                            let _ = cr.stroke();
                        }
                    }
                }

                cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, fg.alpha() as f64);
                for p in paragraphs.iter() {
                    cr.move_to(TEXT_X as f64, p.y as f64);
                    pangocairo::functions::show_layout(cr, &p.layout);

                    for mark in &p.ruby {
                        let rect = p.layout.index_to_pos(mark.base_byte_start);
                        let fx = TEXT_X as f64 + (rect.x() / pango::SCALE) as f64;
                        let fy = p.y as f64 + (rect.y() / pango::SCALE) as f64 - FURIGANA_GAP_PX as f64 + 2.0;

                        cr.move_to(fx, fy);
                        cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, fg.alpha() as f64 * 0.8);
                        pangocairo::functions::show_layout(cr, &mark.layout);
                        cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, fg.alpha() as f64);
                    }
                }
            }
        ));

        let gesture = GestureClick::new();
        gesture.connect_released(glib::clone!(
            #[strong]
            paragraphs,
            #[strong]
            callbacks,
            #[strong]
            keyboard_focus,
            #[weak]
            area,
            move |_gesture, _n_press, x, y| {
                grab_focus_without_scrolling(&area);
                let Some((para_index, span, entry)) = hit_test(&paragraphs, callbacks.lookup.as_ref(), x, y) else { return };
                // A mouse click also moves keyboard focus to the clicked
                // word, so Enter re-opens the same popup and arrow-key
                // navigation continues on from here rather than from
                // wherever keyboard focus last was (or the very start of
                // the book, if it was never used this session).
                *keyboard_focus.borrow_mut() = Some(span);
                let _ = para_index;
                open_popup(&area, &callbacks, entry, gdk::Rectangle::new(x as i32, y as i32, 1, 1));
            }
        ));
        area.add_controller(gesture);

        let motion = EventControllerMotion::new();
        motion.connect_motion(glib::clone!(
            #[strong]
            paragraphs,
            #[strong]
            callbacks,
            #[strong]
            hover,
            #[weak]
            area,
            move |_, x, y| {
                let new_hover = hit_test(&paragraphs, callbacks.lookup.as_ref(), x, y).map(|(_, span, _)| span);
                if *hover.borrow() != new_hover {
                    *hover.borrow_mut() = new_hover;
                    area.queue_draw();
                }
            }
        ));
        motion.connect_leave(glib::clone!(
            #[strong]
            hover,
            #[weak]
            area,
            move |_| {
                if hover.borrow_mut().take().is_some() {
                    area.queue_draw();
                }
            }
        ));
        area.add_controller(motion);

        let focus_controller = EventControllerFocus::new();
        focus_controller.connect_enter(glib::clone!(
            #[strong]
            paragraphs,
            #[strong]
            callbacks,
            #[strong]
            keyboard_focus,
            #[weak]
            area,
            move |_| {
                // First time the reader gets keyboard focus this session:
                // land on the book's very first word, so the highlight
                // (this widget's only real focus indicator — a bare
                // `DrawingArea` has no default one) appears immediately
                // instead of the user having to guess that Tab even did
                // anything.
                if keyboard_focus.borrow().is_none() {
                    let texts: Vec<&str> = paragraphs.iter().map(|p| p.plain_text.as_str()).collect();
                    if let Some(span) = next_word_span(&texts, callbacks.lookup.as_ref(), None) {
                        *keyboard_focus.borrow_mut() = Some(span);
                        // Tabbing into the reader is itself a GTK focus
                        // change on `area`, subject to the same
                        // scroll-the-whole-allocation-into-view default that
                        // `grab_focus_without_scrolling` works around for
                        // clicks — reasserting our own scroll target here
                        // corrects it back to showing the first word (which
                        // is what a keyboard user landing on the reader
                        // actually wants to see) regardless of where GTK's
                        // own default happened to leave the scroll position.
                        reveal_span(&area, &paragraphs, span);
                        area.queue_draw();
                    }
                } else {
                    area.queue_draw();
                }
            }
        ));
        area.add_controller(focus_controller);

        let key_controller = EventControllerKey::new();
        key_controller.connect_key_pressed(glib::clone!(
            #[strong]
            paragraphs,
            #[strong]
            callbacks,
            #[strong]
            keyboard_focus,
            #[weak]
            area,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_, key, _, _| {
                match key {
                    gdk::Key::Right | gdk::Key::Down => {
                        let texts: Vec<&str> = paragraphs.iter().map(|p| p.plain_text.as_str()).collect();
                        // `keyboard_focus.borrow()` is copied out to `from`
                        // in its own statement rather than inlined into the
                        // `if let`'s scrutinee — a temporary `Ref` created
                        // there would otherwise stay alive for the whole
                        // `if let` body (a well-known Rust temporary-scope
                        // pitfall), and the `borrow_mut()` below would then
                        // panic against its own still-live read borrow.
                        let from = *keyboard_focus.borrow();
                        if let Some(span) = next_word_span(&texts, callbacks.lookup.as_ref(), from) {
                            *keyboard_focus.borrow_mut() = Some(span);
                            reveal_span(&area, &paragraphs, span);
                            area.queue_draw();
                        }
                        glib::Propagation::Stop
                    }
                    gdk::Key::Left | gdk::Key::Up => {
                        let texts: Vec<&str> = paragraphs.iter().map(|p| p.plain_text.as_str()).collect();
                        let from = *keyboard_focus.borrow();
                        if let Some(span) = prev_word_span(&texts, callbacks.lookup.as_ref(), from) {
                            *keyboard_focus.borrow_mut() = Some(span);
                            reveal_span(&area, &paragraphs, span);
                            area.queue_draw();
                        }
                        glib::Propagation::Stop
                    }
                    gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::space => {
                        let Some((para_index, start, end)) = *keyboard_focus.borrow() else {
                            return glib::Propagation::Proceed;
                        };
                        let Some(p) = paragraphs.get(para_index) else { return glib::Propagation::Proceed };
                        let Some((_, _, entry)) = (callbacks.lookup)(&p.plain_text, start) else {
                            return glib::Propagation::Proceed;
                        };
                        let (x, y, width, height) = char_range_rect(p, start, end);
                        open_popup(&area, &callbacks, entry, gdk::Rectangle::new(x as i32, y as i32, width as i32, height as i32));
                        glib::Propagation::Stop
                    }
                    _ => glib::Propagation::Proceed,
                }
            }
        ));
        area.add_controller(key_controller);

        // One read-aloud/translate icon per (non-trivial) paragraph, laid
        // out at each paragraph's own `y` in the gutter `TEXT_X` makes room
        // for. Real `Button`s here (rather than hand-drawing icons in
        // `draw_func` and hand-hit-testing them, the way word clicks are
        // handled) get hover/press states and accessibility for free, and
        // don't need any hit-test math of their own — clicks that land on
        // one never reach `area`'s `GestureClick` underneath, and clicks
        // that don't land on one fall through to it untouched.
        let icons = gtk4::Fixed::new();
        // GTK4 pointer picking hit-tests a container's whole allocated box,
        // not just where its children actually sit — an `Align::Fill`
        // `Fixed` here would silently swallow every click meant for the
        // text below it (word lookup, hover) anywhere in the reading
        // column, not just over an icon button. Sizing it to exactly the
        // gutter column (which real text never occupies, since it starts
        // at `TEXT_X`) keeps it out of the text's way while the buttons
        // inside it stay clickable.
        icons.set_halign(gtk4::Align::Start);
        icons.set_valign(gtk4::Align::Fill);
        icons.set_size_request(TEXT_X, -1);
        for p in paragraphs.iter() {
            if p.plain_text.chars().count() <= MIN_CHARS_FOR_ICON {
                continue;
            }
            let icon_btn = gtk4::Button::builder()
                .icon_name("audio-speakers-symbolic")
                .css_classes(["flat", "circular"])
                .tooltip_text("Hear this paragraph and see its translation")
                .build();
            let paragraph_text = p.plain_text.clone();
            icon_btn.connect_clicked(glib::clone!(
                #[strong]
                callbacks,
                #[strong]
                paragraph_text,
                move |btn| {
                    let popover = sentence_popup::build(
                        &paragraph_text,
                        &sentence_popup::SentencePopupCallbacks {
                            speak: callbacks.speak.clone(),
                            translate: callbacks.translate_paragraph.clone(),
                        },
                    );
                    popover.set_parent(btn);
                    popover.popup();
                }
            ));
            icons.put(&icon_btn, H_MARGIN as f64, (p.y - 4).max(0) as f64);
        }

        let overlay = gtk4::Overlay::new();
        overlay.set_child(Some(&area));
        overlay.add_overlay(&icons);

        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .child(&overlay)
            .build();

        Self { widget: scroller.upcast() }
    }
}

/// Draw a rounded rectangle path (without filling/stroking it — the caller
/// does that, since the highlight in `draw_func` reuses the same path shape
/// for both a fill and, for keyboard focus, an outline stroke).
fn rounded_rect(cr: &gtk4::cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    cr.new_sub_path();
    cr.arc(x + width - radius, y + radius, radius, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + width - radius, y + height - radius, radius, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + radius, y + height - radius, radius, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + radius, y + radius, radius, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();
}

/// Open the word popup for `entry`, anchored at `rect` (a click point as a
/// 1×1 rectangle, or a keyboard-focused word's actual bounding box) — shared
/// by the mouse click handler and the keyboard Enter/Space handler so both
/// produce the exact same popup.
fn open_popup(area: &DrawingArea, callbacks: &ReaderCallbacks, entry: VocabEntry, rect: gdk::Rectangle) {
    let popover =
        popup::build(&entry, &popup::PopupCallbacks { speak: callbacks.speak.clone(), add_to_vocabulary: callbacks.add_to_vocabulary.clone() });
    popover.set_parent(area);
    popover.set_pointing_to(Some(&rect));
    // `area`'s own local coordinate space is many pages tall (its full
    // laid-out content, not just the visible viewport — see
    // `grab_focus_without_scrolling`), so a word near the very top of that
    // whole space sits close to (0, 0) with no room above it for the
    // default `PositionType::Top` to place the popover, even though on
    // screen it may be nowhere near the actual window's top edge. Anchoring
    // below the word instead sidesteps that: there's always at least a
    // popover's worth of scrollable content below any real word.
    popover.set_position(gtk4::PositionType::Bottom);
    popover.popup();
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use langspark_core::BookParagraph;

    pub(crate) fn noop_callbacks() -> ReaderCallbacks {
        ReaderCallbacks {
            lookup: Rc::new(|_, _| None),
            speak: Some(Rc::new(|_| {})),
            add_to_vocabulary: Rc::new(|_, _, _| {}),
            translate_paragraph: Rc::new(|_, _, on_error| on_error("translation unavailable in tests".to_string())),
        }
    }

    pub(crate) fn sample_book() -> BookText {
        BookText {
            paragraphs: vec![BookParagraph {
                runs: vec![
                    TextRun::Plain("彼は".to_string()),
                    TextRun::Ruby { base: "受け取る".to_string(), reading: "うけとる".to_string() },
                    TextRun::Plain("。".to_string()),
                ],
            }],
        }
    }

    // Widget-construction is exercised by the consolidated smoke test in
    // `main.rs` (`gtk_smoke` module) rather than here — see
    // `vocabulary::dialog::tests` for why.

    /// A tiny stand-in dictionary for `next_word_span`/`prev_word_span`
    /// tests: recognizes exactly "猫" and "食べる" (anywhere either occurs),
    /// nothing else — enough to exercise multi-word, multi-paragraph, and
    /// "nothing left" navigation without needing a real `DictionaryManager`.
    fn toy_lookup(text: &str, char_index: usize) -> Option<(usize, usize, VocabEntry)> {
        let chars: Vec<char> = text.chars().collect();
        let words: &[&str] = &["食べる", "猫"];
        for start in (0..=char_index).rev() {
            if char_index >= chars.len() {
                return None;
            }
            for word in words {
                let word_chars: Vec<char> = word.chars().collect();
                let end = start + word_chars.len();
                if end > chars.len() || end <= char_index {
                    continue;
                }
                if chars[start..end] == word_chars[..] {
                    return Some((
                        start,
                        end,
                        VocabEntry {
                            id: word.to_string(),
                            word: word.to_string(),
                            reading: None,
                            meanings: vec![],
                            part_of_speech: vec![],
                            level: None,
                            language: "ja".to_string(),
                            examples: vec![],
                        },
                    ));
                }
            }
        }
        None
    }

    #[test]
    fn test_next_word_span_finds_first_word_from_none() {
        let paragraphs = ["猫がいる。"];
        let span = next_word_span(&paragraphs, &toy_lookup, None).unwrap();
        assert_eq!(span, (0, 0, 1)); // "猫"
    }

    #[test]
    fn test_next_word_span_advances_past_current_word() {
        let paragraphs = ["猫が食べる。"];
        let first = next_word_span(&paragraphs, &toy_lookup, None).unwrap();
        let second = next_word_span(&paragraphs, &toy_lookup, Some(first)).unwrap();
        assert_eq!(second, (0, 2, 5)); // "食べる", after "猫が"
    }

    #[test]
    fn test_next_word_span_crosses_paragraph_boundary() {
        let paragraphs = ["猫。", "食べる。"];
        let first = next_word_span(&paragraphs, &toy_lookup, None).unwrap();
        assert_eq!(first.0, 0);
        let second = next_word_span(&paragraphs, &toy_lookup, Some(first)).unwrap();
        assert_eq!(second, (1, 0, 3)); // next paragraph's "食べる"
    }

    #[test]
    fn test_next_word_span_returns_none_at_end_of_book() {
        let paragraphs = ["猫。"];
        let first = next_word_span(&paragraphs, &toy_lookup, None).unwrap();
        assert!(next_word_span(&paragraphs, &toy_lookup, Some(first)).is_none());
    }

    #[test]
    fn test_prev_word_span_finds_last_word_from_none() {
        let paragraphs = ["猫が食べる。"];
        let span = prev_word_span(&paragraphs, &toy_lookup, None).unwrap();
        assert_eq!(span, (0, 2, 5)); // "食べる"
    }

    #[test]
    fn test_prev_word_span_retreats_before_current_word() {
        let paragraphs = ["猫が食べる。"];
        let last = prev_word_span(&paragraphs, &toy_lookup, None).unwrap();
        let before = prev_word_span(&paragraphs, &toy_lookup, Some(last)).unwrap();
        assert_eq!(before, (0, 0, 1)); // "猫", before "食べる"
    }

    #[test]
    fn test_prev_word_span_crosses_paragraph_boundary() {
        let paragraphs = ["猫。", "食べる。"];
        let last = prev_word_span(&paragraphs, &toy_lookup, None).unwrap();
        assert_eq!(last.0, 1);
        let before = prev_word_span(&paragraphs, &toy_lookup, Some(last)).unwrap();
        assert_eq!(before, (0, 0, 1)); // previous paragraph's "猫"
    }

    #[test]
    fn test_prev_word_span_returns_none_at_start_of_book() {
        let paragraphs = ["猫。"];
        let first = prev_word_span(&paragraphs, &toy_lookup, None).unwrap();
        assert!(prev_word_span(&paragraphs, &toy_lookup, Some(first)).is_none());
    }

    #[test]
    fn test_next_word_span_skips_unrecognized_text() {
        // "は" and "、" between the two recognized words aren't in
        // `toy_lookup`'s tiny vocabulary — next/prev must step past them
        // rather than getting stuck or matching garbage.
        let paragraphs = ["猫は、食べる。"];
        let first = next_word_span(&paragraphs, &toy_lookup, None).unwrap();
        let second = next_word_span(&paragraphs, &toy_lookup, Some(first)).unwrap();
        assert_eq!(second.1, 3); // "食べる" starts after "猫は、"
    }
}
