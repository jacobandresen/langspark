//! Dictionary lookup dialog: search the installed dictionary and add
//! results to your vocabulary.

use adw::prelude::*;
use gtk4::{Box as GtkBox, Orientation};
use langspark_core::VocabEntry;
use std::rc::Rc;

/// Callbacks wiring the lookup dialog to `AppState`.
#[derive(Clone)]
pub struct AddWordCallbacks {
    /// Search the loaded dictionary for `query` (word, reading, or meaning).
    pub search: Rc<dyn Fn(&str) -> Vec<VocabEntry>>,
    /// Persist a dictionary entry to the vocabulary database. Must call
    /// exactly one of `on_done` (with the saved entry, including its new id)
    /// or `on_error` once the (asynchronous) write completes.
    pub persist: Rc<dyn Fn(VocabEntry, Box<dyn Fn(langspark_core::VocabularyEntry)>, Box<dyn Fn()>)>,
}

/// Build the "Add Word" dialog. `on_added` is called (via `callbacks.persist`)
/// with each newly-saved entry so the caller can append it to the live list.
pub fn build(callbacks: AddWordCallbacks, on_added: Rc<dyn Fn(langspark_core::VocabularyEntry)>) -> adw::Dialog {
    let AddWordCallbacks { search, persist } = callbacks;

    let root = GtkBox::new(Orientation::Vertical, 8);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let entry = gtk4::SearchEntry::builder().placeholder_text("Search word, reading, or meaning").build();
    root.append(&entry);

    let results = gtk4::ListBox::builder().css_classes(["boxed-list"]).margin_top(8).build();
    let scroller = gtk4::ScrolledWindow::builder().child(&results).vexpand(true).build();
    root.append(&scroller);

    let render = glib::clone!(
        #[weak]
        results,
        #[strong]
        search,
        #[strong]
        persist,
        #[strong]
        on_added,
        move |query: &str| {
            while let Some(child) = results.first_child() {
                results.remove(&child);
            }
            if query.is_empty() {
                return;
            }
            let matches = search(query);
            if matches.is_empty() {
                results.append(&adw::ActionRow::builder().title("No matches").build());
                return;
            }
            for word in matches.into_iter().take(50) {
                let title = match &word.reading {
                    Some(r) if r != &word.word => format!("{}  ({r})", word.word),
                    _ => word.word.clone(),
                };
                let row = adw::ActionRow::builder().title(title).subtitle(word.meanings.join("; ")).build();

                let add_btn = gtk4::Button::builder().label("Add").valign(gtk4::Align::Center).build();
                row.add_suffix(&add_btn);

                let persist = persist.clone();
                let on_added = on_added.clone();
                add_btn.connect_clicked(move |btn| {
                    btn.set_sensitive(false);
                    btn.set_label("Adding\u{2026}");
                    let on_added = on_added.clone();
                    let done_btn = btn.clone();
                    let error_btn = btn.clone();
                    persist(
                        word.clone(),
                        Box::new(move |saved| {
                            done_btn.set_label("Added");
                            on_added(saved);
                        }),
                        Box::new(move || {
                            error_btn.set_label("Add");
                            error_btn.set_sensitive(true);
                        }),
                    );
                });

                results.append(&row);
            }
        }
    );

    entry.connect_search_changed(glib::clone!(
        #[strong]
        render,
        move |e| render(&e.text())
    ));

    // The search entry grabs focus and consumes Escape (to clear its text)
    // rather than letting it close the dialog, and a bare `.child()` dialog
    // has no header bar of its own — so without an explicit close button
    // there'd be no way to dismiss this dialog at all.
    // Explicit close button below, rather than relying on AdwHeaderBar's
    // automatic window-control buttons (whose behavior inside a Dialog isn't
    // consistent across libadwaita versions) — this guarantees exactly one.
    let header = adw::HeaderBar::builder().show_title(true).show_end_title_buttons(false).build();
    let toolbar_view = adw::ToolbarView::builder().content(&root).build();
    toolbar_view.add_top_bar(&header);

    let dialog = adw::Dialog::builder()
        .title("Add Word from Dictionary")
        .content_width(420)
        .content_height(520)
        .child(&toolbar_view)
        .build();

    let close_btn = gtk4::Button::builder().icon_name("window-close-symbolic").build();
    close_btn.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| {
            dialog.close();
        }
    ));
    header.pack_end(&close_btn);

    dialog
}
