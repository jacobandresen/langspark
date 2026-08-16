//! Preferences dialog: language, dictionary location, TTS voices, SRS
//! algorithm/params, audio devices, cache cleanup, language install.

use crate::config::Settings;
use adw::prelude::*;
use anyhow::Context;
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

    let books_group = adw::PreferencesGroup::builder().title("Books Data").build();
    let books_row = adw::ActionRow::builder()
        .title("Data location")
        .subtitle(
            settings
                .borrow()
                .books_data_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Default".to_string()),
        )
        .build();
    books_group.add(&books_row);
    general_page.add(&books_group);

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
        install_group.add(&build_install_row(meta.display_name, already_installed, "Not installed", {
            let dict_dir = dict_dir.clone();
            move || {
                let dict_dir = dict_dir.clone().context("couldn't determine dictionary data directory")?;
                let jmdict_dest = dict_dir.join("ja.json");
                let kanjidic_dest = dict_dir.join("kanjidic.json");
                let version = langspark_core::install_jmdict(&jmdict_dest, &|_, _| {})?;
                langspark_core::install_kanjidic(&kanjidic_dest, &|_, _| {})?;
                Ok(format!("Installed (JMdict {version})"))
            }
        }));

        // Supplemental example sentences (Tatoeba corpus) — separate from the
        // dictionary install above since it's a much bigger download (~150MB
        // vs ~50MB) that most words don't strictly need (JMdict's own
        // smaller curated example subset already covers ~85% of common
        // vocabulary — see `installer::install_tatoeba_examples`).
        let code = meta.code;
        let already_installed = dict_dir.as_ref().is_some_and(|d| d.join(format!("tatoeba_{code}.tsv")).exists());
        install_group.add(&build_install_row(
            "Example sentences",
            already_installed,
            "Not installed (~150MB download)",
            move || {
                let dest = dict_dir
                    .clone()
                    .context("couldn't determine dictionary data directory")?
                    .join(format!("tatoeba_{code}.tsv"));
                let count = langspark_core::install_tatoeba_examples(&dest, &|_, _| {})?;
                Ok(format!("Installed ({count} sentence pairs)"))
            },
        ));

        // VOICEVOX Engine (Japanese TTS) — a native (Docker-free) build only
        // exists for Linux x86_64/aarch64 and Windows x86_64 (see
        // installer::voicevox_platform); `install_voicevox_engine` reports a
        // clear error on other platforms, pointing at
        // scripts/setup-voicevox.sh's Docker path instead.
        let voicevox_dir = crate::config::AppDirs::new().map(|d| d.voicevox_engine_dir());
        let already_installed = voicevox_dir
            .as_ref()
            .is_some_and(|d| d.join(langspark_core::voicevox_run_executable_name()).exists());
        install_group.add(&build_install_row(
            "VOICEVOX Engine (Japanese TTS)",
            already_installed,
            "Not installed (~2GB download)",
            move || {
                let dir = voicevox_dir.clone().context("couldn't determine the VOICEVOX Engine directory")?;
                let version = langspark_core::install_voicevox_engine(&dir, &|_, _| {})?;
                Ok(format!("Installed ({version}) — restart LangSpark to start it"))
            },
        ));

        // Speech recognition model (see app.rs's `asr_model_installed`, which
        // gates whether the Pronunciation tab shows at all). Only present in
        // builds with the `asr` Cargo feature: `qwen3-asr-rs` dynamically
        // links libtorch as a hard *runtime* dependency (confirmed via
        // `ldd`/`readelf` on a built binary — a plain `NEEDED
        // libtorch_cpu.so` entry with no baked-in rpath, so the dynamic
        // linker has to find it via `LD_LIBRARY_PATH` or a system lib dir;
        // only `scripts/install.sh`'s from-source path bakes an rpath via
        // `RUSTFLAGS`). Official release builds are `--no-default-features`
        // (no libtorch bundled — see `.github/workflows/release-builds.yml`),
        // so without this `#[cfg]` a user could install a 1.5GB model here
        // only to have every transcription attempt fail with "unavailable"
        // once the Pronunciation tab appeared.
        #[cfg(feature = "asr")]
        {
            let asr_dir = crate::config::AppDirs::new().map(|d| d.asr_model_dir(meta.code));
            let already_installed = asr_dir.as_ref().is_some_and(|d| d.join("tokenizer.json").exists());
            install_group.add(&build_install_row(
                "Speech recognition model (Qwen3-ASR)",
                already_installed,
                "Not installed (~1.5GB download, needs python3 on PATH)",
                move || {
                    let dir = asr_dir.clone().context("couldn't determine the ASR model directory")?;
                    let message = langspark_core::install_asr_model("Qwen3-ASR-0.6B", &dir, &|_, _| {})?;
                    Ok(format!("{message} — restart LangSpark for the Pronunciation tab to appear"))
                },
            ));
        }

        // Book catalog (see app.rs's `load_installed_book_catalog`, which
        // gates whether the Books tab shows at all). Only the catalog
        // metadata is fetched here — a book's own text is downloaded lazily
        // the first time it's opened (see `langspark_core::fetch_book`).
        let books_dir = settings.borrow().books_data_dir.clone().or_else(|| crate::config::AppDirs::new().map(|d| d.books_dir()));
        let already_installed = books_dir.as_ref().is_some_and(|d| d.join("catalog.json").exists());
        install_group.add(&build_install_row(
            "Book catalog (Aozora Bunko)",
            already_installed,
            "Not installed",
            move || {
                let dir = books_dir.clone().context("couldn't determine the books directory")?;
                let count = langspark_core::install_aozora_catalog(&dir.join("catalog.json"), &|_, _| {})?;
                Ok(format!("Installed ({count} books) — restart LangSpark for the Books tab to appear"))
            },
        ));

        // Paragraph translation model (see app.rs's `translate_paragraph_callback`,
        // which reports "not installed" through the paragraph popup until
        // this is installed — no restart needed, unlike the rows above,
        // since the model loads lazily on first use rather than at startup).
        let translation_dir = crate::config::AppDirs::new().map(|d| d.translation_model_dir());
        let already_installed = translation_dir.as_ref().is_some_and(|d| d.join("model.safetensors").exists());
        install_group.add(&build_install_row(
            "Paragraph translation (Helsinki-NLP OPUS-MT ja\u{2192}en)",
            already_installed,
            "Not installed (~850MB download)",
            move || {
                let dir = translation_dir.clone().context("couldn't determine the translation model directory")?;
                langspark_core::install_translation_model(&dir, &|_, _| {})
            },
        ));
    }
    study_page.add(&install_group);

    dialog.add(&study_page);

    // --- Data Sources page: what's downloaded, from where, under what license ---
    let data_page =
        adw::PreferencesPage::builder().title("Data Sources").icon_name("dialog-information-symbolic").build();

    let dict_dir = settings
        .borrow()
        .dictionary_data_dir
        .clone()
        .or_else(|| crate::config::AppDirs::new().map(|d| d.dictionaries_dir()));
    let books_dir_display = settings
        .borrow()
        .books_data_dir
        .clone()
        .or_else(|| crate::config::AppDirs::new().map(|d| d.books_dir()));
    let location_group = adw::PreferencesGroup::builder()
        .title("Downloaded To")
        .description("Installed via Study \u{2192} Language Installation")
        .build();
    location_group.add(
        &adw::ActionRow::builder()
            .title("Dictionary data directory")
            .subtitle(dict_dir.map(|p| p.display().to_string()).unwrap_or_else(|| "Unknown".to_string()))
            .build(),
    );
    location_group.add(
        &adw::ActionRow::builder()
            .title("Books data directory")
            .subtitle(books_dir_display.map(|p| p.display().to_string()).unwrap_or_else(|| "Unknown".to_string()))
            .build(),
    );
    data_page.add(&location_group);

    let dict_sources_group = adw::PreferencesGroup::builder()
        .title("Dictionary Data")
        .description("All freely available for reuse, with attribution")
        .build();
    dict_sources_group.add(&build_data_source_row(
        "JMdict",
        "Japanese-English dictionary entries: words, readings, meanings, parts of speech.",
        "Electronic Dictionary Research and Development Group (EDRDG), via scriptin/jmdict-simplified",
        "CC BY-SA 4.0",
        "https://www.edrdg.org/wiki/index.php/JMdict-EDICT_Dictionary_Project",
    ));
    dict_sources_group.add(&build_data_source_row(
        "Kanjidic",
        "Kanji readings, meanings, stroke counts, and JLPT/grade levels.",
        "Electronic Dictionary Research and Development Group (EDRDG), via scriptin/jmdict-simplified",
        "CC BY-SA 4.0",
        "https://www.edrdg.org/wiki/index.php/KANJIDIC_Project",
    ));
    dict_sources_group.add(&build_data_source_row(
        "Example sentences",
        "Japanese/English sentence pairs, used to fill in examples JMdict itself doesn't have for a word.",
        "The Tatoeba Project (tatoeba.org)",
        "CC BY 2.0 FR (a minority of sentences are CC0)",
        "https://tatoeba.org/en/downloads",
    ));
    data_page.add(&dict_sources_group);

    let books_group = adw::PreferencesGroup::builder().title("Reading Material").build();
    books_group.add(&build_data_source_row(
        "Aozora Bunko (青空文庫) book catalog",
        "Public-domain and author-permitted Japanese classic literature: full text plus \
         author/genre metadata for every work. Each book's text is downloaded the first time \
         it's opened from the Books tab, then cached locally.",
        "Aozora Bunko (aozora.gr.jp), via the aozorabunko/aozorabunko GitHub mirror",
        "Public domain / author-permitted — see each work's own colophon",
        "https://www.aozora.gr.jp/index_pages/aozora_manual.html",
    ));
    books_group.add(&build_data_source_row(
        "Paragraph translation model (OPUS-MT ja\u{2192}en, installable above)",
        "Installing converts the model's weights to a format this app's translation engine \
         (candle) can load, needing a throwaway Python + PyTorch environment for that one-time \
         step only. Translating itself runs fully offline afterward via candle (no libtorch, \
         unlike the speech recognition model below — deliberately, to avoid a conflicting \
         second libtorch version) — no text ever leaves this device to be translated.",
        "Language Technology Research Group at the University of Helsinki, via huggingface.co/Helsinki-NLP",
        "CC BY 4.0",
        "https://github.com/Helsinki-NLP/Opus-MT",
    ));
    data_page.add(&books_group);

    let voice_group = adw::PreferencesGroup::builder().title("Speech").build();
    voice_group.add(&build_data_source_row(
        "VOICEVOX Engine (Japanese TTS)",
        "Installable above on Linux x86_64/aarch64 or Windows x86_64 (Study \u{2192} Language \
         Installation); macOS and other architectures need Docker instead (see \
         scripts/setup-voicevox.sh). Free for commercial and non-commercial use, but requires \
         crediting \u{201c}VOICEVOX:\u{305a}\u{3093}\u{3060}\u{3082}\u{3093}\u{201d} (the default \
         Zundamon voice) wherever synthesized audio is used.",
        "Hiroshiba Kazuyuki / VOICEVOX project",
        "Free, with required credit — see terms",
        "https://voicevox.hiroshiba.jp/term/",
    ));
    voice_group.add(&build_data_source_row(
        "Speech recognition model (Qwen3-ASR-0.6B, installable above)",
        "Weights download directly; the tokenizer needs a throwaway Python venv (python3 on \
         PATH), removed once it's done. Only matters if this build has the optional 'asr' \
         Cargo feature enabled (the default — see README.md).",
        "Alibaba Qwen team, via huggingface.co/Qwen",
        "Apache 2.0",
        "https://huggingface.co/Qwen/Qwen3-ASR-0.6B",
    ));
    data_page.add(&voice_group);

    dialog.add(&data_page);

    dialog
}

/// Build an informational row describing one external data/service source:
/// what it is, who publishes it, and under what license — with a "Learn
/// more" link to the authoritative source. Purely informational (no
/// install button; see `build_install_row` for that).
fn build_data_source_row(title: &str, description: &str, source: &str, license: &str, url: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(format!("{description}\n\nSource: {source}\nLicense: {license}"))
        .build();
    row.add_suffix(&gtk4::LinkButton::builder().uri(url).label("Learn more").valign(gtk4::Align::Center).build());
    row
}

/// Build an "Install"/"Reinstall" row for a downloadable resource. Clicking
/// the button runs `install` on a background thread and shows its result in
/// the subtitle: `Ok(message)` becomes the new subtitle (and flips the
/// button to "Reinstall"), `Err(e)` becomes `"Install failed: {e}"`.
fn build_install_row(
    title: &str,
    already_installed: bool,
    not_installed_subtitle: &str,
    install: impl Fn() -> anyhow::Result<String> + Send + Sync + 'static,
) -> adw::ActionRow {
    let install = std::sync::Arc::new(install);

    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(if already_installed { "Installed" } else { not_installed_subtitle })
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
        #[strong]
        install,
        move |_| {
            install_btn.set_sensitive(false);
            row.set_subtitle("Installing\u{2026}");

            crate::task::spawn_on_main(glib::clone!(
                #[weak]
                row,
                #[weak]
                install_btn,
                #[strong]
                install,
                async move {
                    match crate::task::run_blocking(move || install()).await {
                        Ok(message) => {
                            row.set_subtitle(&message);
                            install_btn.set_label("Reinstall");
                        }
                        Err(e) => row.set_subtitle(&format!("Install failed: {e}")),
                    }
                    install_btn.set_sensitive(true);
                }
            ));
        }
    ));

    row
}

// Widget construction is exercised by the consolidated smoke test in
// `main.rs` (`gtk_smoke` module); see vocabulary::dialog::tests for why.
