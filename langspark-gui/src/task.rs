//! Async task infrastructure bridging blocking `langspark-core` work (SQLite
//! queries, dictionary parsing, audio synthesis) onto GTK's single-threaded
//! main loop, following the `run_blocking` pattern used by breadbin.
//!
//! GTK widgets aren't `Send`, so UI updates must happen on the GLib main
//! context (via [`spawn_on_main`]). Anything that blocks the thread (DB I/O,
//! CPU-bound parsing) must instead run on a background thread via
//! [`run_blocking`] and have its result awaited back on the main context.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

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

/// Progress update for a long-running operation (e.g. downloading a
/// dictionary or voice model), reported from a background thread.
#[derive(Debug, Clone)]
pub struct Progress {
    pub current: u64,
    pub total: u64,
    pub message: String,
}

impl Progress {
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.current as f64 / self.total as f64).clamp(0.0, 1.0)
        }
    }
}

/// Cooperative cancellation flag for long-running background operations.
/// The background closure must poll [`CancelToken::is_cancelled`]
/// periodically and return early when it flips.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// A blocking operation reporting progress via `on_progress`, cancellable via
/// `token`. Runs on the background thread pool; the returned future resolves
/// to `None` if the closure observed cancellation and returned early, `Some`
/// otherwise. `on_progress` runs on the *background* thread, so it must not
/// touch GTK widgets directly — forward it through a channel to the main
/// context if UI updates are needed.
pub fn run_blocking_cancellable<F, T>(token: CancelToken, f: F) -> impl Future<Output = Option<T>>
where
    F: FnOnce(&CancelToken) -> Option<T> + Send + 'static,
    T: Send + 'static,
{
    let handle = runtime().spawn_blocking(move || f(&token));
    async move { handle.await.expect("background task panicked") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_blocking_returns_result() {
        let result = run_blocking(|| 2 + 2).await;
        assert_eq!(result, 4);
    }

    #[tokio::test]
    async fn test_cancel_token() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_run_blocking_cancellable_early_return() {
        let token = CancelToken::new();
        token.cancel();

        let result = run_blocking_cancellable(token, |t| {
            if t.is_cancelled() {
                return None;
            }
            Some(42)
        })
        .await;

        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_run_blocking_cancellable_completes() {
        let token = CancelToken::new();
        let result = run_blocking_cancellable(token, |_| Some(7)).await;
        assert_eq!(result, Some(7));
    }

    #[test]
    fn test_progress_fraction() {
        let progress = Progress { current: 3, total: 10, message: "downloading".to_string() };
        assert!((progress.fraction() - 0.3).abs() < 1e-9);

        let zero_total = Progress { current: 0, total: 0, message: String::new() };
        assert_eq!(zero_total.fraction(), 0.0);
    }
}
