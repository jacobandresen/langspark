//! Vocabulary tab: browse vocabulary entries grouped into sections (JLPT
//! level), each shown as a horizontal strip of cards with a "Show All"
//! button that expands to a full grid.

pub mod dialog;
pub mod lookup;

pub use lookup::AddWordCallbacks;

use adw::prelude::*;
use gtk4::{Box, FlowBox, Label, Orientation, Revealer, ScrolledWindow};
use langspark_core::VocabularyEntry;
use std::boxed::Box as StdBox;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Callbacks a card needs to back its detail dialog's Play/Delete buttons.
/// `remove` is internal (see `build_tab`): it drops the entry from the tab's
/// live list once `delete` reports success, so the card disappears without
/// a full reload.
#[derive(Clone)]
struct CardCallbacks {
    on_play: Option<Rc<dyn Fn(String)>>,
    delete: Rc<dyn Fn(i64, StdBox<dyn Fn()>, StdBox<dyn Fn()>)>,
    remove: Rc<dyn Fn(i64)>,
    /// Look up example sentences for a word (by exact text match against the
    /// loaded dictionary). `None` if no dictionary is loaded for the active
    /// language, in which case the dialog just shows "no example available".
    example_lookup: Option<Rc<dyn Fn(&str) -> Vec<langspark_core::ExampleSentence>>>,
}

/// A single vocabulary entry rendered as a clickable card. Clicking it opens
/// the detail dialog (`dialog::build`), wired to `callbacks`.
fn build_card(entry: &VocabularyEntry, callbacks: &CardCallbacks) -> gtk4::Button {
    let content = Box::new(Orientation::Vertical, 3);
    content.set_size_request(148, -1);

    // `max_width_chars` + `ellipsize` (rather than `wrap`) keeps every card
    // in a row the same width regardless of word length — a long katakana
    // loanword (e.g. "コミュニケーションギャップ") would otherwise size the
    // label to its full unwrapped width, stretching that one card past its
    // neighbors and breaking the strip/grid's uniform layout. The full text
    // is still reachable via the card's tooltip below.
    let word = Label::builder()
        .label(&entry.word)
        .css_classes(["title-4"])
        .xalign(0.0)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(9)
        .build();
    content.append(&word);

    if let Some(reading) = &entry.reading {
        let reading_label = Label::builder()
            .label(reading)
            .css_classes(["caption", "dim-label"])
            .xalign(0.0)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .max_width_chars(14)
            .build();
        content.append(&reading_label);
    }

    let meaning = Label::builder()
        .label(&entry.meaning)
        .xalign(0.0)
        .wrap(true)
        .lines(2)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .max_width_chars(20)
        .margin_top(2)
        .build();
    content.append(&meaning);

    let card = gtk4::Button::builder().child(&content).css_classes(["card", "langspark-card"]).build();
    // Full word (unellipsized) plus its meaning, so nothing truncated above
    // is lost — not just the meaning, which was all the tooltip carried before.
    let tooltip = match &entry.reading {
        Some(reading) if reading != &entry.word => format!("{}\u{3000}({reading})\n{}", entry.word, entry.meaning),
        _ => format!("{}\n{}", entry.word, entry.meaning),
    };
    card.set_tooltip_text(Some(&tooltip));

    let dialog_entry = entry.clone();
    let callbacks = callbacks.clone();
    card.connect_clicked(move |btn| {
        let id = dialog_entry.id;
        let delete = callbacks.delete.clone();
        let remove = callbacks.remove.clone();
        let on_delete: StdBox<dyn Fn()> = StdBox::new(move || {
            let Some(id) = id else { return };
            let remove = remove.clone();
            delete(id, StdBox::new(move || remove(id)), StdBox::new(|| {}));
        });

        let examples = callbacks.example_lookup.as_ref().map(|lookup| lookup(&dialog_entry.word)).unwrap_or_default();

        let dialog = dialog::build(
            &dialog_entry,
            &examples,
            dialog::VocabularyDialogCallbacks { speak: callbacks.on_play.clone(), on_delete },
        );
        dialog.present(Some(btn));
    });

    card
}

/// A labeled section (e.g. "N4") showing a horizontal strip of its entries
/// with a +/- toggle that expands to reveal the rest in a wrapping grid.
/// `header_css` picks the header's visual weight — `"langspark-section-header"`
/// for a top-level shelf, `"langspark-subsection-header"` for one of
/// "School"'s nested N5/N4/N3 sub-shelves (see `build_shelf`).
fn build_section(level: &str, entries: &[&VocabularyEntry], callbacks: &CardCallbacks, header_css: &str) -> gtk4::Box {
    let section = Box::new(Orientation::Vertical, 6);

    let header = Box::new(Orientation::Horizontal, 8);
    header.set_valign(gtk4::Align::Center);
    let title = Label::builder().label(level).css_classes([header_css]).xalign(0.0).hexpand(true).build();
    header.append(&title);

    let show_all = gtk4::ToggleButton::builder()
        .label("+")
        .css_classes(["circular", "flat", "langspark-expand-toggle"])
        .tooltip_text("Expand")
        .build();
    header.append(&show_all);
    section.append(&header);

    // Compact preview: the first few entries. A `FlowBox` (not a horizontal
    // strip in a `ScrolledWindow`, this section's previous approach) so a
    // card that doesn't fit on the current row wraps onto a new one instead
    // of being cut off at the window's edge — there was always *some* window
    // width at which a horizontally-scrolling strip's last card sat exactly
    // on that edge, half-visible. Capped to the same 6 entries the strip
    // used to show; the "Show All" toggle below reveals the rest.
    let preview = FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .max_children_per_line(6)
        .row_spacing(10)
        .column_spacing(10)
        .margin_top(2)
        .build();
    for entry in entries.iter().take(6) {
        preview.insert(&build_card(entry, callbacks), -1);
    }
    section.append(&preview);

    // Full grid, revealed by the +/- toggle.
    let grid = FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .max_children_per_line(6)
        .row_spacing(10)
        .column_spacing(10)
        .build();
    for entry in entries {
        grid.insert(&build_card(entry, callbacks), -1);
    }
    let revealer = Revealer::builder()
        .transition_type(gtk4::RevealerTransitionType::SlideDown)
        .child(&grid)
        .build();
    section.append(&revealer);

    show_all.connect_toggled(glib::clone!(
        #[weak]
        revealer,
        #[weak]
        preview,
        move |btn| {
            let expanded = btn.is_active();
            btn.set_label(if expanded { "\u{2212}" } else { "+" });
            btn.set_tooltip_text(Some(if expanded { "Collapse" } else { "Expand" }));
            revealer.set_reveal_child(expanded);
            preview.set_visible(!expanded);
        }
    ));

    section
}

/// Callbacks the vocabulary tab needs from the host application.
pub struct VocabTabCallbacks {
    /// `Some` (showing an "Add Word" button) when a dictionary is installed
    /// for the active language.
    pub add_word: Option<AddWordCallbacks>,
    /// Speak a word/reading aloud, from the detail dialog. `None` if no TTS
    /// backend is available for the active language.
    pub on_play: Option<Rc<dyn Fn(String)>>,
    /// Delete a vocabulary entry by id, from the detail dialog. Must call
    /// exactly one of the two callbacks once the (asynchronous) delete
    /// completes.
    pub delete: Rc<dyn Fn(i64, StdBox<dyn Fn()>, StdBox<dyn Fn()>)>,
    /// Look up example sentences for a word, from the detail dialog. `None`
    /// if no dictionary is loaded for the active language.
    pub example_lookup: Option<Rc<dyn Fn(&str) -> Vec<langspark_core::ExampleSentence>>>,
}

/// The vocabulary tab's root widget, plus its live `append` hook — exposed so
/// callers elsewhere in the app (currently `books::reader`'s "Add to
/// Vocabulary" popup) can push a newly-saved word into the *same* on-screen
/// list this tab renders from, rather than the word only appearing after a
/// restart. See `build_tab`'s own doc comment for why an in-tab add already
/// works without this.
pub struct VocabTab {
    pub widget: gtk4::Widget,
    /// Append a newly-persisted entry to the tab's live list and re-render.
    /// Idempotent to call from anywhere, any number of times — it only ever
    /// pushes and redraws, matching what `lookup::build`'s own `on_added`
    /// wiring already does for words added from this tab's "Add Word" button.
    pub append: Rc<dyn Fn(VocabularyEntry)>,
}

/// Build the vocabulary tab: a search box (plus an "Add Word" button when
/// `callbacks.add_word` is `Some`, i.e. a dictionary is installed for the
/// active language) followed by entries grouped by `level` (falling back to
/// "Uncategorized"), each its own section. All entries are loaded up front
/// (see `state::AppState`), so search/filter is done client-side by
/// rebuilding the sections on each keystroke. Words added via the dictionary
/// lookup dialog, or deleted via a card's detail dialog, are appended to/
/// dropped from the live list without a full reload — and the returned
/// `VocabTab::append` extends that same immediacy to words added from
/// outside this tab entirely (see `VocabTab`'s doc comment).
pub fn build_tab(entries: &[VocabularyEntry], callbacks: VocabTabCallbacks) -> VocabTab {
    let VocabTabCallbacks { add_word, on_play, delete, example_lookup } = callbacks;

    let root = Box::new(Orientation::Vertical, 12);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let search_row = Box::new(Orientation::Horizontal, 8);
    let search = gtk4::SearchEntry::builder().placeholder_text("Search word, reading, or meaning").hexpand(true).build();
    search_row.append(&search);
    root.append(&search_row);

    let sections_box = Box::new(Orientation::Vertical, 12);
    root.append(&sections_box);

    let entries_state: Rc<RefCell<Vec<VocabularyEntry>>> = Rc::new(RefCell::new(entries.to_vec()));
    let query_state: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    // Populated below, once `remove` (which itself needs `render`) exists —
    // `render` reads through this cell rather than capturing `CardCallbacks`
    // directly to break that cycle.
    let card_callbacks: Rc<RefCell<Option<CardCallbacks>>> = Rc::new(RefCell::new(None));

    let render = glib::clone!(
        #[weak]
        sections_box,
        #[strong]
        entries_state,
        #[strong]
        card_callbacks,
        move |query: &str| {
            while let Some(child) = sections_box.first_child() {
                sections_box.remove(&child);
            }

            let entries = entries_state.borrow();
            let filtered = filter_entries(&entries, query);
            if filtered.is_empty() {
                let message =
                    if query.is_empty() { "No vocabulary yet. Add words to get started." } else { "No matches." };
                sections_box.append(&Label::builder().label(message).css_classes(["dim-label"]).margin_top(24).build());
                return;
            }

            let by_level = group_by_level(&filtered);
            let card_callbacks = card_callbacks.borrow();
            let card_callbacks = card_callbacks.as_ref().expect("card_callbacks set before first render");
            for shelf in build_shelves(by_level) {
                sections_box.append(&build_shelf(&shelf, card_callbacks));
            }
        }
    );

    search.connect_search_changed(glib::clone!(
        #[strong]
        render,
        #[strong]
        query_state,
        move |entry| {
            let text = entry.text().to_string();
            *query_state.borrow_mut() = text.clone();
            render(&text);
        }
    ));

    let append: Rc<dyn Fn(VocabularyEntry)> = Rc::new(glib::clone!(
        #[strong]
        entries_state,
        #[strong]
        query_state,
        #[strong]
        render,
        move |new_entry: VocabularyEntry| {
            entries_state.borrow_mut().push(new_entry);
            render(&query_state.borrow());
        }
    ));

    let remove: Rc<dyn Fn(i64)> = Rc::new(glib::clone!(
        #[strong]
        entries_state,
        #[strong]
        query_state,
        #[strong]
        render,
        move |id: i64| {
            entries_state.borrow_mut().retain(|e| e.id != Some(id));
            render(&query_state.borrow());
        }
    ));
    *card_callbacks.borrow_mut() = Some(CardCallbacks { on_play, delete, remove, example_lookup });
    render("");

    if let Some(add_word) = add_word {
        let add_btn = gtk4::Button::builder().label("Add Word").valign(gtk4::Align::Center).build();
        search_row.append(&add_btn);

        add_btn.connect_clicked(glib::clone!(
            #[strong]
            add_word,
            #[strong]
            append,
            move |btn| {
                let dialog = lookup::build(add_word.clone(), append.clone());
                dialog.present(Some(btn));
            }
        ));
    }

    let scroller = ScrolledWindow::builder().child(&root).vexpand(true).build();
    VocabTab { widget: scroller.upcast(), append }
}

/// Entries whose word, reading, or meaning contains `query` (case-insensitive).
/// Pulled out for testability without a GTK display connection.
fn filter_entries<'a>(entries: &'a [VocabularyEntry], query: &str) -> Vec<&'a VocabularyEntry> {
    if query.is_empty() {
        return entries.iter().collect();
    }
    let query = query.to_lowercase();
    entries
        .iter()
        .filter(|e| {
            e.word.to_lowercase().contains(&query)
                || e.reading.as_deref().map(|r| r.to_lowercase().contains(&query)).unwrap_or(false)
                || e.meaning.to_lowercase().contains(&query)
        })
        .collect()
}

/// JLPT levels nested inside the "School" shelf (see `build_shelves`) —
/// exactly the levels `seed::seed_ja_school_vocabulary` pre-populates (the
/// standard N5-N3 school curriculum). Listed beginner-first, the order
/// they're displayed as "School"'s own sub-shelves.
const SCHOOL_LEVELS: [&str; 3] = ["N5", "N4", "N3"];

/// One vocabulary shelf as rendered in the tab: either a flat, single-level
/// group (every level except the JLPT school levels) or "School" itself,
/// nested one level deeper into its own N5/N4/N3 sub-shelves — see
/// `build_shelf`'s handling of each. Two levels deep (not a general tree)
/// because that's what "School" containing N3/N4/N5 actually needs; nothing
/// else in the data nests further.
enum Shelf<'a> {
    Flat(String, Vec<&'a VocabularyEntry>),
    Nested(String, Vec<(String, Vec<&'a VocabularyEntry>)>),
}

/// Group entries by proficiency level, falling back to "Uncategorized" —
/// pulled out of `build_shelves` so the raw bucketing is testable without a
/// GTK display connection.
fn group_by_level<'a>(entries: &[&'a VocabularyEntry]) -> BTreeMap<String, Vec<&'a VocabularyEntry>> {
    let mut by_level: BTreeMap<String, Vec<&VocabularyEntry>> = BTreeMap::new();
    for &entry in entries {
        let level = entry.level.clone().unwrap_or_else(|| "Uncategorized".to_string());
        by_level.entry(level).or_default().push(entry);
    }
    by_level
}

/// Turn `group_by_level`'s flat, alphabetically-keyed buckets into the
/// ordered list of shelves `build_tab` renders: `SCHOOL_LEVELS` collapsed
/// into one nested "School" shelf, then N2/N1, then anything else (other
/// languages' level tags, "Uncategorized") in `group_by_level`'s own
/// alphabetical order. Plain alphabetical order alone would put "School"
/// after "N1"/"N2" and leave N3/N4/N5 as three separate top-level sections,
/// backwards from a learner's actual progression.
fn build_shelves(by_level: BTreeMap<String, Vec<&VocabularyEntry>>) -> Vec<Shelf<'_>> {
    let mut by_level = by_level;
    let mut shelves = Vec::new();

    let school: Vec<(String, Vec<&VocabularyEntry>)> =
        SCHOOL_LEVELS.iter().filter_map(|&lvl| by_level.remove(lvl).map(|v| (lvl.to_string(), v))).collect();
    if !school.is_empty() {
        shelves.push(Shelf::Nested("School".to_string(), school));
    }

    for &lvl in &["N2", "N1"] {
        if let Some(v) = by_level.remove(lvl) {
            shelves.push(Shelf::Flat(lvl.to_string(), v));
        }
    }

    for (level, entries) in by_level {
        shelves.push(Shelf::Flat(level, entries));
    }

    shelves
}

/// Render one `Shelf`: a flat shelf is just `build_section`; "School" gets
/// its own outer header — with a disclosure toggle to open/close the whole
/// shelf, open by default — plus an indented `build_section` per JLPT
/// sub-level.
fn build_shelf(shelf: &Shelf, callbacks: &CardCallbacks) -> gtk4::Box {
    match shelf {
        Shelf::Flat(title, entries) => build_section(title, entries, callbacks, "langspark-section-header"),
        Shelf::Nested(title, subgroups) => {
            let outer = Box::new(Orientation::Vertical, 8);

            let header = Box::new(Orientation::Horizontal, 8);
            header.set_valign(gtk4::Align::Center);
            let title_label =
                Label::builder().label(title).css_classes(["langspark-section-header"]).xalign(0.0).hexpand(true).build();
            header.append(&title_label);

            let disclosure = gtk4::ToggleButton::builder()
                .active(true)
                .icon_name("pan-down-symbolic")
                .css_classes(["flat", "circular"])
                .tooltip_text("Collapse")
                .build();
            header.append(&disclosure);
            outer.append(&header);

            let sub_container = Box::new(Orientation::Vertical, 10);
            sub_container.set_margin_start(20);
            for (sub_title, sub_entries) in subgroups {
                sub_container.append(&build_section(sub_title, sub_entries, callbacks, "langspark-subsection-header"));
            }

            let revealer = Revealer::builder()
                .transition_type(gtk4::RevealerTransitionType::SlideDown)
                .reveal_child(true)
                .child(&sub_container)
                .build();
            outer.append(&revealer);

            disclosure.connect_toggled(glib::clone!(
                #[weak]
                revealer,
                move |btn| {
                    let expanded = btn.is_active();
                    revealer.set_reveal_child(expanded);
                    btn.set_icon_name(if expanded { "pan-down-symbolic" } else { "pan-end-symbolic" });
                    btn.set_tooltip_text(Some(if expanded { "Collapse" } else { "Expand" }));
                }
            ));

            outer
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(word: &str, level: &str) -> VocabularyEntry {
        VocabularyEntry {
            id: None,
            word: word.to_string(),
            reading: Some("reading".to_string()),
            meaning: "meaning".to_string(),
            language: "ja".to_string(),
            level: Some(level.to_string()),
            part_of_speech: None,
            tags: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn test_filter_entries_matches_word_reading_or_meaning() {
        let entries = vec![sample_entry("受け取る", "N4"), sample_entry("食べる", "N5")];

        assert_eq!(filter_entries(&entries, "").len(), 2);
        assert_eq!(filter_entries(&entries, "受け取る").len(), 1);
        assert_eq!(filter_entries(&entries, "MEANING").len(), 2); // case-insensitive, matches both
        assert_eq!(filter_entries(&entries, "nonexistent").len(), 0);
    }

    #[test]
    fn test_group_by_level_keeps_levels_distinct() {
        let entries =
            vec![sample_entry("受け取る", "N4"), sample_entry("食べる", "N5"), sample_entry("経済", "N3")];
        let refs: Vec<&VocabularyEntry> = entries.iter().collect();
        let grouped = group_by_level(&refs);
        // Raw bucketing keeps N3/N4/N5 apart — `build_shelves` is what nests
        // them into "School", not this.
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped["N4"][0].word, "受け取る");
    }

    #[test]
    fn test_group_by_level_uncategorized_fallback() {
        let mut entry = sample_entry("hola", "B1");
        entry.level = None;
        let entries = [entry];
        let refs: Vec<&VocabularyEntry> = entries.iter().collect();
        let grouped = group_by_level(&refs);
        assert!(grouped.contains_key("Uncategorized"));
    }

    fn shelf_titles<'a>(shelves: &'a [Shelf]) -> Vec<&'a str> {
        shelves
            .iter()
            .map(|s| match s {
                Shelf::Flat(title, _) => title.as_str(),
                Shelf::Nested(title, _) => title.as_str(),
            })
            .collect()
    }

    #[test]
    fn test_build_shelves_nests_n3_n4_n5_inside_school() {
        let entries =
            vec![sample_entry("受け取る", "N4"), sample_entry("食べる", "N5"), sample_entry("経済", "N3")];
        let refs: Vec<&VocabularyEntry> = entries.iter().collect();
        let shelves = build_shelves(group_by_level(&refs));

        assert_eq!(shelves.len(), 1); // one top-level shelf...
        match &shelves[0] {
            Shelf::Nested(title, subgroups) => {
                assert_eq!(title, "School");
                // ...containing three sub-shelves, beginner (N5) first.
                let sub_titles: Vec<&str> = subgroups.iter().map(|(t, _)| t.as_str()).collect();
                assert_eq!(sub_titles, vec!["N5", "N4", "N3"]);
                assert_eq!(subgroups[0].1[0].word, "食べる");
            }
            Shelf::Flat(..) => panic!("expected School to be a Nested shelf"),
        }
    }

    #[test]
    fn test_build_shelves_orders_school_before_n2_and_n1() {
        let entries =
            vec![sample_entry("憂鬱", "N1"), sample_entry("経済", "N2"), sample_entry("食べる", "N5")];
        let refs: Vec<&VocabularyEntry> = entries.iter().collect();
        let shelves = build_shelves(group_by_level(&refs));
        assert_eq!(shelf_titles(&shelves), vec!["School", "N2", "N1"]);
    }

    #[test]
    fn test_build_shelves_appends_uncategorized_after_known_levels() {
        let mut uncategorized = sample_entry("hola", "B1");
        uncategorized.level = None;
        let entries = vec![sample_entry("食べる", "N5"), uncategorized];
        let refs: Vec<&VocabularyEntry> = entries.iter().collect();
        let shelves = build_shelves(group_by_level(&refs));
        assert_eq!(shelf_titles(&shelves), vec!["School", "Uncategorized"]);
    }

    #[test]
    fn test_build_shelves_omits_school_when_no_school_levels_present() {
        let entries = vec![sample_entry("経済", "N2")];
        let refs: Vec<&VocabularyEntry> = entries.iter().collect();
        let shelves = build_shelves(group_by_level(&refs));
        assert_eq!(shelf_titles(&shelves), vec!["N2"]);
    }
}
