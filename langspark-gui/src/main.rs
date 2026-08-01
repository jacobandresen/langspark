//! LangSpark GTK4 Application
//!
//! Main application entry point. Initializes GTK, sets up the application window,
//! and coordinates between UI components and core logic.

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use libadwaita::{AdwApplication, AdwApplicationWindow};

mod app;
mod ui;

fn main() -> glib::ExitCode {
    // Initialize logging
    env_logger::init();
    
    // Create the application
    let app = AdwApplication::builder()
        .application_id("org.langspark.LangSpark")
        .build();
    
    app.connect_activate(|app| {
        // Create the main window
        let window = app::build_main_window(app);
        window.present();
    });
    
    // Run the application
    app.run()
}
