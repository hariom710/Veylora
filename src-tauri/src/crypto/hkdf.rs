//! HKDF-SHA256 domain-separated subkey derivation.
//!
//! Every subkey uses an explicit label. Labels are constrained to the frozen
//! list; unknown or empty labels are rejected.

use hkdf::Hkdf;
use sha2::Sha256;

use crate::crypto::{CryptoError, Result};

pub const LABEL_VAULT_KEY: &[u8] = b"veylora/v1/vault-key";
pub const LABEL_METADATA_KEY: &[u8] = b"veylora/v1/metadata-key";
pub const LABEL_RECOVERY_WRAP: &[u8] = b"veylora/v1/recovery-wrap";
pub const LABEL_BACKUP_KEY: &[u8] = b"veylora/v1/backup-key";
pub const LABEL_NONCE: &[u8] = b"veylora/v1/nonce";
pub const LABEL_HEADER_NONCE: &[u8] = b"veylora/v1/header-nonce";
pub const LABEL_FINALIZE_NONCE: &[u8] = b"veylora/v1/finalize-nonce";
pub const LABEL_FILE_KEY_PREFIX: &[u8] = b"veylora/v1/file-key/";

const EXACT_LABELS: &[&[u8]] = &[
    LABEL_VAULT_KEY,
    LABEL_METADATA_KEY,
    LABEL_RECOVERY_WRAP,
    LABEL_BACKUP_KEY,
    LABEL_NONCE,
    LABEL_HEADER_NONCE,
    LABEL_FINALIZE_NONCE,
];

pub fn derive_subkey(key: &[u8; 32], label: &[u8]) -> Result<[u8; 32]> {
    validate_label(label)?;
    let hkdf = Hkdf::<Sha256>::new(None, key);
    let mut okm = [0u8; 32];
    hkdf.expand(label, &mut okm)
        .map_err(|_| CryptoError::Hkdf)?;
    Ok(okm)
}

/// Derive a subkey with extra context bytes appended to the label info.
///
/// The base `label` is validated against the frozen label list; `info` is
/// appended to the HKDF info field for domain separation within a label
/// (e.g. chunk index for per-chunk nonces).
pub fn derive_subkey_info(key: &[u8; 32], label: &[u8], info: &[u8]) -> Result<[u8; 32]> {
    validate_label(label)?;
    let hkdf = Hkdf::<Sha256>::new(None, key);
    let mut combined = label.to_vec();
    combined.extend_from_slice(info);
    let mut okm = [0u8; 32];
    hkdf.expand(&combined, &mut okm)
        .map_err(|_| CryptoError::Hkdf)?;
    Ok(okm)
}

fn validate_label(label: &[u8]) -> Result<()> {
    if label.is_empty() {
        return Err(CryptoError::InvalidLabel(label.to_vec()));
    }
    if EXACT_LABELS.contains(&label) {
        return Ok(());
    }
    if label.starts_with(LABEL_FILE_KEY_PREFIX) && label.len() > LABEL_FILE_KEY_PREFIX.len() {
        return Ok(());
    }
    Err(CryptoError::InvalidLabel(label.to_vec()))
}
