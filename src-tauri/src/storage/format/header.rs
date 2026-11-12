//! Header read/write with AEAD.
//!
//! Header layout (on disk):
//!   magic            "VYLR"          [4]
//!   format_version   u32 LE = 1      [4]
//!   file_id          [16]
//!   chunk_size       u32 LE = 65536  [4]
//!   total_chunks     u64 LE          [8]
//!   total_length     u64 LE          [8]
//!   header_nonce     [24]
//!   header_ciphertext [N]            AEAD over header core (32 bytes)
//!   header_tag       [16]
//!
//! Header core (encrypted)   = file_id || chunk_size || total_chunks || total_length
//! Header AAD (authenticated) = "VYLR" || format_version_LE

use chacha20poly1305::aead::{Aead, Payload};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use hybrid_array::Array;
use typenum::U24;

use crate::crypto::hkdf::derive_subkey;
use crate::storage::format::{
    FileKey, FormatError, Result, CHUNK_SIZE, HEADER_CORE_LEN, MAGIC, NONCE_LEN, TAG_LEN, VERSION,
};

pub(crate) const HEADER_FIXED_LEN: usize = 4 + 4 + 16 + 4 + 8 + 8; // before nonce
type AeadNonce = Array<u8, U24>;

#[derive(Debug, Clone)]
pub struct Header {
    pub file_id: [u8; 16],
    pub chunk_size: u32,
    pub total_chunks: u64,
    pub total_length: u64,
}

impl Header {
    pub fn new(file_id: [u8; 16], total_chunks: u64, total_length: u64) -> Self {
        Self {
            file_id,
            chunk_size: CHUNK_SIZE as u32,
            total_chunks,
            total_length,
        }
    }

    /// Serialize the header core (encrypted portion): file_id || chunk_size || total_chunks || total_length.
    pub fn core_bytes(&self) -> [u8; HEADER_CORE_LEN] {
        let mut buf = [0u8; HEADER_CORE_LEN];
        buf[0..16].copy_from_slice(&self.file_id);
        buf[16..20].copy_from_slice(&self.chunk_size.to_le_bytes());
        buf[20..28].copy_from_slice(&self.total_chunks.to_le_bytes());
        buf[28..36].copy_from_slice(&self.total_length.to_le_bytes());
        buf
    }

    /// Serialize the fixed header portion (before nonce): magic || version || file_id || chunk_size || total_chunks || total_length.
    pub fn fixed_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_FIXED_LEN);
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&VERSION.to_le_bytes());
        buf.extend_from_slice(&self.file_id);
        buf.extend_from_slice(&self.chunk_size.to_le_bytes());
        buf.extend_from_slice(&self.total_chunks.to_le_bytes());
        buf.extend_from_slice(&self.total_length.to_le_bytes());
        buf
    }

    /// Header AAD: "VYLR" || format_version_LE.
    pub fn aad(&self) -> Vec<u8> {
        let mut aad = Vec::with_capacity(8);
        aad.extend_from_slice(MAGIC);
        aad.extend_from_slice(&VERSION.to_le_bytes());
        aad
    }
}

/// Write header: fixed bytes || nonce || ciphertext(32) || tag(16).
/// Returns total bytes written.
pub fn write_header(
    header: &Header,
    file_key: &FileKey,
    dest: &mut dyn std::io::Write,
) -> Result<usize> {
    let cipher = XChaCha20Poly1305::new_from_slice(file_key.as_bytes())
        .map_err(|e| FormatError::Crypto(e.to_string()))?;

    let nonce_bytes = derive_subkey(file_key.as_bytes(), crate::crypto::hkdf::LABEL_HEADER_NONCE)?;
    let mut nonce = AeadNonce::default();
    nonce
        .as_mut_slice()
        .copy_from_slice(&nonce_bytes[..NONCE_LEN]);

    let core = header.core_bytes();

    let aad = header.aad();
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                aad: aad.as_slice(),
                msg: &core,
            },
        )
        .map_err(|_| FormatError::HeaderAuth)?;

    let fixed = header.fixed_bytes();
    dest.write_all(&fixed)
        .map_err(|e| FormatError::Io(e.to_string()))?;
    dest.write_all(&nonce_bytes[..NONCE_LEN])
        .map_err(|e| FormatError::Io(e.to_string()))?;
    dest.write_all(&ciphertext)
        .map_err(|e| FormatError::Io(e.to_string()))?;

    Ok(fixed.len() + NONCE_LEN + ciphertext.len())
}

/// Read and authenticate header from a byte slice.
pub fn read_header(file: &[u8], file_key: &FileKey) -> Result<(Header, usize)> {
    if file.len() < HEADER_FIXED_LEN + NONCE_LEN + HEADER_CORE_LEN + TAG_LEN {
        return Err(FormatError::TooSmall);
    }

    if &file[0..4] != MAGIC {
        return Err(FormatError::BadMagic);
    }
    let version = u32::from_le_bytes(file[4..8].try_into().unwrap());
    if version != VERSION {
        return Err(FormatError::UnsupportedVersion(version));
    }

    let file_id: [u8; 16] = file[8..24].try_into().unwrap();
    let chunk_size = u32::from_le_bytes(file[24..28].try_into().unwrap());
    let total_chunks = u64::from_le_bytes(file[28..36].try_into().unwrap());
    let total_length = u64::from_le_bytes(file[36..44].try_into().unwrap());

    let header_nonce_start = HEADER_FIXED_LEN;
    let header_nonce = &file[header_nonce_start..header_nonce_start + NONCE_LEN];
    let ct_start = header_nonce_start + NONCE_LEN;
    let ct_end = ct_start + HEADER_CORE_LEN + TAG_LEN;
    let header_ct = &file[ct_start..ct_end];

    let cipher = XChaCha20Poly1305::new_from_slice(file_key.as_bytes())
        .map_err(|e| FormatError::Crypto(e.to_string()))?;
    let mut nonce = AeadNonce::default();
    nonce.as_mut_slice().copy_from_slice(header_nonce);

    let aad = {
        let mut a = Vec::with_capacity(8);
        a.extend_from_slice(MAGIC);
        a.extend_from_slice(&VERSION.to_le_bytes());
        a
    };
    let plaintext = cipher
        .decrypt(
            &nonce,
            Payload {
                aad: aad.as_slice(),
                msg: header_ct,
            },
        )
        .map_err(|_| FormatError::HeaderAuth)?;

    if plaintext.len() != HEADER_CORE_LEN {
        return Err(FormatError::HeaderAuth);
    }

    // Verify decrypted core matches the plaintext fields in the header.
    let expected_core = {
        let mut buf = [0u8; HEADER_CORE_LEN];
        buf[0..16].copy_from_slice(&file_id);
        buf[16..20].copy_from_slice(&chunk_size.to_le_bytes());
        buf[20..28].copy_from_slice(&total_chunks.to_le_bytes());
        buf[28..36].copy_from_slice(&total_length.to_le_bytes());
        buf
    };
    if plaintext.as_slice() != expected_core.as_slice() {
        return Err(FormatError::HeaderAuth);
    }

    let header = Header::new(file_id, total_chunks, total_length);
    Ok((header, ct_end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::mvk::Mvk;

    #[test]
    fn header_roundtrip() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive file key");
        let header = Header::new(file_id, 3, 200000);

        let mut buf = Vec::new();
        let written = write_header(&header, &file_key, &mut buf).expect("write header");
        assert!(written > 0);

        let (read_header, _) = read_header(&buf, &file_key).expect("read header");
        assert_eq!(read_header.file_id, file_id);
        assert_eq!(read_header.total_chunks, 3);
        assert_eq!(read_header.total_length, 200000);
        assert_eq!(read_header.chunk_size, CHUNK_SIZE as u32);
    }

    #[test]
    fn header_bad_magic_rejected() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive file key");
        let header = Header::new(file_id, 1, 100);

        let mut buf = Vec::new();
        write_header(&header, &file_key, &mut buf).expect("write header");
        buf[0] ^= 0xFF; // tamper magic

        let err = read_header(&buf, &file_key).unwrap_err();
        assert!(matches!(err, FormatError::BadMagic));
    }

    #[test]
    fn header_tampered_ciphertext_rejected() {
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive file key");
        let header = Header::new(file_id, 1, 100);

        let mut buf = Vec::new();
        write_header(&header, &file_key, &mut buf).expect("write header");
        // tamper a byte in the ciphertext region
        let ct_start = HEADER_FIXED_LEN + NONCE_LEN;
        buf[ct_start] ^= 0x01;

        let err = read_header(&buf, &file_key).unwrap_err();
        assert!(matches!(err, FormatError::HeaderAuth));
    }
}
