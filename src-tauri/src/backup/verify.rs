//! Backup verification: check container integrity without restoring.
//!
//! Verifies header AEAD, manifest hash, and trailer blobs_hash.
//!
//! When MVK is provided: full verification (header AEAD + manifest meta_hash +
//! blobs_hash + trailer).
//! When MVK is None (session locked): partial verification — verifies trailer
//! structure and blobs_hash over the blob region (meta_len is plaintext in the
//! header, so we can locate the blob region without decrypting).
//!
//! Returns `verified_fields` listing which regions were cryptographically
//! verified. Always includes "blobs_hash" when trailer is readable;
//! includes "manifest" only when MVK decrypts the manifest AEAD.

use std::io::Read;

use crate::backup::{
    container, BackupError, BackupInfo, Result,
};
use crate::crypto::hkdf::LABEL_BACKUP_KEY;
use sha2::{Digest, Sha256};
use crate::database::Db;

/// Verify a backup container's integrity.
///
/// If `mvk` is provided (session unlocked), performs full verification:
/// decrypts header + manifest, verifies meta_hash, validates blobs_hash.
///
/// If `mvk` is None (session locked), performs partial verification:
/// verifies trailer structure (magic, file_count, blobs_hash, tag structure)
/// and computes blobs_hash over the blob region using meta_len (plaintext in
/// header) to locate the blob region boundary. Cannot decrypt manifest.
pub fn verify_backup(
    mvk: Option<&[u8; 32]>,
    backup_path: &str,
) -> Result<BackupInfo> {
    let data = std::fs::read(backup_path).map_err(|e| BackupError::Io(e))?;
    if data.len() < container::TRAILER_FIXED_LEN {
        return Err(BackupError::InvalidContainer("file too small".to_string()));
    }

    let trailer_start = data.len() - container::TRAILER_FIXED_LEN;
    let trailer_data = &data[trailer_start..];

    if let Some(mvk) = mvk {
        // Full verification with MVK.
        let backup_key = crate::crypto::hkdf::derive_subkey(mvk, LABEL_BACKUP_KEY)
            .map_err(|e| BackupError::Internal(e.to_string()))?;

        // Read and decrypt header.
        let mut cursor = std::io::Cursor::new(&data);
        let header = container::HeaderRegion::read(&mut cursor, &backup_key)?;

        // Read encrypted manifest.
        let mut manifest_encrypted = vec![0u8; header.meta_len as usize];
        cursor.read_exact(&mut manifest_encrypted).map_err(|e| {
            BackupError::InvalidContainer(format!("bad manifest read: {e}"))
        })?;

        // Verify meta_hash.
        let mut hash = Sha256::new();
        hash.update(&manifest_encrypted);
        let actual_hash: [u8; 32] = hash.finalize().into();
        if actual_hash != header.meta_hash {
            return Err(BackupError::Corrupted("manifest hash mismatch".to_string()));
        }

        // Verify trailer.
        let trailer_nonce = derive_backup_nonce(&backup_key, b"trailer-nonce")?;
        let trailer = container::TrailerRegion::read(
            &mut &trailer_data[..],
            &backup_key,
            &trailer_nonce,
        )?;

        // Verify blobs_hash by streaming through blob region.
        let manifest_end = cursor.position() as usize;
        let mut blobs_hasher = Sha256::new();
        for i in manifest_end..trailer_start {
            blobs_hasher.update(&[data[i]]);
        }
        let actual_blobs_hash: [u8; 32] = blobs_hasher.finalize().into();
        if actual_blobs_hash != trailer.blobs_hash {
            return Err(BackupError::Corrupted("blobs hash mismatch".to_string()));
        }

        Ok(BackupInfo {
            path: backup_path.to_string(),
            file_count: trailer.file_count,
            total_size: 0, // unknown without full manifest decryption
            created_at_unix: header.created_at,
            verified_fields: vec!["manifest".to_string(), "blobs_hash".to_string()],
        })
    } else {
        // Partial verification without MVK.
        // We can't decrypt the header (no MVK), so we can't verify header AEAD
        // or manifest contents. But meta_len is plaintext in the header, so we
        // can locate the blob region and verify blobs_hash.
        let mut pos: usize = 0;

        // Read header fixed portion: magic(4) + version(4) + nonce(24) + payload_len(4)
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&data[pos..pos + 4]);
        pos += 4;
        if magic != *container::BACKUP_MAGIC {
            return Err(BackupError::InvalidContainer("bad magic".to_string()));
        }

        let mut ver_bytes = [0u8; 4];
        ver_bytes.copy_from_slice(&data[pos..pos + 4]);
        pos += 4;
        let _version = u32::from_le_bytes(ver_bytes);

        let mut nonce_bytes = [0u8; 24];
        nonce_bytes.copy_from_slice(&data[pos..pos + 24]);
        pos += 24;

        let mut plen_bytes = [0u8; 4];
        plen_bytes.copy_from_slice(&data[pos..pos + 4]);
        pos += 4;
        let payload_len = u32::from_le_bytes(plen_bytes) as usize;

        // Skip encrypted header payload (N bytes).
        if pos + payload_len > trailer_start {
            return Err(BackupError::InvalidContainer("header payload overlaps trailer".to_string()));
        }
        pos += payload_len;

        // Read meta_len (4 bytes, plaintext).
        let mut mlen_bytes = [0u8; 4];
        mlen_bytes.copy_from_slice(&data[pos..pos + 4]);
        pos += 4;
        let meta_len = u32::from_le_bytes(mlen_bytes) as usize;

        // Skip meta_hash(32) + header_tag(16).
        if pos + 32 + 16 > trailer_start {
            return Err(BackupError::InvalidContainer("header tail overlaps trailer".to_string()));
        }
        pos += 32 + 16;

        // Manifest region: meta_len bytes after the header tail.
        let manifest_start = pos;
        let manifest_end = manifest_start + meta_len;
        if manifest_end > trailer_start {
            return Err(BackupError::InvalidContainer("manifest overlaps trailer".to_string()));
        }

        // Verify trailer structure.
        let mut trailer_cursor = &mut &trailer_data[..];

        let mut trailer_magic = [0u8; 4];
        trailer_cursor.read_exact(&mut trailer_magic)
            .map_err(|e| BackupError::InvalidContainer(format!("bad trailer magic: {e}")))?;
        if trailer_magic != *container::TRAILER_MAGIC {
            return Err(BackupError::InvalidContainer("bad trailer magic".to_string()));
        }

        let mut fc_bytes = [0u8; 8];
        trailer_cursor.read_exact(&mut fc_bytes)
            .map_err(|e| BackupError::InvalidContainer(format!("bad trailer file count: {e}")))?;
        let file_count = u64::from_le_bytes(fc_bytes);

        let mut blobs_hash = [0u8; 32];
        trailer_cursor.read_exact(&mut blobs_hash)
            .map_err(|e| BackupError::InvalidContainer(format!("bad trailer blobs hash: {e}")))?;

        let mut tag = [0u8; 16];
        trailer_cursor.read_exact(&mut tag)
            .map_err(|e| BackupError::InvalidContainer(format!("bad trailer tag: {e}")))?;

        // Verify blobs_hash over the blob region.
        // Blob region = [manifest_end, trailer_start).
        let mut blobs_hasher = Sha256::new();
        for i in manifest_end..trailer_start {
            blobs_hasher.update(&[data[i]]);
        }
        let actual_blobs_hash: [u8; 32] = blobs_hasher.finalize().into();
        if actual_blobs_hash != blobs_hash {
            return Err(BackupError::Corrupted("blobs hash mismatch".to_string()));
        }

        Ok(BackupInfo {
            path: backup_path.to_string(),
            file_count,
            total_size: 0, // unknown without MVK
            created_at_unix: 0, // unknown without MVK
            verified_fields: vec!["blobs_hash".to_string()],
        })
    }
}

fn derive_backup_nonce(backup_key: &[u8; 32], info: &[u8]) -> Result<[u8; 24]> {
    let subkey = crate::crypto::hkdf::derive_subkey_info(backup_key, LABEL_BACKUP_KEY, info)
        .map_err(|e| BackupError::Internal(e.to_string()))?;
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&subkey[..24]);
    Ok(nonce)
}

/// List backup history from the database.
pub fn list_backup_history(db: &Db) -> Result<Vec<crate::backup::BackupHistoryEntry>> {
    let mut stmt = db.conn().prepare(
        "SELECT backup_path, status, created_at FROM backup_history ORDER BY created_at DESC",
    ).map_err(|e| BackupError::Internal(e.to_string()))?;

    let rows = stmt.query_map([], |row| {
        Ok(crate::backup::BackupHistoryEntry {
            path: row.get(0)?,
            status: row.get(1)?,
            created_at_unix: row.get(2)?,
        })
    }).map_err(|e| BackupError::Internal(e.to_string()))?;

    let mut entries = Vec::new();
    for r in rows {
        entries.push(r.map_err(|e| BackupError::Internal(e.to_string()))?);
    }
    Ok(entries)
}

/// Record a backup completion in the history table.
pub fn record_backup_history(
    db: &Db,
    path: &str,
    _status: &str,
    created_at_unix: i64,
) -> Result<()> {
    db.conn().execute(
        "INSERT INTO backup_history (created_at, backup_path, encrypted_size, verified, status)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![created_at_unix, path, 0, 0, _status],
    ).map_err(|e| BackupError::Internal(e.to_string()))?;
    Ok(())
}
