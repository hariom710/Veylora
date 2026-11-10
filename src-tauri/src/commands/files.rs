//! Slice B — file and folder IPC commands: import, list, export, rename,
//! delete. Thin layer: all crypto goes through the existing storage/database
//! APIs; no key material ever crosses the IPC boundary.

#![allow(missing_docs)]

use std::sync::Arc;

use crate::auth::session::SessionManager;
use crate::crypto::hkdf::{derive_subkey, LABEL_METADATA_KEY};
use crate::crypto::mvk::Mvk;
use crate::database::files::FileRecord;
use crate::database::folders::FolderRecord;
use crate::storage::format::{FileKey, read_header, read_range};

/// File row for the UI (decrypted metadata — UI strings by design).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileInfo {
    pub id: u64,
    pub filename: String,
    /// Hex blob id — the path segment for vault:// streaming.
    pub file_id: String,
    pub folder_id: Option<u64>,
    pub encrypted_size: u64,
    pub created_at: String,
    pub modified_at: String,
}

/// Folder row for the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FolderInfo {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub name: String,
    pub created_at: String,
}

/// Files + subfolders of one directory (File Manager listing).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Listing {
    pub files: Vec<FileInfo>,
    pub folders: Vec<FolderInfo>,
}

fn to_file_info(r: FileRecord) -> FileInfo {
    FileInfo {
        id: r.id,
        filename: r.filename,
        file_id: r.file_id,
        folder_id: r.folder_id,
        encrypted_size: r.encrypted_size,
        created_at: r.created_at,
        modified_at: r.modified_at,
    }
}

fn to_folder_info(r: FolderRecord) -> FolderInfo {
    FolderInfo {
        id: r.id,
        parent_id: r.parent_id,
        name: r.name,
        created_at: r.created_at,
    }
}

/// Lock gate + vault root, shared by every command here.
fn require_unlocked(session: &SessionManager) -> Result<String, String> {
    if session.is_locked() {
        return Err("vault is locked".to_string());
    }
    session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())
}

/// metadata_key = HKDF(MVK, "veylora/v1/metadata-key") — the key that
/// decrypts filenames/timestamps in vault.db.
fn metadata_key(session: &SessionManager) -> Result<[u8; 32], String> {
    super::with_mvk(session, |mvk| {
        derive_subkey(mvk, LABEL_METADATA_KEY).map_err(|e| e.to_string())
    })?
}

/// Encrypt files from disk into the vault. The source files are copied, not
/// moved — plaintext on disk is the caller's concern after import.
pub fn file_import(
    paths: Vec<String>,
    folder_id: Option<u64>,
    session: Arc<SessionManager>,
) -> Result<Vec<FileInfo>, String> {
    let vault_root = require_unlocked(&session)?;
    let vault_root = std::path::PathBuf::from(vault_root);
    let vault_id = super::vault_id_at(vault_root.to_str().ok_or("invalid vault path")?)?;
    let db = super::open_db_for_session(&session)?;
    let metadata_key = metadata_key(&session)?;

    let mut imported = Vec::new();
    for path in paths {
        let src = std::path::Path::new(&path);
        let filename = src
            .file_name()
            .ok_or_else(|| format!("invalid path: {path}"))?
            .to_string_lossy()
            .into_owned();
        let mut source =
            std::fs::File::open(src).map_err(|e| format!("cannot open {path}: {e}"))?;

        let mut file_id = [0u8; 16];
        getrandom::getrandom(&mut file_id).map_err(|e| e.to_string())?;
        let file_id_hex = hex_encode(&file_id);

        let file_key = super::with_mvk(&session, |mvk| {
            FileKey::derive(&Mvk::from_array(*mvk), &file_id).map_err(|e| e.to_string())
        })??;

        // Blob BEFORE DB row: a crash leaves a cleanable orphan blob, never
        // a DB row pointing at a missing blob. write_blob is atomic (temp+rename).
        let encrypted_size = crate::storage::blob::write_blob(
            &vault_root,
            &file_id,
            &mut source,
            &file_key,
        )
        .map_err(|e| e.to_string())?;

        let id = crate::database::files::insert_file(
            &db,
            &metadata_key,
            vault_id,
            &file_id_hex,
            &filename,
            folder_id,
            encrypted_size,
        )
        .map_err(|e| e.to_string())?;

        let record =
            crate::database::files::get_file(&db, &metadata_key, id).map_err(|e| e.to_string())?;
        imported.push(to_file_info(record));
    }
    Ok(imported)
}

/// List files and subfolders of one directory (None = vault root).
pub fn file_list(
    folder_id: Option<u64>,
    session: Arc<SessionManager>,
) -> Result<Listing, String> {
    require_unlocked(&session)?;
    let db = super::open_db_for_session(&session)?;
    let metadata_key = metadata_key(&session)?;

    let files = crate::database::files::list_files(&db, &metadata_key, folder_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(to_file_info)
        .collect();
    let folders = crate::database::folders::list_children(&db, &metadata_key, folder_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(to_folder_info)
        .collect();
    Ok(Listing { files, folders })
}

/// Decrypt a vault file to `dest_path`. User-initiated plaintext export —
/// the one place v1 writes decrypted data to disk.
pub fn file_export(
    id: u64,
    dest_path: String,
    session: Arc<SessionManager>,
) -> Result<u64, String> {
    let vault_root = require_unlocked(&session)?;
    let vault_root = std::path::PathBuf::from(vault_root);
    let db = super::open_db_for_session(&session)?;
    let metadata_key = metadata_key(&session)?;

    let record =
        crate::database::files::get_file(&db, &metadata_key, id).map_err(|e| e.to_string())?;
    let file_id = hex_decode(&record.file_id).ok_or("invalid file_id")?;

    // Key material stays inside the closure (zeroized on drop with FileKey).
    let written = super::with_mvk(&session, |mvk| -> Result<u64, String> {
        let file_key = FileKey::derive(&Mvk::from_array(*mvk), &file_id)
            .map_err(|e| e.to_string())?;
        let vdir = crate::storage::vault_dir::VaultDir::new(vault_root.clone());
        let blob_path = vdir.blob_path(&file_id);
        let data = std::fs::read(&blob_path).map_err(|e| format!("cannot read blob: {e}"))?;
        let (header, _) = read_header(&data, &file_key)
            .map_err(|e| format!("corrupted blob: {e}"))?;
        let plain = read_range(&data, 0, header.total_length, &file_key)
            .map_err(|e| format!("decryption failed: {e}"))?;
        std::fs::write(&dest_path, &plain)
            .map_err(|e| format!("cannot write {dest_path}: {e}"))?;
        Ok(plain.len() as u64)
    })??;
    Ok(written)
}

/// Delete the DB row and the blob (permanent — no recycle bin).
pub fn file_delete(id: u64, session: Arc<SessionManager>) -> Result<(), String> {
    let vault_root = require_unlocked(&session)?;
    let db = super::open_db_for_session(&session)?;
    crate::database::files::delete_file(&db, id, true, Some(std::path::Path::new(&vault_root)))
        .map_err(|e| e.to_string())
}

/// Rename a file: the new name is re-encrypted in vault.db.
pub fn file_rename(
    id: u64,
    new_filename: String,
    session: Arc<SessionManager>,
) -> Result<(), String> {
    require_unlocked(&session)?;
    if new_filename.trim().is_empty() {
        return Err("filename cannot be empty".to_string());
    }
    let db = super::open_db_for_session(&session)?;
    let metadata_key = metadata_key(&session)?;
    crate::database::files::rename_file(&db, &metadata_key, id, new_filename.trim())
        .map_err(|e| e.to_string())
}

/// List child folders (None = root).
pub fn folder_list(
    parent_id: Option<u64>,
    session: Arc<SessionManager>,
) -> Result<Vec<FolderInfo>, String> {
    require_unlocked(&session)?;
    let db = super::open_db_for_session(&session)?;
    let metadata_key = metadata_key(&session)?;
    crate::database::folders::list_children(&db, &metadata_key, parent_id)
        .map_err(|e| e.to_string())
        .map(|v| v.into_iter().map(to_folder_info).collect())
}

/// Create a folder under `parent_id` (None = root).
pub fn folder_create(
    name: String,
    parent_id: Option<u64>,
    session: Arc<SessionManager>,
) -> Result<FolderInfo, String> {
    let vault_root = require_unlocked(&session)?;
    let vault_id = super::vault_id_at(&vault_root)?;
    let db = super::open_db_for_session(&session)?;
    let metadata_key = metadata_key(&session)?;
    if name.trim().is_empty() {
        return Err("folder name cannot be empty".to_string());
    }
    let id = crate::database::folders::insert_folder(
        &db,
        &metadata_key,
        vault_id,
        parent_id,
        name.trim(),
    )
    .map_err(|e| e.to_string())?;
    let record =
        crate::database::folders::get_folder(&db, &metadata_key, id).map_err(|e| e.to_string())?;
    Ok(to_folder_info(record))
}

/// Star or unstar a file.
pub fn file_favorite(
    id: u64,
    favorite: bool,
    session: Arc<SessionManager>,
) -> Result<(), String> {
    let vault_root = require_unlocked(&session)?;
    let vault_id = super::vault_id_at(&vault_root)?;
    let db = super::open_db_for_session(&session)?;
    let metadata_key = metadata_key(&session)?;
    // Resolves the row and 404s on unknown ids.
    let record =
        crate::database::files::get_file(&db, &metadata_key, id).map_err(|e| e.to_string())?;
    crate::database::favorites::set_favorite(&db, vault_id, &record.file_id, favorite)
        .map_err(|e| e.to_string())
}

/// All favorited files (any folder), most recently starred first.
pub fn file_favorites(session: Arc<SessionManager>) -> Result<Vec<FileInfo>, String> {
    let vault_root = require_unlocked(&session)?;
    let vault_id = super::vault_id_at(&vault_root)?;
    let db = super::open_db_for_session(&session)?;
    let metadata_key = metadata_key(&session)?;

    let mut out = Vec::new();
    for file_id in crate::database::favorites::list_favorite_ids(&db, vault_id)
        .map_err(|e| e.to_string())?
    {
        // A favorite whose file was deleted is skipped, not fatal.
        if let Ok(row_id) =
            crate::database::files::get_file_id_by_file_id(&db, &file_id)
        {
            if let Ok(record) =
                crate::database::files::get_file(&db, &metadata_key, row_id)
            {
                out.push(to_file_info(record));
            }
        }
    }
    Ok(out)
}

/// Recently imported files across all folders, newest first.
pub fn file_recent(limit: u64, session: Arc<SessionManager>) -> Result<Vec<FileInfo>, String> {
    require_unlocked(&session)?;
    let db = super::open_db_for_session(&session)?;
    let metadata_key = metadata_key(&session)?;
    crate::database::files::list_recent(&db, &metadata_key, limit)
        .map_err(|e| e.to_string())
        .map(|v| v.into_iter().map(to_file_info).collect())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
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
