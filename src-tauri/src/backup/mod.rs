//! Backup and restore container format.
//!
//! The backup container packages encrypted blobs + metadata into a single
//! .vey file, encrypted under backup_key = HKDF(MVK, "veylora/v1/backup-key").
//!
//! Container layout:
//!   REGION_HEADER   magic || version || backup_nonce || encrypted_header_payload
//!                   || meta_len || meta_hash || header_tag
//!   REGION_MANIFEST AEAD-encrypted manifest (file metadata)
//!   REGION_BLOBS    Raw encrypted blob contents (format v1, not re-encrypted)
//!   REGION_TRAILER  trailer_magic || file_count || blobs_hash || final_tag

pub mod container;
pub mod create;
pub mod restore;
pub mod verify;

use thiserror::Error;

/// All backup/restore errors are typed; never silent.
#[derive(Debug, Error)]
pub enum BackupError {
    #[error("invalid container: {0}")]
    InvalidContainer(String),
    #[error("corrupted container: {0}")]
    Corrupted(String),
    #[error("file already exists: {file_id}")]
    FileExists { file_id: String },
    #[error("vault is locked")]
    Locked,
    #[error("internal error: {0}")]
    Internal(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, BackupError>;

/// Information returned on backup create/verify.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupInfo {
    pub path: String,
    pub file_count: u64,
    pub total_size: u64,
    pub created_at_unix: i64,
    /// Which regions were cryptographically verified.
    /// Always contains "blobs_hash" when trailer is readable;
    /// contains "manifest" only when MVK is provided and manifest AEAD succeeds.
    pub verified_fields: Vec<String>,
}

/// Information returned on backup restore.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RestoreInfo {
    pub files_restored: u64,
    pub bytes_written: u64,
}

/// History entry from the backup_history table.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupHistoryEntry {
    pub path: String,
    pub status: String,
    pub created_at_unix: i64,
}
