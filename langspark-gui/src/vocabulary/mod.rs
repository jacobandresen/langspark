//! Vocabulary tab: browse vocabulary entries grouped into sections (JLPT
//! level for Japanese, CEFR level for Spanish), each shown as a horizontal
//! strip of cards with a "Show All" button that expands to a full grid.

pub mod dialog;
pub mod lookup;

pub use lookup::AddWordCallbacks;

use adw::prelude::*;
use gtk4::{Box, FlowBox, Label, Orientation, Revealer, ScrolledWindow};
use langspark_core::VocabularyEntry;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A single vocabulary entry rendered as a clickable card.
pub fn build_card(entry: &VocabularyEntry) -> gtk4::Button {
    let content = Box::new(Orientation::Vertical, 4);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let word = Label::builder().label(&entry.word).css_classes(["title-4"]).xalign(0.0).build();
    content.append(&word);

    if let Some(reading) = &entry.reading {
        let reading_label = Label::builder().label(reading).css_classes(["caption"]).xalign(0.0).build();
        content.append(&reading_label);
    }

    let meaning = Label::builder()
        .label(&entry.meaning)
        .xalign(0.0)
        .wrap(true)
        .max_width_chars(24)
        .build();
    content.append(&meaning);

    let card = gtk4::Button::builder().child(&content).css_classes(["card", "langspark-card"]).build();
    card.set_tooltip_text(Some(&entry.meaning));
    card
}

/// A labeled section (e.g. "N4") showing a horizontal strip of its entries
/// with a "Show All" toggle that reveals the rest in a wrapping grid.
fn build_section(level: &str, entries: &[&VocabularyEntry]) -> gtk4::Box {
    let section = Box::new(Orientation::Vertical, 4);

    let header = Box::new(Orientation::Horizontal, 8);
    let title = Label::builder()
        .label(level)
        .css_classes(["langspark-section-header"])
        .xalign(0.0)
        .hexpand(true)
        .build();
    header.append(&title);

    let show_all = gtk4::ToggleButton::builder().label("Show All").build();
    header.append(&show_all);
    section.append(&header);

    // Horizontal strip: first few entries in a row, always visible.
    let strip = Box::new(Orientation::Horizontal, 8);
    strip.set_margin_top(4);
    for entry in entries.iter().take(6) {
        strip.append(&build_card(entry));
    }
    let strip_scroller = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .child(&strip)
        .build();
    section.append(&strip_scroller);

    // Full grid, revealed by "Show All".
    let grid = FlowBox::builder().selection_mode(gtk4::SelectionMode::None).max_children_per_line(6).build();
    for entry in entries {
        grid.insert(&build_card(entry), -1);
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
        strip_scroller,
        move |btn| {
            revealer.set_reveal_child(btn.is_active());
            strip_scroller.set_visible(!btn.is_active());
        }
    ));

    section
}

/// Build the vocabulary tab's root widget: a search box (plus an "Add Word"
/// button when `add_word` is `Some`, i.e. a dictionary is installed for the
/// active language) followed by entries grouped by `level` (falling back to
/// "Uncategorized"), each its own section. All entries are loaded up front
/// (see `state::AppState`), so search/filter is done client-side by
/// rebuilding the sections on each keystroke. Words added via the dictionary
/// lookup dialog are appended to the live list without a full rebuild.
pub fn build_tab(entries: &[VocabularyEntry], add_word: Option<AddWordCallbacks>) -> gtk4::Widget {
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

    let render = glib::clone!(
        #[weak]
        sections_box,
        #[strong]
        entries_state,
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

            let mut by_level: BTreeMap<String, Vec<&VocabularyEntry>> = BTreeMap::new();
            for entry in filtered {
                let level = entry.level.clone().unwrap_or_else(|| "Uncategorized".to_string());
                by_level.entry(level).or_default().push(entry);
            }
            for (level, level_entries) in &by_level {
                sections_box.append(&build_section(level, level_entries));
            }
        }
    );
    render("");

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
    scroller.upcast()
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

/// Group entries by proficiency level (falling back to "Uncategorized"),
/// pulled out of `build_tab` so the grouping logic is testable without a
/// GTK display connection.
fn group_by_level(entries: &[VocabularyEntry]) -> BTreeMap<String, Vec<&VocabularyEntry>> {
    let mut by_level: BTreeMap<String, Vec<&VocabularyEntry>> = BTreeMap::new();
    for entry in entries {
        let level = entry.level.clone().unwrap_or_else(|| "Uncategorized".to_string());
        by_level.entry(level).or_default().push(entry);
    }
    by_level
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
    fn test_group_by_level() {
        let entries = vec![sample_entry("受け取る", "N4"), sample_entry("食べる", "N5")];
        let grouped = group_by_level(&entries);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["N4"][0].word, "受け取る");
    }

    #[test]
    fn test_group_by_level_uncategorized_fallback() {
        let mut entry = sample_entry("hola", "B1");
        entry.level = None;
        let entries = [entry];
        let grouped = group_by_level(&entries);
        assert!(grouped.contains_key("Uncategorized"));
    }
}
