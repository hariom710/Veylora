//! Backup restore: extract a .vey container into an unlocked vault.
//!
//! All-or-nothing: on any failure, DB rows and blobs written during this
//! restore are removed. Uses a rusqlite transaction for DB writes.

use std::io::Read;

use crate::backup::{
    container, BackupError, RestoreInfo, Result,
};

use sha2::{Digest, Sha256};

use crate::crypto::hkdf::{LABEL_BACKUP_KEY, LABEL_METADATA_KEY};
use crate::crypto::mvk::Mvk;
use crate::database::Db;
use rusqlite::OptionalExtension;
use crate::storage::blob::write_blob;
use crate::storage::format::{FileKey, verify_file};
use crate::storage::vault_dir::VaultDir;

/// Derive backup_key = HKDF(MVK, "veylora/v1/backup-key").
fn derive_backup_key(mvk: &[u8; 32]) -> Result<[u8; 32]> {
    crate::crypto::hkdf::derive_subkey(mvk, LABEL_BACKUP_KEY)
        .map_err(|e| BackupError::Internal(e.to_string()))
}

/// Derive a 24-byte nonce from backup_key using subkey derivation with `info`.
fn derive_backup_nonce(backup_key: &[u8; 32], info: &[u8]) -> Result<[u8; 24]> {
    let subkey = crate::crypto::hkdf::derive_subkey_info(backup_key, LABEL_BACKUP_KEY, info)
        .map_err(|e| BackupError::Internal(e.to_string()))?;
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&subkey[..24]);
    Ok(nonce)
}

/// Restore a backup container into the currently unlocked vault.
///
/// Requires an unlocked session (MVK). On success, all files from the
/// backup are written as blobs and DB rows are inserted. On failure,
/// all changes are rolled back (all-or-nothing).
pub fn restore_backup(
    mvk: &[u8; 32],
    db: &mut Db,
    vdir: &VaultDir,
    backup_path: &str,
) -> Result<RestoreInfo> {
    let backup_key = derive_backup_key(mvk)?;
    let metadata_key = crate::crypto::hkdf::derive_subkey(mvk, LABEL_METADATA_KEY)
        .map_err(|e| BackupError::Internal(e.to_string()))?;

    // Read the entire backup file.
    let data = std::fs::read(backup_path).map_err(|e| BackupError::Io(e))?;
    if data.len() < container::TRAILER_FIXED_LEN {
        return Err(BackupError::InvalidContainer("file too small".to_string()));
    }

    let mut cursor = std::io::Cursor::new(&data);

    // 1. Read and decrypt header.
    let header = container::HeaderRegion::read(&mut cursor, &backup_key)?;

    // 2. Read encrypted manifest and verify meta_hash.
    let mut manifest_encrypted = vec![0u8; header.meta_len as usize];
    cursor.read_exact(&mut manifest_encrypted).map_err(|e| {
        BackupError::InvalidContainer(format!("bad manifest read: {e}"))
    })?;

    let mut hash = Sha256::new();
    hash.update(&manifest_encrypted);
    let actual_hash: [u8; 32] = hash.finalize().into();
    if actual_hash != header.meta_hash {
        return Err(BackupError::Corrupted("manifest hash mismatch".to_string()));
    }

    // 3. Decrypt manifest.
    let manifest_nonce = derive_backup_nonce(&backup_key, b"manifest-nonce")?;
    let manifest = container::ManifestRegion::read(
        &manifest_encrypted,
        &backup_key,
        &manifest_nonce,
    )?;

    // 4. Determine blob region bounds.
    let manifest_end = cursor.position() as usize;
    let trailer_start = data.len() - container::TRAILER_FIXED_LEN;
    if trailer_start < manifest_end {
        return Err(BackupError::Corrupted("manifest overlaps trailer".to_string()));
    }

    // 5. Start a DB transaction for all-or-nothing restore.
    let mut files_restored: u64 = 0;
    let mut bytes_written: u64 = 0;
    let mut restored_file_ids: Vec<String> = Vec::new();
    let mut restored_blob_paths: Vec<std::path::PathBuf> = Vec::new();

    let tx = db.conn_mut().transaction().map_err(|e| BackupError::Internal(e.to_string()))?;

    for entry in &manifest.entries {
        // Check for duplicate file_id.
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM encrypted_files WHERE file_id = ?1",
                rusqlite::params![hex_encode(&entry.file_id)],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| BackupError::Internal(e.to_string()))?;
        if existing.is_some() {
            return Err(BackupError::FileExists {
                file_id: hex_encode(&entry.file_id),
            });
        }

        // Read encrypted blob bytes from the backup container.
        let mut blob_data = vec![0u8; entry.encrypted_size as usize];
        cursor
            .read_exact(&mut blob_data)
            .map_err(|e| BackupError::InvalidContainer(format!("bad blob read: {e}")))?;

        // Verify the blob with format v1 (validates chunk chain + finalize).
        let file_key = FileKey::derive(
            &Mvk::from_array(*mvk),
            &entry.file_id,
        )
        .map_err(|e| BackupError::Internal(e.to_string()))?;
        verify_file(&blob_data, &file_key).map_err(|e| {
            BackupError::Corrupted(format!("blob verification failed for {}: {e}", hex_encode(&entry.file_id)))
        })?;

        // Write blob atomically.
        let encrypted_size = write_blob(&vdir.root, &entry.file_id, &mut blob_data.as_slice(), &file_key)
            .map_err(|e| BackupError::Internal(e.to_string()))?;
        bytes_written += encrypted_size;

        let blob_path = vdir.blob_path(&entry.file_id);
        restored_blob_paths.push(blob_path);

        // Insert DB row.
        // Encrypt filename with metadata-key for the new row_id.
        // We insert with row_id 0 first, then re-encrypt with the real row_id.
        let (fn_nonce, fn_cipher) = crate::database::crypto::encrypt_meta(
            &metadata_key,
            "encrypted_files",
            0,
            "filename",
            entry.name.as_bytes(),
        )
        .map_err(|e| BackupError::Internal(e.to_string()))?;
        let (ca_nonce, ca_cipher) = crate::database::crypto::encrypt_meta(
            &metadata_key,
            "encrypted_files",
            0,
            "created_at",
            entry.created_at.to_string().as_bytes(),
        )
        .map_err(|e| BackupError::Internal(e.to_string()))?;
        let (ma_nonce, ma_cipher) = crate::database::crypto::encrypt_meta(
            &metadata_key,
            "encrypted_files",
            0,
            "modified_at",
            entry.modified_at.to_string().as_bytes(),
        )
        .map_err(|e| BackupError::Internal(e.to_string()))?;

        tx.execute(
            "INSERT INTO encrypted_files (vault_id, file_id, filename_nonce, filename_ciphertext,
                folder_id, encrypted_size, encryption_version,
                created_at_nonce, created_at_ciphertext, modified_at_nonce, modified_at_ciphertext)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                1, // vault_id
                hex_encode(&entry.file_id),
                fn_nonce,
                fn_cipher,
                None::<i64>, // folder_id
                entry.encrypted_size as i64,
                entry.encryption_ver,
                ca_nonce,
                ca_cipher,
                ma_nonce,
                ma_cipher,
            ],
        )
        .map_err(|e| BackupError::Internal(e.to_string()))?;

        // Get the new row id and re-encrypt with correct row_id in AAD.
        let id: i64 = tx
            .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
            .map_err(|e| BackupError::Internal(e.to_string()))?;

        let (fn_nonce, fn_cipher) = crate::database::crypto::encrypt_meta(
            &metadata_key,
            "encrypted_files",
            id as u64,
            "filename",
            entry.name.as_bytes(),
        )
        .map_err(|e| BackupError::Internal(e.to_string()))?;
        let (ca_nonce, ca_cipher) = crate::database::crypto::encrypt_meta(
            &metadata_key,
            "encrypted_files",
            id as u64,
            "created_at",
            entry.created_at.to_string().as_bytes(),
        )
        .map_err(|e| BackupError::Internal(e.to_string()))?;
        let (ma_nonce, ma_cipher) = crate::database::crypto::encrypt_meta(
            &metadata_key,
            "encrypted_files",
            id as u64,
            "modified_at",
            entry.modified_at.to_string().as_bytes(),
        )
        .map_err(|e| BackupError::Internal(e.to_string()))?;

        tx.execute(
            "UPDATE encrypted_files
             SET filename_nonce = ?2, filename_ciphertext = ?3,
                 created_at_nonce = ?4, created_at_ciphertext = ?5,
                 modified_at_nonce = ?6, modified_at_ciphertext = ?7
             WHERE id = ?1",
            rusqlite::params![
                id,
                fn_nonce,
                fn_cipher,
                ca_nonce,
                ca_cipher,
                ma_nonce,
                ma_cipher,
            ],
        )
        .map_err(|e| BackupError::Internal(e.to_string()))?;

        restored_file_ids.push(hex_encode(&entry.file_id));
        files_restored += 1;
    }

    // 6. Verify trailer.
    let trailer_data = &data[trailer_start..];
    let trailer_nonce = derive_backup_nonce(&backup_key, b"trailer-nonce")?;
    let trailer = container::TrailerRegion::read(
        &mut &trailer_data[..],
        &backup_key,
        &trailer_nonce,
    )?;

    if trailer.file_count != manifest.entries.len() as u64 {
        return Err(BackupError::Corrupted("trailer file count mismatch".to_string()));
    }

    // Commit the transaction.
    tx.commit().map_err(|e| BackupError::Internal(e.to_string()))?;

    Ok(RestoreInfo {
        files_restored,
        bytes_written,
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

