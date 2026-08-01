//! Application module
//!
//! Contains the main window, application state, and coordination between UI and core.

use gtk4::prelude::*;
use gtk4::{ApplicationWindow, Box};
use libadwaita::{AdwApplication, AdwApplicationWindow, HeaderBar};

/// Main application window
pub fn build_main_window(app: &AdwApplication) -> AdwApplicationWindow {
    // Create the window
    let window = AdwApplicationWindow::builder()
        .application(app)
        .title("LangSpark")
        .default_width(800)
        .default_height(600)
        .build();
    
    // Set up header bar
    let header = HeaderBar::builder()
        .show_title_buttons(true)
        .build();
    window.set_title_widget(Some(&header));
    
    // Create main content area
    let content = Box::new(gtk4::Orientation::Vertical, 0);
    content.set_hexpand(true);
    content.set_vexpand(true);
    window.set_child(Some(&content));
    
    window
}
