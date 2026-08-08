//! Async task infrastructure bridging blocking `langspark-core` work (SQLite
//! queries, dictionary parsing, audio synthesis) onto GTK's single-threaded
//! main loop, following the `run_blocking` pattern used by breadbin.
//!
//! GTK widgets aren't `Send`, so UI updates must happen on the GLib main
//! context (via [`spawn_on_main`]). Anything that blocks the thread (DB I/O,
//! CPU-bound parsing) must instead run on a background thread via
//! [`run_blocking`] and have its result awaited back on the main context.

use std::future::Future;
use std::sync::OnceLock;

/// A lazily-initialized multi-threaded Tokio runtime used only for
/// `spawn_blocking` — the GTK main loop itself is driven by `glib`, not Tokio.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to build background task runtime")
    })
}

/// Run a blocking closure on a background thread pool and await its result.
/// Use this to wrap any `langspark-core` call that touches disk (SQLite,
/// dictionary files, audio) from a GTK callback without freezing the UI:
///
/// ```ignore
/// glib::spawn_future_local(async move {
///     let entries = task::run_blocking(move || repo.get_by_language("ja")).await;
///     // ... update widgets with `entries` here, back on the main context ...
/// });
/// ```
pub fn run_blocking<F, T>(f: F) -> impl Future<Output = T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let handle = runtime().spawn_blocking(f);
    async move { handle.await.expect("background task panicked") }
}

/// Spawn a future on the GLib main context — the only place GTK widgets may
/// be touched. Thin wrapper so callers don't need to import `glib` directly
/// just to kick off UI-updating async work.
pub fn spawn_on_main<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    glib::spawn_future_local(future);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_blocking_returns_result() {
        let result = run_blocking(|| 2 + 2).await;
        assert_eq!(result, 4);
    }
}
