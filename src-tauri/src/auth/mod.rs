//! Authentication module: PIN, password, recovery unlock, session management,
//! attempt throttling, and auto-lock.
//!
//! No key material ever leaves this module. The Session holds the MVK in a
//! zeroizing buffer; IPC commands borrow it via with_mvk() closures.

use zeroize::{Zeroize, ZeroizeOnDrop};

pub mod auto_lock;
pub mod password;
pub mod pin;
pub mod recovery;
pub mod session;
pub mod session_lock;
pub mod throttle;

/// How the vault was unlocked.
#[derive(Debug, Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub enum UnlockMethod {
    Pin,
    Password,
    Recovery,
}

/// All authentication errors. Never leak which credential or wrapped entry
/// failed — every failure is InvalidCredential.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid credential")]
    InvalidCredential,
    #[error("vault not found at path")]
    VaultNotFound,
    #[error("vault metadata corrupted")]
    VaultCorrupted,
    #[error("vault locked out: try again in {0} seconds")]
    LockedOut(i64),
    #[error("vault already unlocked")]
    AlreadyUnlocked,
    #[error("vault already locked")]
    AlreadyLocked,
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("session error: {0}")]
    Session(String),
}

impl From<crate::crypto::CryptoError> for AuthError {
    fn from(e: crate::crypto::CryptoError) -> Self {
        AuthError::Crypto(e.to_string())
    }
}

impl From<crate::database::DbError> for AuthError {
    fn from(e: crate::database::DbError) -> Self {
        AuthError::Database(e.to_string())
    }
}

pub type AuthResult<T> = std::result::Result<T, AuthError>;
