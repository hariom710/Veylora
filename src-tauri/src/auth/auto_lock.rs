//! Auto-lock watcher: spawns a std::thread that checks last_activity every
//! 30s and calls lock() on timeout.
//!
//! No tokio/async runtime — pure std::thread.

use crate::auth::session::SessionManager;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Idle timeout in seconds, adjustable at runtime from Settings → Security.
/// 0 disables auto-lock. Defaults to 5 minutes (300s), per the UI spec.
static TIMEOUT_SECS: AtomicU64 = AtomicU64::new(300);

/// Change the idle timeout for the already-running watcher.
pub fn set_timeout(secs: u64) {
    TIMEOUT_SECS.store(secs, Ordering::Relaxed);
}

/// Current idle timeout in seconds (0 = never).
pub fn timeout_secs() -> u64 {
    TIMEOUT_SECS.load(Ordering::Relaxed)
}

/// Spawn a background thread that periodically checks if the session has been
/// idle too long and locks the vault if so.
///
/// Checks every 30 seconds. The timeout is read from TIMEOUT_SECS on every
/// tick, so Settings changes apply without a restart.
pub fn spawn_watcher(manager: Arc<SessionManager>) -> thread::JoinHandle<()> {
    let manager = Arc::clone(&manager);
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(30));
        let timeout = timeout_secs();
        if timeout == 0 {
            continue;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        manager.check_auto_lock(now, timeout);
    })
}
