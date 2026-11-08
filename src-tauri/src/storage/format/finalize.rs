//! Finalize record read/write with AEAD.
//!
//! Finalize (on disk, after last chunk):
//!   nonce        [24]  = HKDF(file-key, "finalize-nonce")
//!   ciphertext   [N]   = AEAD(root_hash[32] || total_length_u64_LE || total_chunks_u64_LE)
//!   tag          [16]
//!   AAD          = "VYLR" || format_version_LE || file_id

use chacha20poly1305::{XChaCha20Poly1305, KeyInit};
use chacha20poly1305::aead::{Aead, Payload};
use hybrid_array::Array;
use sha2::{Digest, Sha256};

use crate::crypto::hkdf::{
    derive_subkey, LABEL_FINALIZE_NONCE,
};
use crate::storage::format::{
    MAGIC, NONCE_LEN, TAG_LEN, VERSION, FormatError, FileKey, Result,
};

/// root_hash = SHA-256 over all chunk (ciphertext || tag) pairs, in order.
pub fn compute_root_hash(chunk_ct_tags: &[Vec<u8>]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for ct_tag in chunk_ct_tags {
        hasher.update(ct_tag);
    }
    hasher.finalize().into()
}

/// Encrypt the finalize record. Returns nonce || ciphertext || tag.
pub fn encrypt_finalize(
    file_key: &FileKey,
    file_id: &[u8; 16],
    total_chunks: u64,
    total_length: u64,
    root_hash: &[u8; 32],
) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new_from_slice(file_key.as_bytes())
        .expect("aead key init cannot fail");
    let nonce_bytes = derive_subkey(file_key.as_bytes(), LABEL_FINALIZE_NONCE)
        .expect("finalize nonce derivation cannot fail with valid label");
    let nonce_arr: [u8; NONCE_LEN] = nonce_bytes[..NONCE_LEN]
        .try_into()
        .expect("nonce bytes are always 24");
    let nonce = Array(nonce_arr);

    let mut aad = Vec::with_capacity(4 + 4 + 16);
    aad.extend_from_slice(MAGIC);
    aad.extend_from_slice(&VERSION.to_le_bytes());
    aad.extend_from_slice(file_id);

    let mut plaintext = Vec::with_capacity(32 + 8 + 8);
    plaintext.extend_from_slice(root_hash);
    plaintext.extend_from_slice(&total_length.to_le_bytes());
    plaintext.extend_from_slice(&total_chunks.to_le_bytes());

    let buffer = cipher
        .encrypt(&nonce, Payload { aad: aad.as_slice(), msg: &plaintext })
        .expect("finalize encryption cannot fail");

    let mut out = Vec::with_capacity(NONCE_LEN + buffer.len());
    out.extend_from_slice(&nonce_bytes[..NONCE_LEN]);
    out.extend_from_slice(&buffer);
    out
}

/// Decrypt and verify the finalize record.
pub fn decrypt_finalize(
    file_key: &FileKey,
    file_id: &[u8; 16],
    total_chunks: u64,
    total_length: u64,
    root_hash: &[u8; 32],
    nonce_ct_tag: &[u8],
) -> Result<()> {
    if nonce_ct_tag.len() < NONCE_LEN + TAG_LEN {
        return Err(FormatError::FinalizeAuth);
    }

    let cipher = XChaCha20Poly1305::new_from_slice(file_key.as_bytes())
        .map_err(|e| FormatError::Crypto(e.to_string()))?;
    let nonce_bytes = &nonce_ct_tag[..NONCE_LEN];
    let ct_tag = &nonce_ct_tag[NONCE_LEN..];

    let nonce_arr: [u8; NONCE_LEN] = nonce_bytes.try_into().map_err(|_| FormatError::FinalizeAuth)?;
    let nonce = Array(nonce_arr);

    let mut aad = Vec::with_capacity(4 + 4 + 16);
    aad.extend_from_slice(MAGIC);
    aad.extend_from_slice(&VERSION.to_le_bytes());
    aad.extend_from_slice(file_id);

    let plaintext = cipher
        .decrypt(&nonce, Payload { aad: aad.as_slice(), msg: ct_tag })
        .map_err(|_| FormatError::FinalizeAuth)?;

    // Expected plaintext: root_hash(32) || total_length(8) || total_chunks(8)
    let mut expected = Vec::with_capacity(48);
    expected.extend_from_slice(root_hash);
    expected.extend_from_slice(&total_length.to_le_bytes());
    expected.extend_from_slice(&total_chunks.to_le_bytes());

    if plaintext != expected {
        return Err(FormatError::FinalizeAuth);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::mvk::Mvk;
    use crate::storage::format::FileKey;

    #[test]
    fn finalize_roundtrip() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        let total_chunks = 2u64;
        let total_length = 100000u64;
        let root_hash = [0xabu8; 32];

        let enc = encrypt_finalize(&file_key, &file_id, total_chunks, total_length, &root_hash);
        decrypt_finalize(&file_key, &file_id, total_chunks, total_length, &root_hash, &enc)
            .expect("decrypt finalize");
    }

    #[test]
    fn finalize_tampered_rejected() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        let total_chunks = 1u64;
        let total_length = 100u64;
        let root_hash = [0xabu8; 32];

        let enc = encrypt_finalize(&file_key, &file_id, total_chunks, total_length, &root_hash);
        let mut tampered = enc.clone();
        tampered[NONCE_LEN] ^= 0x01;

        let err = decrypt_finalize(
            &file_key, &file_id, total_chunks, total_length, &root_hash, &tampered,
        )
        .unwrap_err();
        assert!(matches!(err, FormatError::FinalizeAuth));
    }

    #[test]
    fn root_hash_detects_reorder() {
        let a = vec![1u8; 10];
        let b = vec![2u8; 20];
        let h1 = compute_root_hash(&[a.clone(), b.clone()]);
        let h2 = compute_root_hash(&[b.clone(), a.clone()]);
        assert_ne!(h1, h2);
    }
}
