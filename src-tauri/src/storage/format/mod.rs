//! Format-level constants, error type, and re-exports.
//!
//! Format v1 is FROZEN (see docs/CRYPTO_DESIGN.md). Changes require a
//! `format_version` bump.

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::CryptoError;
use crate::crypto::hkdf::{derive_subkey, LABEL_FILE_KEY_PREFIX};
use crate::crypto::mvk::Mvk;
use thiserror::Error;

pub mod chunk;
pub mod finalize;
pub mod header;
pub mod reader;
pub mod writer;

pub const MAGIC: &[u8; 4] = b"VYLR";
pub const VERSION: u32 = 1;
pub const CHUNK_SIZE: u64 = 65536;
pub const TAG_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const HEADER_CORE_LEN: usize = 36; // file_id(16) + chunk_size(4) + total_chunks(8) + total_length(8)

/// All format errors are typed; never silent.
#[derive(Error, Debug)]
pub enum FormatError {
    #[error("file too small for header")]
    TooSmall,
    #[error("bad magic: expected VYLR")]
    BadMagic,
    #[error("unsupported format version: {0}")]
    UnsupportedVersion(u32),
    #[error("header AEAD authentication failed")]
    HeaderAuth,
    #[error("chunk {0} AEAD authentication failed")]
    ChunkAuth(u64),
    #[error("finalize record AEAD authentication failed")]
    FinalizeAuth,
    #[error("root hash mismatch (reordered or duplicated chunks)")]
    RootHashMismatch,
    #[error("declared length mismatch")]
    LengthMismatch,
    #[error("invalid byte range")]
    InvalidRange,
    #[error("io error: {0}")]
    Io(String),
    #[error("crypto error: {0}")]
    Crypto(String),
}

impl From<CryptoError> for FormatError {
    fn from(e: CryptoError) -> Self {
        FormatError::Crypto(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, FormatError>;

/// Per-file key (256 bits), derived from MVK via HKDF with label
/// `veylora/v1/file-key/<file_id_hex>`. Zeroized on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct FileKey([u8; 32]);

impl FileKey {
    /// Derive from MVK and a 16-byte file_id.
    pub fn derive(mvk: &Mvk, file_id: &[u8; 16]) -> Result<Self> {
        let file_id_hex = hex_encode(file_id);
        let mut label = LABEL_FILE_KEY_PREFIX.to_vec();
        label.extend_from_slice(file_id_hex.as_bytes());
        let subkey = derive_subkey(mvk.as_bytes(), &label)?;
        Ok(Self::from_bytes(&subkey))
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self(*bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

pub use chunk::*;
pub use finalize::*;
pub use header::*;
pub use reader::*;
pub use writer::*;
