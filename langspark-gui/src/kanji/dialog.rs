//! Kanji detail dialog: large character, all readings, all meanings, stroke
//! count/radical info.

use adw::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};
use langspark_core::KanjiEntry;

pub fn build(entry: &KanjiEntry) -> adw::Dialog {
    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let character = Label::builder().label(&entry.character).css_classes(["title-1"]).build();
    character.set_margin_top(8);
    character.set_margin_bottom(8);
    // Render large, per task 17.10 ("large rendering")
    let attrs = gtk4::pango::AttrList::new();
    let mut font_desc = gtk4::pango::FontDescription::new();
    font_desc.set_size(48 * gtk4::pango::SCALE);
    attrs.insert(gtk4::pango::AttrFontDesc::new(&font_desc));
    character.set_attributes(Some(&attrs));
    root.append(&character);

    let list = gtk4::ListBox::builder().css_classes(["boxed-list"]).build();
    if let Some(on) = &entry.on_readings {
        if !on.is_empty() {
            list.append(&adw::ActionRow::builder().title("On readings (音読み)").subtitle(on).build());
        }
    }
    if let Some(kun) = &entry.kun_readings {
        if !kun.is_empty() {
            list.append(&adw::ActionRow::builder().title("Kun readings (訓読み)").subtitle(kun).build());
        }
    }
    list.append(&adw::ActionRow::builder().title("Meanings").subtitle(&entry.meanings).build());
    if let Some(strokes) = entry.stroke_count {
        list.append(&adw::ActionRow::builder().title("Stroke count").subtitle(strokes.to_string()).build());
    }
    if let Some(radical) = &entry.radical {
        list.append(&adw::ActionRow::builder().title("Radical").subtitle(radical).build());
    }
    if let Some(jlpt) = entry.jlpt_level {
        list.append(&adw::ActionRow::builder().title("JLPT level").subtitle(format!("N{jlpt}")).build());
    }
    root.append(&list);

    adw::Dialog::builder().title(&entry.character).content_width(360).content_height(480).child(&root).build()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn sample_entry() -> KanjiEntry {
        KanjiEntry {
            id: Some(1),
            character: "受".to_string(),
            on_readings: Some("ジュ".to_string()),
            kun_readings: Some("う.ける".to_string()),
            meanings: "receive; accept".to_string(),
            stroke_count: Some(8),
            radical: Some("又".to_string()),
            jlpt_level: Some(3),
            grade: Some(3),
            language: "ja".to_string(),
            created_at: None,
        }
    }

    // See vocabulary::dialog::tests for why widget construction isn't tested here.
}
