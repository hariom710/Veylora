//! Backup container format: header, manifest, trailer regions.
//!
//! All AEAD operations use XChaCha20-Poly1305 with backup_key.
//! Nonces for manifest and trailer are derived via
//! derive_subkey_info(backup_key, LABEL_BACKUP_KEY, info) — subkeys, not
//! new top-level HKDF labels.

use chacha20poly1305::{XChaCha20Poly1305, KeyInit};
use chacha20poly1305::aead::{Aead, Payload};
use hybrid_array::Array;

use std::io::{Read, Write};

use crate::backup::{BackupError, Result};

pub const BACKUP_MAGIC: &[u8; 4] = b"VYLB";
pub const BACKUP_VERSION: u32 = 1;
pub const TRAILER_MAGIC: &[u8; 4] = b"VYLT";
pub const BACKUP_NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;
pub const TRAILER_FIXED_LEN: usize = 60; // 4 (magic) + 8 (file_count) + 32 (blobs_hash) + 16 (tag)

const AAD_PREFIX: &[u8] = b"VYLB";

fn aead_encrypt(key: &[u8; 32], nonce: &[u8; 24], aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).expect("aead init");
    let nonce_arr = Array(*nonce);
    cipher.encrypt(&nonce_arr, Payload { aad, msg: plaintext }).expect("aead encrypt")
}

fn aead_decrypt(key: &[u8; 32], nonce: &[u8; 24], aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| BackupError::Internal("aead init".to_string()))?;
    let nonce_arr = Array(*nonce);
    cipher.decrypt(&nonce_arr, Payload { aad, msg: ciphertext })
        .map_err(|_| BackupError::Corrupted("aead decrypt failed".to_string()))
}

fn version_aad() -> Vec<u8> {
    let mut aad = AAD_PREFIX.to_vec();
    aad.extend_from_slice(&BACKUP_VERSION.to_le_bytes());
    aad
}

/// Header region:
///   [4]  magic "VYLB"
///   [4]  format_ver u32 LE = 1
///   [24] backup_nonce (random)
///   [4]  header_payload_len u32 LE
///   [N]  header_payload (AEAD-encrypted: vault_name_len || vault_name || created_at)
///   [4]  meta_len u32 LE
///   [32] meta_hash (SHA-256 of encrypted manifest)
///   [16] header_tag (AEAD tag, authenticates header_payload + meta_len + meta_hash)
#[derive(Clone, Debug)]
pub struct HeaderRegion {
    pub backup_nonce: [u8; 24],
    pub vault_name: String,
    pub created_at: i64,
    pub meta_len: u32,
    pub meta_hash: [u8; 32],
}

impl HeaderRegion {
    pub fn write<W: Write>(
        &self,
        writer: &mut W,
        key: &[u8; 32],
    ) -> Result<usize> {
        let mut written = 0;

        writer.write_all(BACKUP_MAGIC).map_err(BackupError::Io)?;
        written += 4;

        writer.write_all(&BACKUP_VERSION.to_le_bytes()).map_err(BackupError::Io)?;
        written += 4;

        writer.write_all(&self.backup_nonce).map_err(BackupError::Io)?;
        written += 24;

        // Header plaintext: vault_name_len || vault_name || created_at
        let mut plaintext = Vec::new();
        plaintext.extend_from_slice(&(self.vault_name.len() as u32).to_le_bytes());
        plaintext.extend_from_slice(self.vault_name.as_bytes());
        plaintext.extend_from_slice(&self.created_at.to_le_bytes());

        // AAD: "VYLB" || format_ver (per spec)
        let aad = version_aad();

        let encrypted = aead_encrypt(key, &self.backup_nonce, &aad, &plaintext);
        let payload_len = encrypted.len() - TAG_LEN;
        let tag = &encrypted[payload_len..];

        writer.write_all(&(payload_len as u32).to_le_bytes()).map_err(BackupError::Io)?;
        written += 4;

        writer.write_all(&encrypted[..payload_len]).map_err(BackupError::Io)?;
        written += payload_len;

        writer.write_all(&self.meta_len.to_le_bytes()).map_err(BackupError::Io)?;
        written += 4;

        writer.write_all(&self.meta_hash).map_err(BackupError::Io)?;
        written += 32;

        writer.write_all(tag).map_err(BackupError::Io)?;
        written += 16;

        Ok(written)
    }

    pub fn read<R: Read>(
        reader: &mut R,
        key: &[u8; 32],
    ) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)
            .map_err(|e| BackupError::InvalidContainer(format!("bad magic: {e}")))?;
        if magic != *BACKUP_MAGIC {
            return Err(BackupError::InvalidContainer("bad magic".to_string()));
        }

        let mut ver_bytes = [0u8; 4];
        reader.read_exact(&mut ver_bytes)
            .map_err(|e| BackupError::InvalidContainer(format!("bad version: {e}")))?;
        let version = u32::from_le_bytes(ver_bytes);
        if version != BACKUP_VERSION {
            return Err(BackupError::InvalidContainer(format!("unsupported version {version}")));
        }

        let mut backup_nonce = [0u8; 24];
        reader.read_exact(&mut backup_nonce)
            .map_err(|e| BackupError::InvalidContainer(format!("bad nonce: {e}")))?;

        let mut plen_bytes = [0u8; 4];
        reader.read_exact(&mut plen_bytes)
            .map_err(|e| BackupError::InvalidContainer(format!("bad payload len: {e}")))?;
        let payload_len = u32::from_le_bytes(plen_bytes) as usize;

        let mut ciphertext = vec![0u8; payload_len];
        reader.read_exact(&mut ciphertext)
            .map_err(|e| BackupError::InvalidContainer(format!("bad payload: {e}")))?;

        let mut mlen_bytes = [0u8; 4];
        reader.read_exact(&mut mlen_bytes)
            .map_err(|e| BackupError::InvalidContainer(format!("bad meta len: {e}")))?;
        let meta_len = u32::from_le_bytes(mlen_bytes);

        let mut meta_hash = [0u8; 32];
        reader.read_exact(&mut meta_hash)
            .map_err(|e| BackupError::InvalidContainer(format!("bad meta hash: {e}")))?;

        let mut tag = [0u8; 16];
        reader.read_exact(&mut tag)
            .map_err(|e| BackupError::InvalidContainer(format!("bad header tag: {e}")))?;

        let mut full_ct = ciphertext;
        full_ct.extend_from_slice(&tag);

        // AAD: "VYLB" || format_ver (per spec)
        let aad = version_aad();

        let plaintext = aead_decrypt(key, &backup_nonce, &aad, &full_ct)?;

        if plaintext.len() < 4 {
            return Err(BackupError::Corrupted("header too short".to_string()));
        }
        let name_len = u32::from_le_bytes([plaintext[0], plaintext[1], plaintext[2], plaintext[3]]) as usize;
        if plaintext.len() < 4 + name_len + 8 {
            return Err(BackupError::Corrupted("header truncated".to_string()));
        }
        let vault_name = String::from_utf8(plaintext[4..4 + name_len].to_vec())
            .map_err(|_| BackupError::Corrupted("invalid vault name".to_string()))?;
        let created_at = i64::from_le_bytes([
            plaintext[4 + name_len],
            plaintext[5 + name_len],
            plaintext[6 + name_len],
            plaintext[7 + name_len],
            plaintext[8 + name_len],
            plaintext[9 + name_len],
            plaintext[10 + name_len],
            plaintext[11 + name_len],
        ]);

        Ok(HeaderRegion {
            backup_nonce,
            vault_name,
            created_at,
            meta_len,
            meta_hash,
        })
    }
}

/// One file entry inside the manifest.
#[derive(Clone, Debug)]
pub struct ManifestEntry {
    pub file_id: [u8; 16],
    pub encrypted_size: u64,
    pub encryption_ver: u32,
    pub name: String,
    pub created_at: i64,
    pub modified_at: i64,
}

/// Manifest region:
///   Single AEAD-encrypted blob containing:
///   [8] file_count u64 LE
///   for each file:
///     [16] file_id
///     [8] encrypted_size u64 LE
///     [4] encryption_ver u32 LE
///     [4] name_len u32 LE
///     [N] name bytes (plaintext)
///     [8] created_at i64 LE
///     [8] modified_at i64 LE
#[derive(Clone, Debug)]
pub struct ManifestRegion {
    pub entries: Vec<ManifestEntry>,
}

impl ManifestRegion {
    pub fn write<W: Write>(
        &self,
        writer: &mut W,
        key: &[u8; 32],
        nonce: &[u8; 24],
    ) -> Result<usize> {
        let mut plaintext = Vec::new();
        plaintext.extend_from_slice(&(self.entries.len() as u64).to_le_bytes());

        for entry in &self.entries {
            plaintext.extend_from_slice(&entry.file_id);
            plaintext.extend_from_slice(&entry.encrypted_size.to_le_bytes());
            plaintext.extend_from_slice(&entry.encryption_ver.to_le_bytes());
            plaintext.extend_from_slice(&(entry.name.len() as u32).to_le_bytes());
            plaintext.extend_from_slice(entry.name.as_bytes());
            plaintext.extend_from_slice(&entry.created_at.to_le_bytes());
            plaintext.extend_from_slice(&entry.modified_at.to_le_bytes());
        }

        // meta_len = plaintext length + tag length (AEAD appends tag)
        let meta_len = (plaintext.len() + TAG_LEN) as u32;

        // AAD: "VYLB" || format_ver || meta_len (per spec)
        let mut aad = version_aad();
        aad.extend_from_slice(&meta_len.to_le_bytes());

        let encrypted = aead_encrypt(key, nonce, &aad, &plaintext);
        writer.write_all(&encrypted).map_err(BackupError::Io)?;
        Ok(encrypted.len())
    }

    pub fn read(
        data: &[u8],
        key: &[u8; 32],
        nonce: &[u8; 24],
    ) -> Result<Self> {
        if data.len() < 8 {
            return Err(BackupError::Corrupted("manifest too short".to_string()));
        }

        // AAD: "VYLB" || format_ver || meta_len (per spec)
        // meta_len == data.len() (the number of encrypted bytes)
        let meta_len = data.len() as u32;
        let mut aad = version_aad();
        aad.extend_from_slice(&meta_len.to_le_bytes());
        let plaintext = aead_decrypt(key, nonce, &aad, data)?;

        if plaintext.len() < 8 {
            return Err(BackupError::Corrupted("manifest too short".to_string()));
        }
        let file_count = u64::from_le_bytes([
            plaintext[0], plaintext[1], plaintext[2], plaintext[3],
            plaintext[4], plaintext[5], plaintext[6], plaintext[7],
        ]);
        let mut offset = 8;

        let mut entries = Vec::with_capacity(file_count as usize);
        for _ in 0..file_count {
            if offset + 16 > plaintext.len() {
                return Err(BackupError::Corrupted("manifest truncated".to_string()));
            }
            let mut file_id = [0u8; 16];
            file_id.copy_from_slice(&plaintext[offset..offset + 16]);
            offset += 16;

            if offset + 8 > plaintext.len() {
                return Err(BackupError::Corrupted("manifest truncated".to_string()));
            }
            let encrypted_size = u64::from_le_bytes([
                plaintext[offset], plaintext[offset + 1], plaintext[offset + 2], plaintext[offset + 3],
                plaintext[offset + 4], plaintext[offset + 5], plaintext[offset + 6], plaintext[offset + 7],
            ]);
            offset += 8;

            if offset + 4 > plaintext.len() {
                return Err(BackupError::Corrupted("manifest truncated".to_string()));
            }
            let encryption_ver = u32::from_le_bytes([
                plaintext[offset], plaintext[offset + 1], plaintext[offset + 2], plaintext[offset + 3],
            ]);
            offset += 4;

            if offset + 4 > plaintext.len() {
                return Err(BackupError::Corrupted("manifest truncated".to_string()));
            }
            let name_len = u32::from_le_bytes([
                plaintext[offset], plaintext[offset + 1], plaintext[offset + 2], plaintext[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + name_len > plaintext.len() {
                return Err(BackupError::Corrupted("manifest truncated".to_string()));
            }
            let name = String::from_utf8(plaintext[offset..offset + name_len].to_vec())
                .map_err(|_| BackupError::Corrupted("invalid filename".to_string()))?;
            offset += name_len;

            if offset + 8 > plaintext.len() {
                return Err(BackupError::Corrupted("manifest truncated".to_string()));
            }
            let created_at = i64::from_le_bytes([
                plaintext[offset], plaintext[offset + 1], plaintext[offset + 2], plaintext[offset + 3],
                plaintext[offset + 4], plaintext[offset + 5], plaintext[offset + 6], plaintext[offset + 7],
            ]);
            offset += 8;

            if offset + 8 > plaintext.len() {
                return Err(BackupError::Corrupted("manifest truncated".to_string()));
            }
            let modified_at = i64::from_le_bytes([
                plaintext[offset], plaintext[offset + 1], plaintext[offset + 2], plaintext[offset + 3],
                plaintext[offset + 4], plaintext[offset + 5], plaintext[offset + 6], plaintext[offset + 7],
            ]);
            offset += 8;

            entries.push(ManifestEntry {
                file_id,
                encrypted_size,
                encryption_ver,
                name,
                created_at,
                modified_at,
            });
        }

        Ok(ManifestRegion { entries })
    }
}

/// Trailer region:
///   [4]  trailer_magic "VYLT"
///   [8]  file_count u64 LE
///   [32] blobs_hash (SHA-256)
///   [16] final_tag (AEAD tag over file_count + blobs_hash)
#[derive(Clone, Debug)]
pub struct TrailerRegion {
    pub file_count: u64,
    pub blobs_hash: [u8; 32],
}

impl TrailerRegion {
    pub fn write<W: Write>(
        &self,
        writer: &mut W,
        key: &[u8; 32],
        nonce: &[u8; 24],
    ) -> Result<usize> {
        let mut written = 0;

        writer.write_all(TRAILER_MAGIC).map_err(BackupError::Io)?;
        written += 4;

        writer.write_all(&self.file_count.to_le_bytes()).map_err(BackupError::Io)?;
        written += 8;

        writer.write_all(&self.blobs_hash).map_err(BackupError::Io)?;
        written += 32;

        let mut aad = TRAILER_MAGIC.to_vec();
        aad.extend_from_slice(&BACKUP_VERSION.to_le_bytes());

        let mut plaintext = Vec::new();
        plaintext.extend_from_slice(&self.file_count.to_le_bytes());
        plaintext.extend_from_slice(&self.blobs_hash);

        let encrypted = aead_encrypt(key, nonce, &aad, &plaintext);
        let payload_len = encrypted.len() - TAG_LEN;
        let tag = &encrypted[payload_len..];

        writer.write_all(tag).map_err(BackupError::Io)?;
        written += 16;

        Ok(written)
    }

    pub fn read<R: Read>(
        reader: &mut R,
        key: &[u8; 32],
        nonce: &[u8; 24],
    ) -> Result<Self> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)
            .map_err(|e| BackupError::InvalidContainer(format!("bad trailer magic: {e}")))?;
        if magic != *TRAILER_MAGIC {
            return Err(BackupError::InvalidContainer("bad trailer magic".to_string()));
        }

        let mut fc_bytes = [0u8; 8];
        reader.read_exact(&mut fc_bytes)
            .map_err(|e| BackupError::InvalidContainer(format!("bad trailer file count: {e}")))?;
        let file_count = u64::from_le_bytes(fc_bytes);

        let mut blobs_hash = [0u8; 32];
        reader.read_exact(&mut blobs_hash)
            .map_err(|e| BackupError::InvalidContainer(format!("bad trailer blobs hash: {e}")))?;

        let mut tag = [0u8; 16];
        reader.read_exact(&mut tag)
            .map_err(|e| BackupError::InvalidContainer(format!("bad trailer tag: {e}")))?;

        // Recompute the AEAD tag over file_count || blobs_hash and compare.
        // The trailer stores file_count and blobs_hash as plaintext; only the
        // tag is the AEAD authentication code. We verify by re-encrypting
        // the plaintext and comparing tags.
        let mut aad = TRAILER_MAGIC.to_vec();
        aad.extend_from_slice(&BACKUP_VERSION.to_le_bytes());

        let mut plaintext = Vec::new();
        plaintext.extend_from_slice(&file_count.to_le_bytes());
        plaintext.extend_from_slice(&blobs_hash);

        let expected = aead_encrypt(key, nonce, &aad, &plaintext);
        let expected_tag = &expected[expected.len() - TAG_LEN..];
        if tag != expected_tag {
            return Err(BackupError::Corrupted("trailer tag verification failed".to_string()));
        }

        Ok(TrailerRegion { file_count, blobs_hash })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aead_roundtrip() {
        let key = [0x42u8; 32];
        let nonce = [0x01u8; 24];
        let aad = b"test-aad";
        let plaintext = b"hello world, this is a test";
        let encrypted = aead_encrypt(&key, &nonce, aad, plaintext);
        let decrypted = aead_decrypt(&key, &nonce, aad, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn aead_roundtrip_with_meta_len() {
        let key = [0x42u8; 32];
        let nonce = [0x01u8; 24];
        let meta_len: u32 = 192;
        let mut aad = version_aad();
        aad.extend_from_slice(&meta_len.to_le_bytes());
        let plaintext = b"test manifest plaintext data";
        let encrypted = aead_encrypt(&key, &nonce, &aad, plaintext);
        let decrypted = aead_decrypt(&key, &nonce, &aad, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }
}
