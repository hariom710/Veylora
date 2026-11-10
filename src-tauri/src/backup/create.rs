//! Backup creation: build a .vey container from an unlocked vault.

use std::io::Write;
use std::path::Path;

use crate::backup::{container, BackupError, BackupInfo, Result};
use crate::backup::container::ManifestEntry;
use sha2::Sha256;
use crate::database::Db;
use crate::storage::vault_dir::VaultDir;

/// Derive backup_key = HKDF(MVK, "veylora/v1/backup-key").
fn derive_backup_key(mvk: &[u8; 32]) -> Result<[u8; 32]> {
    crate::crypto::hkdf::derive_subkey(mvk, crate::crypto::hkdf::LABEL_BACKUP_KEY)
        .map_err(|e| BackupError::Internal(e.to_string()))
}

/// Derive a 24-byte nonce from backup_key using subkey derivation with `info`.
fn derive_backup_nonce(backup_key: &[u8; 32], info: &[u8]) -> Result<[u8; 24]> {
    let subkey = crate::crypto::hkdf::derive_subkey_info(backup_key, crate::crypto::hkdf::LABEL_BACKUP_KEY, info)
        .map_err(|e| BackupError::Internal(e.to_string()))?;
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&subkey[..24]);
    Ok(nonce)
}

fn hex_decode(hex: &str) -> Option<[u8; 16]> {
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for i in 0..16 {
        bytes[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

fn parse_i64(s: &str) -> Result<i64> {
    s.parse::<i64>()
        .map_err(|_| BackupError::Internal(format!("invalid timestamp: {s}")))
}

/// Create a backup container at `dest_path` from the currently unlocked vault.
///
/// The backup includes all file metadata and the raw encrypted blob contents.
/// Blobs are NOT re-encrypted — they are the already-encrypted format v1 files.
pub fn create_backup(
    mvk: &[u8; 32],
    db: &Db,
    vdir: &VaultDir,
    dest_path: &str,
) -> Result<BackupInfo> {
    let backup_key = derive_backup_key(mvk)?;

    // Metadata key for reading filenames/timestamps from DB.
    let metadata_key = crate::crypto::hkdf::derive_subkey(mvk, crate::crypto::hkdf::LABEL_METADATA_KEY)
        .map_err(|e| BackupError::Internal(e.to_string()))?;

    // List all files from the DB.
    let files = crate::database::files::list_files(db, &metadata_key, None)
        .map_err(|e| BackupError::Internal(e.to_string()))?;

    let manifest_nonce = derive_backup_nonce(&backup_key, b"manifest-nonce")?;

    // Build manifest entries by reading actual blob sizes.
    let mut entries: Vec<ManifestEntry> = Vec::new();
    let mut total_size: u64 = 0;
    let mut blob_data_map: Vec<([u8; 16], Vec<u8>)> = Vec::new();
    for file in &files {
        let file_id_bytes = hex_decode(&file.file_id)
            .ok_or_else(|| BackupError::Internal(format!("invalid file_id: {}", file.file_id)))?;
        let blob_path = vdir.blob_path(&file_id_bytes);
        let blob_data = std::fs::read(&blob_path).map_err(|e| BackupError::Io(e))?;
        let actual_size = blob_data.len() as u64;
        entries.push(ManifestEntry {
            file_id: file_id_bytes,
            encrypted_size: actual_size,
            encryption_ver: file.encryption_version,
            name: file.filename.clone(),
            created_at: parse_i64(&file.created_at)?,
            modified_at: parse_i64(&file.modified_at)?,
        });
        total_size += actual_size;
        blob_data_map.push((file_id_bytes, blob_data));
    }

    let file_count = entries.len() as u64;
    let manifest_region = container::ManifestRegion { entries: entries.clone() };

    // Encrypt manifest first (need meta_len and meta_hash for header).
    let mut manifest_buf = Vec::new();
    manifest_region
        .write(&mut manifest_buf, &backup_key, &manifest_nonce)
        .map_err(|e| BackupError::Internal(e.to_string()))?;

    let meta_len = manifest_buf.len() as u32;
    let mut hasher = Sha256::new();
    use sha2::Digest as _;
    hasher.update(&manifest_buf);
    let meta_hash_arr: [u8; 32] = hasher.finalize().into();

    // Generate random backup nonce.
    let mut backup_nonce = [0u8; 24];
    getrandom::getrandom(&mut backup_nonce).map_err(|_| BackupError::Internal("rng".to_string()))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let vault_name = vdir.root.to_string_lossy().to_string();

    let header = container::HeaderRegion {
        backup_nonce,
        vault_name,
        created_at: now,
        meta_len,
        meta_hash: meta_hash_arr,
    };

    // Write to temp file, then rename (atomic).
    let dest = Path::new(dest_path);

    // Use create_new(true) with retry to avoid silent truncation.
    let mut tmp_file = None;
    let mut actual_tmp_path = None;
    for _attempt in 0..3 {
        let suffix = random_hex(16);
        let path = dest.with_extension(format!("vey.tmp.{suffix}"));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(f) => {
                tmp_file = Some(f);
                actual_tmp_path = Some(path);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(BackupError::Io(e)),
        }
    }
    let mut writer = tmp_file.ok_or_else(|| {
        BackupError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "failed to allocate temp path after 3 retries",
        ))
    })?;
    let tmp_path = actual_tmp_path.ok_or_else(|| {
        BackupError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "failed to allocate temp path after 3 retries",
        ))
    })?;

    // Write header.
    header
        .write(&mut writer, &backup_key)
        .map_err(|e| BackupError::Internal(e.to_string()))?;

    // Write encrypted manifest.
    writer.write_all(&manifest_buf).map_err(BackupError::Io)?;

    // Write blobs and track hash.
    let mut blobs_hasher = Sha256::new();
    for ((_file_id, blob_data), entry) in blob_data_map.iter().zip(entries.iter()) {
        blobs_hasher.update(blob_data);
        writer.write_all(blob_data).map_err(BackupError::Io)?;
        // Verify entry.encrypted_size matches actual size
        debug_assert_eq!(entry.encrypted_size, blob_data.len() as u64);
    }
    let blobs_hash_arr: [u8; 32] = blobs_hasher.finalize().into();

    // Write trailer.
    let trailer_nonce = derive_backup_nonce(&backup_key, b"trailer-nonce")?;
    let trailer = container::TrailerRegion {
        file_count,
        blobs_hash: blobs_hash_arr,
    };
    trailer
        .write(&mut writer, &backup_key, &trailer_nonce)
        .map_err(|e| BackupError::Internal(e.to_string()))?;

    writer.flush().map_err(BackupError::Io)?;
    drop(writer);

    // Fsync temp file is done by drop + flush; rename is atomic.
    std::fs::rename(&tmp_path, dest).map_err(BackupError::Io)?;

    // Fsync parent directory.
    if let Some(parent) = dest.parent() {
        crate::storage::vault_dir::fsync_dir(parent).map_err(BackupError::Io)?;
    }

    Ok(BackupInfo {
        path: dest_path.to_string(),
        file_count,
        total_size,
        created_at_unix: now,
        verified_fields: Vec::new(),
    })
}

fn random_hex(len: usize) -> String {
    let mut bytes = vec![0u8; len / 2];
    getrandom::getrandom(&mut bytes).expect("OS RNG failure");
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in &bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}
