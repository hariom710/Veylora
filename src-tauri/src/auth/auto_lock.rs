//! Auto-lock watcher: spawns a std::thread that checks last_activity every
//! 30s and calls lock() on timeout.
//!
//! No tokio/async runtime — pure std::thread.

use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use crate::auth::session::SessionManager;

/// Spawn a background thread that periodically checks if the session has been
/// idle too long and locks the vault if so.
///
/// Checks every 30 seconds. The `timeout_secs` parameter is the idle timeout
/// in seconds (default 300 = 5 minutes).
pub fn spawn_watcher(manager: Arc<SessionManager>, timeout_secs: u64) -> thread::JoinHandle<()> {
    let manager = Arc::clone(&manager);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(30));
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            manager.check_auto_lock(now, timeout_secs);
        }
    })
}
