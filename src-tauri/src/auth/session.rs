#![allow(missing_docs)]

//! Session object: holds the MVK in a zeroizing buffer, plus vault_id,
//! unlock_method, and last_activity. Never serializable, never Clone, never
//! Debug-printable (MVK is redacted).
//!
//! SessionManager wraps Option<Session> in a Mutex so at most one session
//! exists at a time.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::{Zeroize, ZeroizeOnDrop};
use crate::crypto::mvk::Mvk;
use crate::auth::UnlockMethod;

/// Master Vault Key session: the MVK lives in a zeroizing buffer.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Session {
    mvk: Mvk,
    pub vault_id: u64,
    pub unlock_method: UnlockMethod,
    pub last_activity: u64,
}

impl Session {
    pub fn new(mvk: Mvk, vault_id: u64, unlock_method: UnlockMethod) -> Self {
        Self {
            mvk,
            vault_id,
            unlock_method,
            last_activity: now_unix(),
        }
    }

    /// Borrow the MVK inside a closure. The closure receives the raw bytes;
    /// the Session retains ownership.
    pub fn with_mvk<F, R>(&self, f: F) -> Result<R, SessionError>
    where
        F: FnOnce(&[u8; 32]) -> R,
    {
        Ok(f(self.mvk.as_bytes()))
    }
}

// Manual Debug: redact the MVK so it never appears in logs.
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("vault_id", &self.vault_id)
            .field("unlock_method", &self.unlock_method)
            .field("last_activity", &self.last_activity)
            .field("mvk", &"<REDACTED>")
            .finish()
    }
}

/// Inner state of the session manager.
struct Inner {
    session: Option<Session>,
    current_path: Option<String>,
}

/// Session manager: holds at most one active Session behind a Mutex.
pub struct SessionManager {
    inner: Mutex<Inner>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self { inner: Mutex::new(Inner { session: None, current_path: None }) }
    }

    pub fn is_locked(&self) -> bool {
        self.inner.lock().unwrap().session.is_none()
    }

    /// Lock the vault: drop the Session (zeroizing the MVK) and clear path.
    pub fn lock(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.session = None;
        guard.current_path = None;
    }

    pub fn set_session(&self, session: Session, path: String) {
        let mut guard = self.inner.lock().unwrap();
        guard.session = Some(session);
        guard.current_path = Some(path);
    }

    /// Set only the current vault path (no session yet).
    pub fn set_current_path(&self, path: String) {
        let mut guard = self.inner.lock().unwrap();
        guard.current_path = Some(path);
    }

    pub fn current_path(&self) -> Option<String> {
        self.inner.lock().unwrap().current_path.clone()
    }

    /// Get the vault_id of the active session, or None if locked.
    pub fn vault_id(&self) -> Option<u64> {
        self.inner.lock().unwrap().session.as_ref().map(|s| s.vault_id)
    }

    /// Update last_activity on the active session.
    pub fn touch_activity(&self) {
        if let Some(session) = self.inner.lock().unwrap().session.as_mut() {
            session.last_activity = now_unix();
        }
    }

    /// Borrow the MVK from the active session.
    pub fn with_mvk<F, R>(&self, f: F) -> Result<R, SessionError>
    where
        F: FnOnce(&[u8; 32]) -> R,
    {
        let guard = self.inner.lock().unwrap();
        let session = guard.session.as_ref().ok_or(SessionError::Locked)?;
        session.with_mvk(f)
    }

    /// Check auto-lock: if the session has been idle longer than `timeout_secs`,
    /// lock the vault.
    pub fn check_auto_lock(&self, now: u64, timeout_secs: u64) {
        let mut guard = self.inner.lock().unwrap();
        if let Some(session) = guard.session.as_mut() {
            if now - session.last_activity > timeout_secs {
                guard.session = None;
                guard.current_path = None;
            }
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManager")
            .field("locked", &self.is_locked())
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session locked")]
    Locked,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
