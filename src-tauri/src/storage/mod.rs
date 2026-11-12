//! Storage layer: encrypted file format (Phase 2) + vault I/O (Phase 3).
//!
//! Format is frozen (docs/CRYPTO_DESIGN.md). Vault I/O handles atomic writes,
//! directory layout, and crash recovery. No key material is stored here.

pub mod blob;
pub mod crash;
pub mod format;
pub mod vault_dir;

pub use blob::{cleanup_tmp_files, delete_blob, list_blobs, read_blob, write_blob, BlobError};
pub use crash::{startup_scan, CrashError, ScanReport};
pub use format::{
    read_range, verify_file, write_file, FileKey, FormatError, Result, CHUNK_SIZE, MAGIC, VERSION,
};
pub use vault_dir::fsync_dir;
