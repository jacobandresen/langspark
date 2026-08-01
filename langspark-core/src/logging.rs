//! Logging configuration for langspark-core
//!
//! This module provides centralized logging setup for the core library.
//! Use `init_logging` at application startup to configure logging.

use log::{Level, LevelFilter};

/// Initialize logging for the core library
///
/// This sets up the logging level based on the RUST_LOG environment variable.
/// If RUST_LOG is not set, defaults to Warn level.
///
/// # Examples
///
/// ```no_run
/// use langspark_core::logging::init_logging;
/// 
/// fn main() {
///     init_logging();
///     log::info!("Application started");
/// }
/// ```
pub fn init_logging() {
    // The actual logger implementation (env_logger) is initialized in the GUI crate
    // This module just provides the log macros and level configuration
    
    // Default to Warn if RUST_LOG is not set
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "warn,langspark_core=info");
    }
}

/// Check if debug logging is enabled for a specific module
pub fn is_debug_enabled(module: &str) -> bool {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "warn".to_string());
    
    // Simple check - if debug or trace is enabled globally or for this module
    filter.contains("debug") || filter.contains("trace") || filter.contains(&format!("{}::debug", module))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_init_logging() {
        init_logging();
        // Just verify it doesn't panic
    }
    
    #[test]
    fn test_debug_check() {
        std::env::set_var("RUST_LOG", "debug");
        assert!(is_debug_enabled("any_module"));
        
        std::env::set_var("RUST_LOG", "info");
        assert!(!is_debug_enabled("any_module"));
    }
}
