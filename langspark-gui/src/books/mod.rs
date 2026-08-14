//! Books tab: browse the installed Aozora Bunko catalog grouped by genre,
//! open one to read, and (via the reader's word-click popup — see
//! `books::reader`/`books::popup`) look up and save words while reading.

pub mod popup;
pub mod reader;
pub mod sentence_popup;

use adw::prelude::*;
use gtk4::{Box, FlowBox, Label, Orientation, Revealer, ScrolledWindow};
use langspark_core::{BookCatalogEntry, BookText};
use reader::{BookReader, ReaderCallbacks};
use std::boxed::Box as StdBox;
use std::collections::BTreeMap;
use std::rc::Rc;

const UNCATEGORIZED_GENRE: &str = "Other";

/// Callbacks the Books tab needs from the host application.
pub struct BooksTabCallbacks {
    /// Fetch (network + on-disk cache — see `langspark_core::fetch_book`) a
    /// catalog entry's full text in the background, then hand it to the
    /// reader. Errors (e.g. no network) are reported via the second boxed
    /// closure rather than failing silently.
    pub open_book: Rc<dyn Fn(BookCatalogEntry, StdBox<dyn Fn(BookText)>, StdBox<dyn Fn(String)>)>,
    /// Everything an opened book's reader needs — see `reader::ReaderCallbacks`.
    pub reader: ReaderCallbacks,
}

/// A single catalog entry rendered as a clickable card. Clicking it fetches
/// and opens the book (see `BooksTabCallbacks::open_book`), pushing a reader
/// page onto `nav`.
fn build_card(entry: &BookCatalogEntry, callbacks: &Rc<BooksTabCallbacks>, nav: &adw::NavigationView) -> gtk4::Button {
    let content = Box::new(Orientation::Vertical, 3);
    content.set_size_request(160, -1);

    let title = Label::builder()
        .label(&entry.title)
        .css_classes(["title-4"])
        .xalign(0.0)
        .wrap(true)
        .lines(2)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .build();
    content.append(&title);

    let author = Label::builder().label(&entry.author).css_classes(["caption", "dim-label"]).xalign(0.0).build();
    content.append(&author);

    let card = gtk4::Button::builder().child(&content).css_classes(["card", "langspark-card"]).build();
    card.set_tooltip_text(Some(&entry.title));

    let entry = entry.clone();
    let callbacks = callbacks.clone();
    let nav = nav.clone();
    let author = author.clone();
    card.connect_clicked(move |btn| {
        // Same "Installing…" pattern `preferences::build_install_row` uses
        // for any other async fetch — a book's text isn't cached until it's
        // opened once (see `langspark_core::fetch_book`), so the first open
        // hits the network and is worth calling out rather than just
        // disabling the card with no explanation.
        btn.set_sensitive(false);
        author.set_label("Opening\u{2026}");
        let callbacks_for_done = callbacks.clone();
        let nav_for_done = nav.clone();
        let entry_for_done = entry.clone();
        let btn_for_error = btn.clone();
        let author_text = entry.author.clone();
        (callbacks.open_book)(
            entry.clone(),
            StdBox::new({
                let author = author.clone();
                let author_text = author_text.clone();
                move |book: BookText| {
                    open_reader_page(&nav_for_done, &entry_for_done, &book, callbacks_for_done.reader.clone());
                    btn_for_error.set_sensitive(true);
                    author.set_label(&author_text);
                }
            }),
            StdBox::new({
                let btn = btn.clone();
                let author = author.clone();
                move |_message: String| {
                    btn.set_sensitive(true);
                    author.set_label(&author_text);
                }
            }),
        );
    });

    card
}

/// Push a reader page for `book` onto `nav`, titled with the catalog entry's
/// title/author — `AdwNavigationView` supplies the back button automatically
/// via the page's `AdwHeaderBar`.
fn open_reader_page(nav: &adw::NavigationView, entry: &BookCatalogEntry, book: &BookText, reader_callbacks: ReaderCallbacks) {
    let reader = BookReader::new(book, reader_callbacks);

    let toolbar = adw::ToolbarView::builder().content(&reader.widget).build();
    toolbar.add_top_bar(&adw::HeaderBar::builder().title_widget(&adw::WindowTitle::new(&entry.title, &entry.author)).build());

    let page = adw::NavigationPage::new(&toolbar, &entry.title);
    nav.push(&page);
}

/// A labeled genre section showing a horizontal strip of its books with a
/// +/- toggle that expands to reveal the rest in a wrapping grid — mirrors
/// `vocabulary::build_section`.
fn build_section(genre: &str, entries: &[&BookCatalogEntry], callbacks: &Rc<BooksTabCallbacks>, nav: &adw::NavigationView) -> gtk4::Box {
    let section = Box::new(Orientation::Vertical, 6);

    let header = Box::new(Orientation::Horizontal, 8);
    header.set_valign(gtk4::Align::Center);
    let title = Label::builder().label(genre).css_classes(["langspark-section-header"]).xalign(0.0).hexpand(true).build();
    header.append(&title);

    let show_all = gtk4::ToggleButton::builder()
        .label("+")
        .css_classes(["circular", "flat", "langspark-expand-toggle"])
        .tooltip_text("Expand")
        .build();
    header.append(&show_all);
    section.append(&header);

    let strip = Box::new(Orientation::Horizontal, 10);
    strip.set_margin_top(2);
    for entry in entries.iter().take(6) {
        strip.append(&build_card(entry, callbacks, nav));
    }
    let strip_scroller = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .child(&strip)
        .build();
    section.append(&strip_scroller);

    let grid = FlowBox::builder().selection_mode(gtk4::SelectionMode::None).max_children_per_line(6).row_spacing(10).column_spacing(10).build();
    let revealer = Revealer::builder().transition_type(gtk4::RevealerTransitionType::SlideDown).child(&grid).build();
    section.append(&revealer);

    // The full grid is populated lazily, the first time this section is
    // actually expanded, rather than eagerly for every entry up front —
    // Aozora's full catalog runs to ~19k books, and building a card widget
    // for every one of them at tab-construction time (as the much smaller
    // Vocabulary tab's equivalent section does) measured out to ~1GB of
    // extra memory and several seconds of startup time. Most sections are
    // never expanded, so this keeps the common case cheap.
    let entries_owned: Vec<BookCatalogEntry> = entries.iter().map(|&e| e.clone()).collect();
    let populated = std::rc::Rc::new(std::cell::Cell::new(false));
    show_all.connect_toggled(glib::clone!(
        #[weak]
        revealer,
        #[weak]
        strip_scroller,
        #[weak]
        grid,
        #[strong]
        entries_owned,
        #[strong]
        callbacks,
        #[strong]
        nav,
        #[strong]
        populated,
        move |btn| {
            let expanded = btn.is_active();
            btn.set_label(if expanded { "\u{2212}" } else { "+" });
            btn.set_tooltip_text(Some(if expanded { "Collapse" } else { "Expand" }));
            if expanded && !populated.get() {
                for entry in &entries_owned {
                    grid.insert(&build_card(entry, &callbacks, &nav), -1);
                }
                populated.set(true);
            }
            revealer.set_reveal_child(expanded);
            strip_scroller.set_visible(!expanded);
        }
    ));

    section
}

/// Entries whose title or author contains `query` (case-insensitive).
/// Pulled out for testability without a GTK display connection.
fn filter_entries<'a>(entries: &'a [BookCatalogEntry], query: &str) -> Vec<&'a BookCatalogEntry> {
    if query.is_empty() {
        return entries.iter().collect();
    }
    let query = query.to_lowercase();
    entries.iter().filter(|e| e.title.to_lowercase().contains(&query) || e.author.to_lowercase().contains(&query)).collect()
}

/// Group entries by genre (falling back to [`UNCATEGORIZED_GENRE`]), pulled
/// out of `build_catalog_page` for testability without a GTK display
/// connection — mirrors `vocabulary::group_by_level`.
fn group_by_genre<'a>(entries: &[&'a BookCatalogEntry]) -> BTreeMap<String, Vec<&'a BookCatalogEntry>> {
    let mut by_genre: BTreeMap<String, Vec<&BookCatalogEntry>> = BTreeMap::new();
    for &entry in entries {
        let genre = entry.genre.clone().unwrap_or_else(|| UNCATEGORIZED_GENRE.to_string());
        by_genre.entry(genre).or_default().push(entry);
    }
    by_genre
}

/// Build the Books tab's root widget: an `AdwNavigationView` whose first
/// page is the catalog (search box + genre-grouped sections, mirroring
/// `vocabulary::build_tab`), pushing a reader page (see `open_reader_page`)
/// when a book is opened.
pub fn build_tab(catalog: &[BookCatalogEntry], callbacks: BooksTabCallbacks) -> gtk4::Widget {
    let callbacks = Rc::new(callbacks);
    let nav = adw::NavigationView::new();

    let root = Box::new(Orientation::Vertical, 12);
    root.set_margin_top(12);
    root.set_margin_bottom(12);
    root.set_margin_start(12);
    root.set_margin_end(12);

    let search = gtk4::SearchEntry::builder().placeholder_text("Search title or author").build();
    root.append(&search);

    let sections_box = Box::new(Orientation::Vertical, 12);
    root.append(&sections_box);

    let catalog = catalog.to_vec();
    let render = glib::clone!(
        #[weak]
        sections_box,
        #[strong]
        catalog,
        #[strong]
        callbacks,
        #[strong]
        nav,
        move |query: &str| {
            while let Some(child) = sections_box.first_child() {
                sections_box.remove(&child);
            }

            let filtered = filter_entries(&catalog, query);
            if filtered.is_empty() {
                let message = if query.is_empty() { "No books installed yet." } else { "No matches." };
                sections_box.append(&Label::builder().label(message).css_classes(["dim-label"]).margin_top(24).build());
                return;
            }

            for (genre, entries) in &group_by_genre(&filtered) {
                sections_box.append(&build_section(genre, entries, &callbacks, &nav));
            }
        }
    );

    search.connect_search_changed(glib::clone!(
        #[strong]
        render,
        move |entry| render(&entry.text())
    ));
    render("");

    let scroller = ScrolledWindow::builder().child(&root).vexpand(true).build();

    // No header bar of its own here, deliberately — unlike the reader page
    // below (which needs one: it's pushed on top of this one, so it's the
    // only place a back button and the open book's title/author can go).
    // This is the root page, shown alongside the *main* window's own header
    // (which already reads "Books" via the tab switcher) — giving it a
    // second header bar just to repeat that same title stacks two bars for
    // no reason, unlike every other tab.
    let catalog_page = adw::NavigationPage::new(&scroller, "Books");
    nav.push(&catalog_page);

    nav.upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry(title: &str, genre: Option<&str>) -> BookCatalogEntry {
        BookCatalogEntry {
            id: "1".to_string(),
            title: title.to_string(),
            author: "Test Author".to_string(),
            genre: genre.map(str::to_string),
            text_url: "https://example.com/book.zip".to_string(),
        }
    }

    #[test]
    fn test_filter_entries_matches_title_or_author_case_insensitive() {
        let entries = vec![sample_entry("Kokoro", None), sample_entry("Rashomon", None)];
        assert_eq!(filter_entries(&entries, "").len(), 2);
        assert_eq!(filter_entries(&entries, "koko").len(), 1);
        assert_eq!(filter_entries(&entries, "TEST AUTHOR").len(), 2); // matches author on both
        assert_eq!(filter_entries(&entries, "nonexistent").len(), 0);
    }

    #[test]
    fn test_group_by_genre() {
        let entries = vec![sample_entry("Kokoro", Some("Novels & Stories")), sample_entry("A Diary", Some("Diaries & Travel Writing"))];
        let refs: Vec<&BookCatalogEntry> = entries.iter().collect();
        let grouped = group_by_genre(&refs);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["Novels & Stories"][0].title, "Kokoro");
    }

    #[test]
    fn test_group_by_genre_uncategorized_fallback() {
        let entries = [sample_entry("Untitled", None)];
        let refs: Vec<&BookCatalogEntry> = entries.iter().collect();
        let grouped = group_by_genre(&refs);
        assert!(grouped.contains_key(UNCATEGORIZED_GENRE));
    }
}
