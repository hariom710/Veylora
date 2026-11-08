//! Metadata column encryption with XChaCha20-Poly1305.
//!
//! Uses metadata-key (from MVK via frozen label veylora/v1/metadata-key).
//! Nonce: random 24 bytes per column, stored alongside ciphertext.
//! AAD: table_name || row_id_LE_bytes || column_name.
//!
//! No new HKDF labels needed — nonces are random, not derived.

use chacha20poly1305::{XChaCha20Poly1305, KeyInit};
use chacha20poly1305::aead::{Aead, Payload};
use hybrid_array::Array;
use std::result::Result as StdResult;

type EncryptedMeta = ([u8; 24], Vec<u8>);

/// Encrypt a metadata column value.
/// Returns (nonce[24], ciphertext).
pub fn encrypt_meta(
    metadata_key: &[u8; 32],
    table: &str,
    row_id: u64,
    column: &str,
    plaintext: &[u8],
) -> StdResult<EncryptedMeta, MetadataCryptoError> {
    let cipher = XChaCha20Poly1305::new_from_slice(metadata_key)
        .map_err(|_| MetadataCryptoError::AeadInit)?;

    let mut nonce_bytes = [0u8; 24];
    getrandom::getrandom(&mut nonce_bytes).map_err(|_| MetadataCryptoError::Rng)?;
    let nonce = Array(nonce_bytes);

    let aad = build_aad(table, row_id, column);
    let ciphertext = cipher
        .encrypt(&nonce, Payload { aad: aad.as_slice(), msg: plaintext })
        .map_err(|_| MetadataCryptoError::Encrypt)?;

    Ok((nonce_bytes, ciphertext))
}

/// Decrypt a metadata column value.
pub fn decrypt_meta(
    metadata_key: &[u8; 32],
    table: &str,
    row_id: u64,
    column: &str,
    nonce: &[u8; 24],
    ciphertext: &[u8],
) -> StdResult<Vec<u8>, MetadataCryptoError> {
    let cipher = XChaCha20Poly1305::new_from_slice(metadata_key)
        .map_err(|_| MetadataCryptoError::AeadInit)?;
    let nonce = Array(*nonce);

    let aad = build_aad(table, row_id, column);
    let plaintext = cipher
        .decrypt(&nonce, Payload { aad: aad.as_slice(), msg: ciphertext })
        .map_err(|_| MetadataCryptoError::Decrypt)?;

    Ok(plaintext)
}

/// Build AAD: table_name || row_id_LE(8 bytes) || column_name.
fn build_aad(table: &str, row_id: u64, column: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(table.len() + 8 + column.len());
    aad.extend_from_slice(table.as_bytes());
    aad.extend_from_slice(&row_id.to_le_bytes());
    aad.extend_from_slice(column.as_bytes());
    aad
}

/// Errors from metadata encryption/decryption.
#[derive(Debug, thiserror::Error)]
pub enum MetadataCryptoError {
    #[error("AEAD init failed")]
    AeadInit,
    #[error("OS RNG failure")]
    Rng,
    #[error("encrypt failed")]
    Encrypt,
    #[error("decrypt failed (authentication)")]
    Decrypt,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_meta_roundtrip() {
        let key = [0x42u8; 32];
        let plaintext = b"secret filename.pdf";
        let (nonce, ciphertext) = encrypt_meta(&key, "encrypted_files", 1, "filename", plaintext)
            .expect("encrypt");
        let decrypted = decrypt_meta(&key, "encrypted_files", 1, "filename", &nonce, &ciphertext)
            .expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_meta_twice_different_ciphertext() {
        let key = [0x42u8; 32];
        let plaintext = b"same plaintext";
        let (n1, c1) = encrypt_meta(&key, "encrypted_files", 1, "filename", plaintext)
            .expect("encrypt 1");
        let (n2, c2) = encrypt_meta(&key, "encrypted_files", 1, "filename", plaintext)
            .expect("encrypt 2");
        assert_ne!(n1, n2, "nonces should differ");
        assert_ne!(c1, c2, "ciphertexts should differ");
    }

    #[test]
    fn decrypt_meta_same_plaintext() {
        let key = [0x42u8; 32];
        let plaintext = b"same plaintext";
        let (n1, c1) = encrypt_meta(&key, "encrypted_files", 1, "filename", plaintext)
            .expect("encrypt 1");
        let (n2, c2) = encrypt_meta(&key, "encrypted_files", 1, "filename", plaintext)
            .expect("encrypt 2");
        let d1 = decrypt_meta(&key, "encrypted_files", 1, "filename", &n1, &c1).expect("decrypt 1");
        let d2 = decrypt_meta(&key, "encrypted_files", 1, "filename", &n2, &c2).expect("decrypt 2");
        assert_eq!(d1, plaintext);
        assert_eq!(d2, plaintext);
    }

    #[test]
    fn decrypt_meta_tampered_row_id_fails() {
        let key = [0x42u8; 32];
        let plaintext = b"secret";
        let (nonce, ciphertext) = encrypt_meta(&key, "encrypted_files", 1, "filename", plaintext)
            .expect("encrypt");
        let err = decrypt_meta(&key, "encrypted_files", 999, "filename", &nonce, &ciphertext)
            .unwrap_err();
        assert!(matches!(err, MetadataCryptoError::Decrypt));
    }

    #[test]
    fn decrypt_meta_wrong_key_fails() {
        let key1 = [0x42u8; 32];
        let key2 = [0x43u8; 32];
        let plaintext = b"secret";
        let (nonce, ciphertext) = encrypt_meta(&key1, "encrypted_files", 1, "filename", plaintext)
            .expect("encrypt");
        let err = decrypt_meta(&key2, "encrypted_files", 1, "filename", &nonce, &ciphertext)
            .unwrap_err();
        assert!(matches!(err, MetadataCryptoError::Decrypt));
    }

    #[test]
    fn decrypt_meta_tampered_column_fails() {
        let key = [0x42u8; 32];
        let plaintext = b"secret";
        let (nonce, ciphertext) = encrypt_meta(&key, "encrypted_files", 1, "filename", plaintext)
            .expect("encrypt");
        let err = decrypt_meta(&key, "encrypted_files", 1, "folder_name", &nonce, &ciphertext)
            .unwrap_err();
        assert!(matches!(err, MetadataCryptoError::Decrypt));
    }

    #[test]
    fn decrypt_meta_tampered_ciphertext_fails() {
        let key = [0x42u8; 32];
        let plaintext = b"secret";
        let (nonce, mut ciphertext) = encrypt_meta(&key, "encrypted_files", 1, "filename", plaintext)
            .expect("encrypt");
        ciphertext[0] ^= 0x01;
        let err = decrypt_meta(&key, "encrypted_files", 1, "filename", &nonce, &ciphertext)
            .unwrap_err();
        assert!(matches!(err, MetadataCryptoError::Decrypt));
    }
}
