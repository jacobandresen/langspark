//! Preferences dialog: language, dictionary location, TTS voices, SRS
//! algorithm/params, UI theme, audio devices, cache cleanup, language install.

use crate::config::Settings;
use adw::prelude::*;
use langspark_core::LanguageRegistry;
use std::cell::RefCell;
use std::rc::Rc;

/// Build the preferences dialog. `settings` is shared with the caller so
/// changes made here (language, theme, ...) are visible immediately;
/// `on_save` is invoked with the updated settings whenever a field changes.
pub fn build(settings: Rc<RefCell<Settings>>, on_save: impl Fn(&Settings) + 'static) -> adw::PreferencesDialog {
    let dialog = adw::PreferencesDialog::builder().title("Preferences").build();
    let on_save = Rc::new(on_save);

    // --- General page: language, theme, dictionary location ---
    let general_page = adw::PreferencesPage::builder().title("General").icon_name("preferences-system-symbolic").build();

    let language_group = adw::PreferencesGroup::builder().title("Language").build();
    let registry = LanguageRegistry::new();
    let language_model = gtk4::StringList::new(
        &registry
            .get_available_languages()
            .iter()
            .map(|l| registry.get_metadata(*l).map(|m| m.display_name).unwrap_or("Unknown"))
            .collect::<Vec<_>>(),
    );
    let language_row = adw::ComboRow::builder().title("Active language").model(&language_model).build();
    let languages = registry.get_available_languages();
    if let Some(pos) = languages.iter().position(|l| l.code() == settings.borrow().active_language) {
        language_row.set_selected(pos as u32);
    }
    language_row.connect_selected_notify(glib::clone!(
        #[strong]
        settings,
        #[strong]
        on_save,
        #[strong]
        languages,
        move |row| {
            if let Some(lang) = languages.get(row.selected() as usize) {
                settings.borrow_mut().active_language = lang.code().to_string();
                on_save(&settings.borrow());
            }
        }
    ));
    language_group.add(&language_row);
    general_page.add(&language_group);

    let theme_group = adw::PreferencesGroup::builder().title("Appearance").build();
    let theme_model = gtk4::StringList::new(&["System", "Light", "Dark"]);
    let theme_row = adw::ComboRow::builder().title("Theme").model(&theme_model).build();
    theme_row.set_selected(match settings.borrow().ui_theme.as_str() {
        "light" => 1,
        "dark" => 2,
        _ => 0,
    });
    theme_row.connect_selected_notify(glib::clone!(
        #[strong]
        settings,
        #[strong]
        on_save,
        move |row| {
            let theme = match row.selected() {
                1 => "light",
                2 => "dark",
                _ => "system",
            };
            settings.borrow_mut().ui_theme = theme.to_string();
            apply_theme(theme);
            on_save(&settings.borrow());
        }
    ));
    theme_group.add(&theme_row);
    general_page.add(&theme_group);

    let dict_group = adw::PreferencesGroup::builder().title("Dictionary Data").build();
    let dict_row = adw::ActionRow::builder()
        .title("Data location")
        .subtitle(
            settings
                .borrow()
                .dictionary_data_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Default".to_string()),
        )
        .build();
    dict_group.add(&dict_row);
    general_page.add(&dict_group);

    dialog.add(&general_page);

    // --- Audio page: TTS voices, audio devices, cache ---
    let audio_page = adw::PreferencesPage::builder().title("Audio").icon_name("audio-speakers-symbolic").build();

    let voice_group = adw::PreferencesGroup::builder().title("Text-to-Speech Voices").build();
    let ja_voice_row = adw::EntryRow::builder().title("Japanese voice (VOICEVOX speaker)").build();
    ja_voice_row.set_text(&settings.borrow().tts_voice_ja);
    ja_voice_row.connect_changed(glib::clone!(
        #[strong]
        settings,
        #[strong]
        on_save,
        move |row| {
            settings.borrow_mut().tts_voice_ja = row.text().to_string();
            on_save(&settings.borrow());
        }
    ));
    voice_group.add(&ja_voice_row);

    let es_voice_row = adw::EntryRow::builder().title("Spanish voice (Piper model)").build();
    es_voice_row.set_text(&settings.borrow().tts_voice_es);
    es_voice_row.connect_changed(glib::clone!(
        #[strong]
        settings,
        #[strong]
        on_save,
        move |row| {
            settings.borrow_mut().tts_voice_es = row.text().to_string();
            on_save(&settings.borrow());
        }
    ));
    voice_group.add(&es_voice_row);
    audio_page.add(&voice_group);

    let device_group = adw::PreferencesGroup::builder().title("Audio Devices").build();
    let (input_names, output_names) = langspark_core::list_audio_devices();

    let input_row = if input_names.is_empty() {
        adw::ActionRow::builder().title("Microphone").subtitle("Not detected").build().upcast::<gtk4::Widget>()
    } else {
        let model = gtk4::StringList::new(&input_names.iter().map(String::as_str).collect::<Vec<_>>());
        let row = adw::ComboRow::builder().title("Microphone").model(&model).build();
        if let Some(selected) = &settings.borrow().audio_input_device {
            if let Some(pos) = input_names.iter().position(|n| n == selected) {
                row.set_selected(pos as u32);
            }
        }
        row.connect_selected_notify(glib::clone!(
            #[strong]
            settings,
            #[strong]
            on_save,
            #[strong]
            input_names,
            move |row| {
                settings.borrow_mut().audio_input_device = input_names.get(row.selected() as usize).cloned();
                on_save(&settings.borrow());
            }
        ));
        row.upcast()
    };
    device_group.add(&input_row);

    let output_row = if output_names.is_empty() {
        adw::ActionRow::builder().title("Speakers").subtitle("Not detected").build().upcast::<gtk4::Widget>()
    } else {
        let model = gtk4::StringList::new(&output_names.iter().map(String::as_str).collect::<Vec<_>>());
        let row = adw::ComboRow::builder().title("Speakers").model(&model).build();
        if let Some(selected) = &settings.borrow().audio_output_device {
            if let Some(pos) = output_names.iter().position(|n| n == selected) {
                row.set_selected(pos as u32);
            }
        }
        row.connect_selected_notify(glib::clone!(
            #[strong]
            settings,
            #[strong]
            on_save,
            #[strong]
            output_names,
            move |row| {
                settings.borrow_mut().audio_output_device = output_names.get(row.selected() as usize).cloned();
                on_save(&settings.borrow());
            }
        ));
        row.upcast()
    };
    device_group.add(&output_row);
    audio_page.add(&device_group);

    let cache_group = adw::PreferencesGroup::builder().title("Cache").build();
    let cache_row = adw::ActionRow::builder().title("Cached pronunciation audio").build();
    let clear_cache_btn = gtk4::Button::builder().label("Clear Cache").valign(gtk4::Align::Center).build();
    cache_row.add_suffix(&clear_cache_btn);
    cache_group.add(&cache_row);
    audio_page.add(&cache_group);

    clear_cache_btn.connect_clicked(glib::clone!(
        #[weak]
        cache_row,
        move |_| {
            if let Some(dirs) = crate::config::AppDirs::new() {
                let cache = langspark_core::AudioCache::new(dirs.audio_cache_dir());
                match cache.clear() {
                    Ok(removed) => cache_row.set_subtitle(&format!("Cleared {removed} file(s)")),
                    Err(e) => cache_row.set_subtitle(&format!("Failed to clear cache: {e}")),
                }
            }
        }
    ));

    dialog.add(&audio_page);

    // --- Study page: SRS algorithm/params, language install management ---
    let study_page = adw::PreferencesPage::builder().title("Study").icon_name("view-list-symbolic").build();

    let srs_group = adw::PreferencesGroup::builder().title("Spaced Repetition").build();
    let algo_model = gtk4::StringList::new(&["SM-2", "FSRS"]);
    let algo_row = adw::ComboRow::builder().title("Algorithm").model(&algo_model).build();
    algo_row.set_selected(if settings.borrow().srs_algorithm == "fsrs" { 1 } else { 0 });
    algo_row.connect_selected_notify(glib::clone!(
        #[strong]
        settings,
        #[strong]
        on_save,
        move |row| {
            settings.borrow_mut().srs_algorithm = if row.selected() == 1 { "fsrs" } else { "sm2" }.to_string();
            on_save(&settings.borrow());
        }
    ));
    srs_group.add(&algo_row);

    let ease_row = adw::SpinRow::builder()
        .title("Starting ease factor")
        .subtitle("Higher means faster-growing review intervals")
        .adjustment(&gtk4::Adjustment::new(2.5, 1.3, 3.0, 0.1, 0.1, 0.0))
        .digits(1)
        .build();
    srs_group.add(&ease_row);
    study_page.add(&srs_group);

    let install_group = adw::PreferencesGroup::builder().title("Language Installation").build();
    for lang in registry.get_available_languages() {
        if let Some(meta) = registry.get_metadata(lang) {
            let row = adw::ActionRow::builder().title(meta.display_name).subtitle("Not installed").build();
            let install_btn = gtk4::Button::builder().label("Install").valign(gtk4::Align::Center).build();
            row.add_suffix(&install_btn);
            install_group.add(&row);
        }
    }
    study_page.add(&install_group);

    dialog.add(&study_page);

    dialog
}

fn apply_theme(theme: &str) {
    let style_manager = adw::StyleManager::default();
    style_manager.set_color_scheme(match theme {
        "light" => adw::ColorScheme::ForceLight,
        "dark" => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    });
}

// Widget construction is exercised by the consolidated smoke test in
// `main.rs` (`gtk_smoke` module); see vocabulary::dialog::tests for why.
