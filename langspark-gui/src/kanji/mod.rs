//! Kanji tab (Japanese-only): browse kanji grouped into sections by JLPT
//! level, with the same horizontal-strip + "Show All" pattern as the
//! vocabulary tab.

pub mod dialog;

use gtk4::prelude::*;
use gtk4::{Box, FlowBox, Label, Orientation, Revealer, ScrolledWindow};
use langspark_core::KanjiEntry;
use std::collections::BTreeMap;

/// A single kanji rendered as a clickable card: large character, readings, meanings.
pub fn build_card(entry: &KanjiEntry) -> gtk4::Button {
    let content = Box::new(Orientation::Vertical, 4);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let character = Label::builder().label(&entry.character).css_classes(["title-1"]).build();
    content.append(&character);

    if let Some(on) = &entry.on_readings {
        if !on.is_empty() {
            content.append(&Label::builder().label(format!("音: {on}")).css_classes(["caption"]).build());
        }
    }
    if let Some(kun) = &entry.kun_readings {
        if !kun.is_empty() {
            content.append(&Label::builder().label(format!("訓: {kun}")).css_classes(["caption"]).build());
        }
    }

    let meanings = Label::builder().label(&entry.meanings).wrap(true).max_width_chars(20).build();
    content.append(&meanings);

    gtk4::Button::builder().child(&content).css_classes(["card", "langspark-card"]).build()
}

fn build_section(level: &str, entries: &[&KanjiEntry]) -> gtk4::Box {
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

    let strip = Box::new(Orientation::Horizontal, 8);
    strip.set_margin_top(4);
    for entry in entries.iter().take(8) {
        strip.append(&build_card(entry));
    }
    let strip_scroller = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .child(&strip)
        .build();
    section.append(&strip_scroller);

    let grid = FlowBox::builder().selection_mode(gtk4::SelectionMode::None).max_children_per_line(8).build();
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

/// Build the kanji tab's root widget, grouped by JLPT level.
pub fn build_tab(entries: &[KanjiEntry]) -> gtk4::Widget {
    let root = Box::new(Orientation::Vertical, 12);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let by_level = group_by_jlpt_level(entries);

    if by_level.is_empty() {
        root.append(&Label::builder().label("No kanji yet.").css_classes(["dim-label"]).margin_top(24).build());
    }

    for (level, level_entries) in &by_level {
        root.append(&build_section(level, level_entries));
    }

    let scroller = ScrolledWindow::builder().child(&root).vexpand(true).build();
    scroller.upcast()
}

/// Group kanji by JLPT level label (e.g. "N3"), with ungraded kanji last.
/// Pulled out for testability without a GTK display connection.
fn group_by_jlpt_level(entries: &[KanjiEntry]) -> BTreeMap<String, Vec<&KanjiEntry>> {
    let mut by_level: BTreeMap<String, Vec<&KanjiEntry>> = BTreeMap::new();
    for entry in entries {
        let level = match entry.jlpt_level {
            Some(n) => format!("N{n}"),
            None => "Ungraded".to_string(),
        };
        by_level.entry(level).or_default().push(entry);
    }
    by_level
}

/// Whether the kanji tab should be shown for the active language (Japanese only).
pub fn is_visible_for(language: langspark_core::Language) -> bool {
    language == langspark_core::Language::Japanese
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(character: &str, jlpt: Option<i32>) -> KanjiEntry {
        KanjiEntry {
            id: None,
            character: character.to_string(),
            on_readings: Some("on".to_string()),
            kun_readings: Some("kun".to_string()),
            meanings: "meaning".to_string(),
            stroke_count: Some(8),
            radical: None,
            jlpt_level: jlpt,
            grade: None,
            language: "ja".to_string(),
            created_at: None,
        }
    }

    #[test]
    fn test_group_by_jlpt_level() {
        let entries = vec![sample_entry("受", Some(3)), sample_entry("食", Some(5)), sample_entry("鬱", None)];
        let grouped = group_by_jlpt_level(&entries);
        assert_eq!(grouped.len(), 3);
        assert!(grouped.contains_key("N3"));
        assert!(grouped.contains_key("Ungraded"));
    }

    #[test]
    fn test_kanji_tab_visibility() {
        assert!(is_visible_for(langspark_core::Language::Japanese));
        assert!(!is_visible_for(langspark_core::Language::Spanish));
    }
}
