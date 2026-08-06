//! Language selection dropdown widget.
//!
//! Presents the languages available in a `LanguageRegistry` and reports
//! selection changes back to the caller so it can update a `LanguageManager`.

use gtk4::prelude::*;
use gtk4::{DropDown, StringList};
use langspark_core::{Language, LanguageRegistry};

/// A dropdown for picking the active language, backed by the core `LanguageRegistry`.
pub struct LanguageSelector {
    dropdown: DropDown,
    languages: Vec<Language>,
}

impl LanguageSelector {
    /// Build a new selector, defaulting to `active`.
    pub fn new(registry: &LanguageRegistry, active: Language) -> Self {
        let languages = registry.get_available_languages();

        let labels: Vec<String> = languages
            .iter()
            .map(|lang| {
                let meta = registry.get_metadata(*lang);
                match meta {
                    Some(m) => format!("{} {}", m.flag_emoji, m.display_name),
                    None => lang.to_string(),
                }
            })
            .collect();
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();

        let model = StringList::new(&label_refs);
        let dropdown = DropDown::builder().model(&model).build();

        let selected_index = languages.iter().position(|l| *l == active).unwrap_or(0);
        dropdown.set_selected(selected_index as u32);

        Self { dropdown, languages }
    }

    /// The underlying GTK widget, for embedding in a container.
    pub fn widget(&self) -> &DropDown {
        &self.dropdown
    }

    /// Currently selected language.
    pub fn selected_language(&self) -> Language {
        let index = self.dropdown.selected() as usize;
        self.languages.get(index).copied().unwrap_or(Language::Japanese)
    }

    /// Register a callback invoked whenever the selected language changes.
    pub fn connect_changed<F>(&self, callback: F)
    where
        F: Fn(Language) + 'static,
    {
        let languages = self.languages.clone();
        self.dropdown.connect_selected_notify(move |dropdown| {
            let index = dropdown.selected() as usize;
            if let Some(lang) = languages.get(index) {
                callback(*lang);
            }
        });
    }
}
