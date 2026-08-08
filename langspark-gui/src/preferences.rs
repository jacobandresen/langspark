//! Preferences dialog: language, dictionary location, TTS voices, SRS
//! algorithm/params, audio devices, cache cleanup, language install.

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
        .subtitle("Higher means faster-growing review intervals (SM-2 only; FSRS derives its own)")
        .adjustment(&gtk4::Adjustment::new(settings.borrow().starting_ease_factor, 1.3, 3.0, 0.1, 0.1, 0.0))
        .digits(1)
        .build();
    ease_row.connect_value_notify(glib::clone!(
        #[strong]
        settings,
        #[strong]
        on_save,
        move |row| {
            settings.borrow_mut().starting_ease_factor = row.value();
            on_save(&settings.borrow());
        }
    ));
    srs_group.add(&ease_row);
    study_page.add(&srs_group);

    let install_group = adw::PreferencesGroup::builder().title("Language Installation").build();
    if let Some(meta) = registry.get_metadata(langspark_core::Language::Japanese) {
        let dict_dir = settings
            .borrow()
            .dictionary_data_dir
            .clone()
            .or_else(|| crate::config::AppDirs::new().map(|d| d.dictionaries_dir()));
        let already_installed = dict_dir.as_ref().is_some_and(|d| d.join(format!("{}.json", meta.code)).exists());

        let row = adw::ActionRow::builder()
            .title(meta.display_name)
            .subtitle(if already_installed { "Installed" } else { "Not installed" })
            .build();

        let install_btn = gtk4::Button::builder()
            .label(if already_installed { "Reinstall" } else { "Install" })
            .valign(gtk4::Align::Center)
            .build();
        row.add_suffix(&install_btn);

        install_btn.connect_clicked(glib::clone!(
            #[weak]
            row,
            #[weak]
            install_btn,
            move |_| {
                let Some(dict_dir) = dict_dir.clone() else {
                    row.set_subtitle("Couldn't determine dictionary data directory");
                    return;
                };
                install_btn.set_sensitive(false);
                row.set_subtitle("Installing\u{2026}");

                crate::task::spawn_on_main(glib::clone!(
                    #[weak]
                    row,
                    #[weak]
                    install_btn,
                    async move {
                        let jmdict_dest = dict_dir.join("ja.json");
                        let kanjidic_dest = dict_dir.join("kanjidic.json");

                        let result = crate::task::run_blocking(move || {
                            langspark_core::install_jmdict(&jmdict_dest, &|_, _| {})
                                .and_then(|v| langspark_core::install_kanjidic(&kanjidic_dest, &|_, _| {}).map(|_| v))
                        })
                        .await;

                        match result {
                            Ok(version) => {
                                row.set_subtitle(&format!("Installed (JMdict {version})"));
                                install_btn.set_label("Reinstall");
                            }
                            Err(e) => {
                                row.set_subtitle(&format!("Install failed: {e}"));
                            }
                        }
                        install_btn.set_sensitive(true);
                    }
                ));
            }
        ));

        install_group.add(&row);

        // Supplemental example sentences (Tatoeba corpus) — separate from the
        // dictionary install above since it's a much bigger download (~150MB
        // vs ~50MB) that most words don't strictly need (JMdict's own
        // smaller curated example subset already covers ~85% of common
        // vocabulary — see `installer::install_tatoeba_examples`).
        let tatoeba_dict_dir = settings
            .borrow()
            .dictionary_data_dir
            .clone()
            .or_else(|| crate::config::AppDirs::new().map(|d| d.dictionaries_dir()));
        let tatoeba_already_installed =
            tatoeba_dict_dir.as_ref().is_some_and(|d| d.join(format!("tatoeba_{}.tsv", meta.code)).exists());

        let tatoeba_row = adw::ActionRow::builder()
            .title("Example sentences")
            .subtitle(if tatoeba_already_installed { "Installed" } else { "Not installed (~150MB download)" })
            .build();
        let tatoeba_install_btn = gtk4::Button::builder()
            .label(if tatoeba_already_installed { "Reinstall" } else { "Install" })
            .valign(gtk4::Align::Center)
            .build();
        tatoeba_row.add_suffix(&tatoeba_install_btn);

        let tatoeba_code = meta.code;
        tatoeba_install_btn.connect_clicked(glib::clone!(
            #[weak]
            tatoeba_row,
            #[weak]
            tatoeba_install_btn,
            move |_| {
                let Some(dict_dir) = tatoeba_dict_dir.clone() else {
                    tatoeba_row.set_subtitle("Couldn't determine dictionary data directory");
                    return;
                };
                tatoeba_install_btn.set_sensitive(false);
                tatoeba_row.set_subtitle("Installing\u{2026}");

                crate::task::spawn_on_main(glib::clone!(
                    #[weak]
                    tatoeba_row,
                    #[weak]
                    tatoeba_install_btn,
                    async move {
                        let dest = dict_dir.join(format!("tatoeba_{tatoeba_code}.tsv"));
                        let result =
                            crate::task::run_blocking(move || langspark_core::install_tatoeba_examples(&dest, &|_, _| {}))
                                .await;

                        match result {
                            Ok(count) => {
                                tatoeba_row.set_subtitle(&format!("Installed ({count} sentence pairs)"));
                                tatoeba_install_btn.set_label("Reinstall");
                            }
                            Err(e) => {
                                tatoeba_row.set_subtitle(&format!("Install failed: {e}"));
                            }
                        }
                        tatoeba_install_btn.set_sensitive(true);
                    }
                ));
            }
        ));

        install_group.add(&tatoeba_row);
    }
    study_page.add(&install_group);

    dialog.add(&study_page);

    dialog
}

// Widget construction is exercised by the consolidated smoke test in
// `main.rs` (`gtk_smoke` module); see vocabulary::dialog::tests for why.
