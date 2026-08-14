//! UI module
//!
//! Shared UI helpers. The tabs themselves (vocabulary, review, pronunciation)
//! are top-level sibling modules declared in `main.rs`.

/// Load and register LangSpark's custom CSS (`data/style.css`) for the
/// default display. Layers on top of libadwaita's default theme, so it must
/// be called after the `Application` is activated (a default display exists by then).
pub fn load_styles() {
    let provider = gtk4::CssProvider::new();
    provider.load_from_string(include_str!("../data/style.css"));

    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    } else {
        log::warn!("no default display available; custom styles were not loaded");
    }
}
