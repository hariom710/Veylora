//! Chunk encrypt/decrypt with AAD.
//!
//! Chunk i (on disk, contiguous):
//!   nonce        [24]  = HKDF(file-key, "nonce", chunk_index)
//!   ciphertext   [N]   = XChaCha20-Poly1305(plaintext_i)
//!   tag          [16]
//!
//! Chunk AAD (authenticated, not encrypted):
//!   "VYLR" || format_version_LE || file_id ||
//!   chunk_index_u64_LE || total_chunks_u64_LE || total_length_u64_LE

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use hybrid_array::Array;

use crate::crypto::hkdf::{derive_subkey_info, LABEL_NONCE};
use crate::storage::format::{FileKey, FormatError, Result, CHUNK_SIZE, MAGIC, NONCE_LEN, VERSION};

/// Derive the per-chunk nonce: HKDF(file-key, "nonce", chunk_index_le_bytes) → 24 bytes.
pub fn chunk_nonce(file_key: &FileKey, chunk_index: u64) -> [u8; NONCE_LEN] {
    let info = chunk_index.to_le_bytes().to_vec();
    let key32 = derive_subkey_info(file_key.as_bytes(), LABEL_NONCE, &info)
        .expect("nonce derivation cannot fail with valid label");
    let mut n = [0u8; NONCE_LEN];
    n.copy_from_slice(&key32[..NONCE_LEN]);
    n
}

/// Build the chunk AAD: magic || version || file_id || chunk_index || total_chunks || total_length.
pub fn chunk_aad(
    file_id: &[u8; 16],
    chunk_index: u64,
    total_chunks: u64,
    total_length: u64,
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(4 + 4 + 16 + 8 + 8 + 8);
    aad.extend_from_slice(MAGIC);
    aad.extend_from_slice(&VERSION.to_le_bytes());
    aad.extend_from_slice(file_id);
    aad.extend_from_slice(&chunk_index.to_le_bytes());
    aad.extend_from_slice(&total_chunks.to_le_bytes());
    aad.extend_from_slice(&total_length.to_le_bytes());
    aad
}

/// Encrypt a chunk. Returns ciphertext || tag.
pub fn encrypt_chunk(
    file_key: &FileKey,
    file_id: &[u8; 16],
    chunk_index: u64,
    total_chunks: u64,
    total_length: u64,
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(file_key.as_bytes())
        .map_err(|e| FormatError::Crypto(e.to_string()))?;
    let nonce_bytes = chunk_nonce(file_key, chunk_index);
    let nonce = Array(nonce_bytes);

    let aad = chunk_aad(file_id, chunk_index, total_chunks, total_length);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                aad: aad.as_slice(),
                msg: plaintext,
            },
        )
        .map_err(|_| FormatError::ChunkAuth(chunk_index))?;
    Ok(ciphertext)
}

/// Decrypt a chunk (ciphertext || tag). Returns plaintext.
pub fn decrypt_chunk(
    file_key: &FileKey,
    file_id: &[u8; 16],
    chunk_index: u64,
    total_chunks: u64,
    total_length: u64,
    ciphertext_and_tag: &[u8],
) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(file_key.as_bytes())
        .map_err(|e| FormatError::Crypto(e.to_string()))?;
    let nonce_bytes = chunk_nonce(file_key, chunk_index);
    let nonce = Array(nonce_bytes);

    let aad = chunk_aad(file_id, chunk_index, total_chunks, total_length);
    let plaintext = cipher
        .decrypt(
            &nonce,
            Payload {
                aad: aad.as_slice(),
                msg: ciphertext_and_tag,
            },
        )
        .map_err(|_| FormatError::ChunkAuth(chunk_index))?;

    Ok(plaintext)
}

/// Plaintext length of chunk i given total_length and total_chunks.
pub fn chunk_plaintext_len(total_length: u64, total_chunks: u64, chunk_index: u64) -> usize {
    if chunk_index >= total_chunks {
        return 0;
    }
    let start = chunk_index * CHUNK_SIZE;
    if start >= total_length {
        return 0;
    }
    let end = ((chunk_index + 1) * CHUNK_SIZE).min(total_length);
    (end - start) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::mvk::Mvk;

    #[test]
    fn chunk_roundtrip() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        let plaintext = b"hello chunk world";
        let total_chunks = 1u64;
        let total_length = plaintext.len() as u64;

        let enc = encrypt_chunk(
            &file_key,
            &file_id,
            0,
            total_chunks,
            total_length,
            plaintext,
        )
        .expect("encrypt");
        let dec = decrypt_chunk(&file_key, &file_id, 0, total_chunks, total_length, &enc)
            .expect("decrypt");
        assert_eq!(&dec, plaintext);
    }

    #[test]
    fn chunk_tampered_ciphertext_rejected() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        let plaintext = vec![0x55u8; 100];
        let total_chunks = 1u64;
        let total_length = 100u64;

        let enc = encrypt_chunk(
            &file_key,
            &file_id,
            0,
            total_chunks,
            total_length,
            &plaintext,
        )
        .expect("encrypt");
        let mut tampered = enc.clone();
        tampered[NONCE_LEN] ^= 0x01; // flip a ciphertext byte

        let err = decrypt_chunk(
            &file_key,
            &file_id,
            0,
            total_chunks,
            total_length,
            &tampered,
        )
        .unwrap_err();
        assert!(matches!(err, FormatError::ChunkAuth(0)));
    }

    #[test]
    fn chunk_wrong_index_rejected() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        let plaintext = vec![0x55u8; 100];
        let total_chunks = 1u64;
        let total_length = 100u64;

        let enc = encrypt_chunk(
            &file_key,
            &file_id,
            0,
            total_chunks,
            total_length,
            &plaintext,
        )
        .expect("encrypt");
        // decrypt with wrong chunk index -> different nonce/AAD -> auth fail
        let err =
            decrypt_chunk(&file_key, &file_id, 1, total_chunks, total_length, &enc).unwrap_err();
        assert!(matches!(err, FormatError::ChunkAuth(1)));
    }
}
