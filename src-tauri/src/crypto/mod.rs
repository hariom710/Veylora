//! Shared crypto types and errors.

pub mod hkdf;
pub mod kdf;
pub mod mvk;
pub mod recovery;
pub mod wrap;

pub use hkdf::{
    derive_subkey, LABEL_BACKUP_KEY, LABEL_FILE_KEY_PREFIX, LABEL_METADATA_KEY, LABEL_NONCE,
    LABEL_RECOVERY_WRAP, LABEL_VAULT_KEY,
};
pub use kdf::{derive_password_kek, derive_pin_kek};
pub use mvk::{Mvk, SecretKey};
pub use wrap::{unwrap_mvk, wrap_mvk, WrappedPayload};

use thiserror::Error;

/// All recoverable crypto errors. Failures are typed, never silent.
#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("OS RNG failure")]
    Rng,
    #[error("Argon2 error: {0}")]
    Argon2(String),
    #[error("AEAD operation failed")]
    Aead,
    #[error("HKDF operation failed")]
    Hkdf,
    #[error("invalid HKDF label: {0:?}")]
    InvalidLabel(Vec<u8>),
    #[error("MVK unwrap failed (authentication)")]
    UnwrapFailed,
    #[error("invalid recovery key")]
    InvalidRecoveryKey,
    #[error("recovery key checksum mismatch")]
    ChecksumMismatch,
    #[error("invalid base32 in recovery key")]
    InvalidBase32,
    #[error("invalid key length")]
    InvalidLength,
    #[error("serialization failed")]
    Serialization,
    #[error("unsupported vault meta version")]
    UnsupportedVersion,
}

pub type Result<T> = std::result::Result<T, CryptoError>;
