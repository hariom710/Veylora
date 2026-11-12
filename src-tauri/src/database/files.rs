//! File metadata operations with encrypted filename/timestamps.

use crate::database::crypto::{decrypt_meta, encrypt_meta};
use crate::database::Db;
use crate::database::DbError;
use rusqlite::OptionalExtension;
use std::time::SystemTime;

/// A file record with decrypted metadata fields.
#[derive(Debug, Clone)]
pub struct FileRecord {
    pub id: u64,
    pub vault_id: u64,
    pub file_id: String,
    pub filename: String,
    pub folder_id: Option<u64>,
    pub encrypted_size: u64,
    pub encryption_version: u32,
    pub created_at: String,
    pub modified_at: String,
}

const TABLE: &str = "encrypted_files";

/// Current encryption version for file metadata.
const ENCRYPTION_VERSION: u32 = 1;

/// Insert a new file record. Encrypts filename and timestamps.
/// Returns the new row id.
pub fn insert_file(
    db: &Db,
    metadata_key: &[u8; 32],
    vault_id: u64,
    file_id: &str,
    filename: &str,
    folder_id: Option<u64>,
    encrypted_size: u64,
) -> Result<u64, DbError> {
    // Check for duplicate file_id
    let existing: Option<i64> = db
        .conn
        .query_row(
            "SELECT id FROM encrypted_files WHERE file_id = ?1",
            rusqlite::params![file_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(DbError::from)?;
    if existing.is_some() {
        return Err(DbError::DuplicateBlobName(file_id.to_string()));
    }

    let now = now_unix();
    let (fn_nonce, fn_cipher) = encrypt_meta(
        metadata_key,
        TABLE,
        0, // row_id not known yet; use 0 for AAD of plaintext fields before insert
        "filename",
        filename.as_bytes(),
    )
    .map_err(|e| DbError::Sql(e.to_string()))?;
    let (ca_nonce, ca_cipher) = encrypt_meta(
        metadata_key,
        TABLE,
        0,
        "created_at",
        now.to_string().as_bytes(),
    )
    .map_err(|e| DbError::Sql(e.to_string()))?;
    let (ma_nonce, ma_cipher) = encrypt_meta(
        metadata_key,
        TABLE,
        0,
        "modified_at",
        now.to_string().as_bytes(),
    )
    .map_err(|e| DbError::Sql(e.to_string()))?;

    db.conn
        .execute(
            "INSERT INTO encrypted_files (vault_id, file_id, filename_nonce, filename_ciphertext,
            folder_id, encrypted_size, encryption_version,
            created_at_nonce, created_at_ciphertext, modified_at_nonce, modified_at_ciphertext)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                vault_id as i64,
                file_id,
                fn_nonce,
                fn_cipher,
                folder_id.map(|v| v as i64),
                encrypted_size as i64,
                ENCRYPTION_VERSION,
                ca_nonce,
                ca_cipher,
                ma_nonce,
                ma_cipher,
            ],
        )
        .map_err(DbError::from)?;

    // Get the new row id and re-encrypt with correct row_id in AAD.
    let id: i64 = db
        .conn
        .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
        .map_err(DbError::from)?;

    // Re-encrypt with correct row_id for AAD binding
    let (fn_nonce, fn_cipher) = encrypt_meta(
        metadata_key,
        TABLE,
        id as u64,
        "filename",
        filename.as_bytes(),
    )
    .map_err(|e| DbError::Sql(e.to_string()))?;
    let (ca_nonce, ca_cipher) = encrypt_meta(
        metadata_key,
        TABLE,
        id as u64,
        "created_at",
        now.to_string().as_bytes(),
    )
    .map_err(|e| DbError::Sql(e.to_string()))?;
    let (ma_nonce, ma_cipher) = encrypt_meta(
        metadata_key,
        TABLE,
        id as u64,
        "modified_at",
        now.to_string().as_bytes(),
    )
    .map_err(|e| DbError::Sql(e.to_string()))?;

    db.conn
        .execute(
            "UPDATE encrypted_files
         SET filename_nonce = ?2, filename_ciphertext = ?3,
             created_at_nonce = ?4, created_at_ciphertext = ?5,
             modified_at_nonce = ?6, modified_at_ciphertext = ?7
         WHERE id = ?1",
            rusqlite::params![id, fn_nonce, fn_cipher, ca_nonce, ca_cipher, ma_nonce, ma_cipher,],
        )
        .map_err(DbError::from)?;

    Ok(id as u64)
}

/// Get a file record by id, decrypting metadata fields.
pub fn get_file(db: &Db, metadata_key: &[u8; 32], id: u64) -> Result<FileRecord, DbError> {
    let row = db
        .conn
        .query_row(
            "SELECT vault_id, file_id, filename_nonce, filename_ciphertext,
                    folder_id, encrypted_size, encryption_version,
                    created_at_nonce, created_at_ciphertext,
                    modified_at_nonce, modified_at_ciphertext
             FROM encrypted_files WHERE id = ?1",
            rusqlite::params![id as i64],
            |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                    row.get::<_, i64>(5)? as u64,
                    row.get::<_, u32>(6)?,
                    row.get::<_, Vec<u8>>(7)?,
                    row.get::<_, Vec<u8>>(8)?,
                    row.get::<_, Vec<u8>>(9)?,
                    row.get::<_, Vec<u8>>(10)?,
                ))
            },
        )
        .map_err(|e| {
            let msg = e.to_string().to_lowercase();
            if msg.contains("not found") || msg.contains("query returned no rows") {
                DbError::NotFound(id.to_string())
            } else {
                DbError::from(e)
            }
        })?;

    let (
        vault_id,
        file_id,
        fn_nonce,
        fn_cipher,
        folder_id,
        encrypted_size,
        encryption_version,
        ca_nonce,
        ca_cipher,
        ma_nonce,
        ma_cipher,
    ) = row;

    let filename = decrypt_string(metadata_key, TABLE, id, "filename", &fn_nonce, &fn_cipher)?;
    let created_at = decrypt_string(metadata_key, TABLE, id, "created_at", &ca_nonce, &ca_cipher)?;
    let modified_at = decrypt_string(
        metadata_key,
        TABLE,
        id,
        "modified_at",
        &ma_nonce,
        &ma_cipher,
    )?;

    Ok(FileRecord {
        id,
        vault_id,
        file_id,
        filename,
        folder_id,
        encrypted_size,
        encryption_version,
        created_at,
        modified_at,
    })
}

/// One raw `encrypted_files` SELECT row, in column order.
type FileRow = (
    u64,
    String,
    Vec<u8>,
    Vec<u8>,
    Option<u64>,
    u64,
    u32,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
);

/// List files in a folder (folder_id = None means root).
pub fn list_files(
    db: &Db,
    metadata_key: &[u8; 32],
    folder_id: Option<u64>,
) -> Result<Vec<FileRecord>, DbError> {
    let mut rows: Vec<FileRow> = Vec::new();

    match folder_id {
        Some(fid) => {
            let mut stmt = db
                .conn
                .prepare(
                    "SELECT vault_id, file_id, filename_nonce, filename_ciphertext,
                        folder_id, encrypted_size, encryption_version,
                        created_at_nonce, created_at_ciphertext,
                        modified_at_nonce, modified_at_ciphertext
                 FROM encrypted_files WHERE folder_id = ?1",
                )
                .map_err(DbError::from)?;
            let file_rows = stmt
                .query_map(rusqlite::params![fid as i64], |row| {
                    Ok((
                        row.get::<_, i64>(0)? as u64,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                        row.get::<_, i64>(5)? as u64,
                        row.get::<_, u32>(6)?,
                        row.get::<_, Vec<u8>>(7)?,
                        row.get::<_, Vec<u8>>(8)?,
                        row.get::<_, Vec<u8>>(9)?,
                        row.get::<_, Vec<u8>>(10)?,
                    ))
                })
                .map_err(DbError::from)?;
            for r in file_rows {
                rows.push(r.map_err(DbError::from)?);
            }
        }
        None => {
            let mut stmt = db
                .conn
                .prepare(
                    "SELECT vault_id, file_id, filename_nonce, filename_ciphertext,
                        folder_id, encrypted_size, encryption_version,
                        created_at_nonce, created_at_ciphertext,
                        modified_at_nonce, modified_at_ciphertext
                 FROM encrypted_files WHERE folder_id IS NULL",
                )
                .map_err(DbError::from)?;
            let file_rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)? as u64,
                        row.get::<_, String>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                        row.get::<_, i64>(5)? as u64,
                        row.get::<_, u32>(6)?,
                        row.get::<_, Vec<u8>>(7)?,
                        row.get::<_, Vec<u8>>(8)?,
                        row.get::<_, Vec<u8>>(9)?,
                        row.get::<_, Vec<u8>>(10)?,
                    ))
                })
                .map_err(DbError::from)?;
            for r in file_rows {
                rows.push(r.map_err(DbError::from)?);
            }
        }
    }

    let mut records = Vec::new();
    for (
        vault_id,
        file_id,
        fn_nonce,
        fn_cipher,
        folder_id,
        encrypted_size,
        encryption_version,
        ca_nonce,
        ca_cipher,
        ma_nonce,
        ma_cipher,
    ) in rows
    {
        let id = get_file_id_by_file_id(db, &file_id)?;
        let filename = decrypt_string(metadata_key, TABLE, id, "filename", &fn_nonce, &fn_cipher)?;
        let created_at =
            decrypt_string(metadata_key, TABLE, id, "created_at", &ca_nonce, &ca_cipher)?;
        let modified_at = decrypt_string(
            metadata_key,
            TABLE,
            id,
            "modified_at",
            &ma_nonce,
            &ma_cipher,
        )?;
        records.push(FileRecord {
            id,
            vault_id,
            file_id,
            filename,
            folder_id,
            encrypted_size,
            encryption_version,
            created_at,
            modified_at,
        });
    }
    Ok(records)
}

/// Total file count and encrypted bytes across all folders. Both columns are
/// plaintext, so this needs no metadata key (works before unlock as well).
pub fn file_stats(db: &Db) -> Result<(u64, u64), DbError> {
    let (count, total): (i64, i64) = db
        .conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(encrypted_size), 0) FROM encrypted_files",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(DbError::from)?;
    Ok((count as u64, total as u64))
}

/// Most recently imported files across all folders, newest first.
pub fn list_recent(
    db: &Db,
    metadata_key: &[u8; 32],
    limit: u64,
) -> Result<Vec<FileRecord>, DbError> {
    let mut stmt = db
        .conn
        .prepare(
            "SELECT id, vault_id, file_id, filename_nonce, filename_ciphertext,
                folder_id, encrypted_size, encryption_version,
                created_at_nonce, created_at_ciphertext,
                modified_at_nonce, modified_at_ciphertext
         FROM encrypted_files ORDER BY id DESC LIMIT ?1",
        )
        .map_err(DbError::from)?;
    let file_rows = stmt
        .query_map(rusqlite::params![limit as i64], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, i64>(1)? as u64,
                row.get::<_, String>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, Vec<u8>>(4)?,
                row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                row.get::<_, i64>(6)? as u64,
                row.get::<_, u32>(7)?,
                row.get::<_, Vec<u8>>(8)?,
                row.get::<_, Vec<u8>>(9)?,
                row.get::<_, Vec<u8>>(10)?,
                row.get::<_, Vec<u8>>(11)?,
            ))
        })
        .map_err(DbError::from)?;

    let mut records = Vec::new();
    for row in file_rows {
        let (
            id,
            vault_id,
            file_id,
            fn_nonce,
            fn_cipher,
            folder_id,
            encrypted_size,
            encryption_version,
            ca_nonce,
            ca_cipher,
            ma_nonce,
            ma_cipher,
        ) = row.map_err(DbError::from)?;
        let filename = decrypt_string(metadata_key, TABLE, id, "filename", &fn_nonce, &fn_cipher)?;
        let created_at =
            decrypt_string(metadata_key, TABLE, id, "created_at", &ca_nonce, &ca_cipher)?;
        let modified_at = decrypt_string(
            metadata_key,
            TABLE,
            id,
            "modified_at",
            &ma_nonce,
            &ma_cipher,
        )?;
        records.push(FileRecord {
            id,
            vault_id,
            file_id,
            filename,
            folder_id,
            encrypted_size,
            encryption_version,
            created_at,
            modified_at,
        });
    }
    Ok(records)
}

/// Rename a file: re-encrypt new filename with same row_id AAD.
pub fn rename_file(
    db: &Db,
    metadata_key: &[u8; 32],
    id: u64,
    new_filename: &str,
) -> Result<(), DbError> {
    // Verify file exists
    get_file(db, metadata_key, id)?;

    let (nonce, ciphertext) =
        encrypt_meta(metadata_key, TABLE, id, "filename", new_filename.as_bytes())
            .map_err(|e| DbError::Sql(e.to_string()))?;

    db.conn.execute(
        "UPDATE encrypted_files SET filename_nonce = ?2, filename_ciphertext = ?3 WHERE id = ?1",
        rusqlite::params![id as i64, nonce, ciphertext],
    ).map_err(DbError::from)?;

    Ok(())
}

/// Delete a file record. If delete_blob is true, also remove the blob file.
pub fn delete_file(
    db: &Db,
    id: u64,
    delete_blob: bool,
    vault_root: Option<&std::path::Path>,
) -> Result<(), DbError> {
    // Get file_id before deleting
    let file_id: Option<String> = db
        .conn
        .query_row(
            "SELECT file_id FROM encrypted_files WHERE id = ?1",
            rusqlite::params![id as i64],
            |row| row.get(0),
        )
        .optional()
        .map_err(DbError::from)?;
    if file_id.is_none() {
        return Err(DbError::NotFound(id.to_string()));
    }

    db.conn
        .execute(
            "DELETE FROM encrypted_files WHERE id = ?1",
            rusqlite::params![id as i64],
        )
        .map_err(DbError::from)?;
    eprintln!("DEBUG delete_file: deleted id={}", id);

    if delete_blob {
        if let Some(fid_hex) = file_id {
            let file_id_bytes: [u8; 16] = hex_decode(&fid_hex)
                .ok_or_else(|| DbError::Sql("invalid file_id hex".to_string()))?;
            crate::storage::blob::delete_blob(
                vault_root.ok_or_else(|| {
                    DbError::Sql("vault_root required for blob deletion".to_string())
                })?,
                &file_id_bytes,
            )
            .map_err(|e| DbError::Sql(e.to_string()))?;
        }
    }

    Ok(())
}

/// Move a file to a different folder.
pub fn move_file(db: &Db, id: u64, new_folder_id: Option<u64>) -> Result<(), DbError> {
    db.conn
        .execute(
            "UPDATE encrypted_files SET folder_id = ?2 WHERE id = ?1",
            rusqlite::params![id as i64, new_folder_id.map(|v| v as i64)],
        )
        .map_err(DbError::from)?;
    Ok(())
}

/// Helper: get the internal row id from file_id hex.
pub fn get_file_id_by_file_id(db: &Db, file_id: &str) -> Result<u64, DbError> {
    let id: i64 = db
        .conn
        .query_row(
            "SELECT id FROM encrypted_files WHERE file_id = ?1",
            rusqlite::params![file_id],
            |row| row.get(0),
        )
        .map_err(DbError::from)?;
    Ok(id as u64)
}

/// Helper: decrypt a metadata column as UTF-8 string.
fn decrypt_string(
    metadata_key: &[u8; 32],
    table: &str,
    row_id: u64,
    column: &str,
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<String, DbError> {
    let nonce_arr: [u8; 24] = nonce
        .try_into()
        .map_err(|_| DbError::Sql("invalid nonce length".to_string()))?;
    let plaintext = decrypt_meta(metadata_key, table, row_id, column, &nonce_arr, ciphertext)
        .map_err(|e| DbError::Sql(e.to_string()))?;
    String::from_utf8(plaintext).map_err(|e| DbError::Sql(e.to_string()))
}

/// Current unix timestamp.
fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Decode hex string to 16-byte array.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::hkdf::{derive_subkey, LABEL_METADATA_KEY};
    use crate::crypto::mvk::Mvk;
    use crate::database::folders::insert_folder;
    use std::fs;

    fn setup_db() -> (Db, [u8; 32], PathBuf) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::current_dir().unwrap().join(format!(
            "target/debug/test-vault-files-{}-{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let mvk = Mvk::generate().expect("mvk");
        let metadata_key = derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY).expect("derive");
        let db_path = tmp.join("vault.db");
        let db = Db::open(&db_path).expect("open");
        (db, metadata_key, tmp)
    }

    type PathBuf = std::path::PathBuf;

    #[test]
    fn insert_and_get_file_roundtrip() {
        let (db, metadata_key, _tmp) = setup_db();
        let id = insert_file(
            &db,
            &metadata_key,
            1,
            "abc123def456",
            "report.pdf",
            None,
            1000,
        )
        .expect("insert");
        let record = get_file(&db, &metadata_key, id).expect("get");
        assert_eq!(record.filename, "report.pdf");
        assert_eq!(record.file_id, "abc123def456");
        assert_eq!(record.folder_id, None);
        assert_eq!(record.encrypted_size, 1000);
    }

    #[test]
    fn list_files_by_folder() {
        let (db, metadata_key, _tmp) = setup_db();
        insert_file(&db, &metadata_key, 1, "f1", "a.txt", None, 10).unwrap();
        let folder_id = insert_folder(&db, &metadata_key, 1, None, "test folder").unwrap();
        insert_file(&db, &metadata_key, 1, "f2", "b.txt", Some(folder_id), 20).unwrap();
        insert_file(&db, &metadata_key, 1, "f3", "c.txt", Some(folder_id), 30).unwrap();

        let root = list_files(&db, &metadata_key, None).expect("list root");
        assert_eq!(root.len(), 1);
        assert_eq!(root[0].filename, "a.txt");

        let children = list_files(&db, &metadata_key, Some(folder_id)).expect("list children");
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn file_stats_counts_all_folders() {
        let (db, metadata_key, _tmp) = setup_db();
        assert_eq!(file_stats(&db).unwrap(), (0, 0));
        insert_file(&db, &metadata_key, 1, "f1", "a.txt", None, 10).unwrap();
        let folder_id = insert_folder(&db, &metadata_key, 1, None, "d").unwrap();
        insert_file(&db, &metadata_key, 1, "f2", "b.txt", Some(folder_id), 20).unwrap();
        assert_eq!(file_stats(&db).unwrap(), (2, 30));
    }

    #[test]
    fn rename_file_changes_ciphertext() {
        let (db, metadata_key, _tmp) = setup_db();
        let id = insert_file(&db, &metadata_key, 1, "f1", "old.txt", None, 10).unwrap();
        rename_file(&db, &metadata_key, id, "new.txt").expect("rename");
        let record = get_file(&db, &metadata_key, id).expect("get after rename");
        assert_eq!(record.filename, "new.txt");
    }

    #[test]
    fn move_file_changes_folder() {
        let (db, metadata_key, _tmp) = setup_db();
        let id = insert_file(&db, &metadata_key, 1, "f1", "a.txt", None, 10).unwrap();
        let folder_id = insert_folder(&db, &metadata_key, 1, None, "target folder").unwrap();
        move_file(&db, id, Some(folder_id)).expect("move");
        let record = get_file(&db, &metadata_key, id).expect("get after move");
        assert_eq!(record.folder_id, Some(folder_id));
    }

    #[test]
    fn delete_file_removes_row() {
        let (db, metadata_key, _tmp) = setup_db();
        let id = insert_file(&db, &metadata_key, 1, "f1", "a.txt", None, 10).unwrap();
        delete_file(&db, id, false, None).expect("delete");
        let err = get_file(&db, &metadata_key, id).unwrap_err();
        assert!(matches!(err, DbError::NotFound(_)));
    }

    #[test]
    fn duplicate_file_id_rejected() {
        let (db, metadata_key, _tmp) = setup_db();
        insert_file(&db, &metadata_key, 1, "dup", "a.txt", None, 10).unwrap();
        let err = insert_file(&db, &metadata_key, 1, "dup", "b.txt", None, 20).unwrap_err();
        assert!(matches!(err, DbError::DuplicateBlobName(_)));
    }

    #[test]
    fn metadata_encryption_hides_plaintext_in_db() {
        let (db, metadata_key, tmp) = setup_db();
        insert_file(
            &db,
            &metadata_key,
            1,
            "f1",
            "secret_document.pdf",
            None,
            100,
        )
        .unwrap();

        // Read raw bytes of vault.db and search for the plaintext filename
        let db_path = tmp.join("vault.db");
        let raw = fs::read(&db_path).expect("read db");
        assert!(
            !raw.windows(b"secret_document.pdf".len())
                .any(|w| w == b"secret_document.pdf"),
            "plaintext filename found in vault.db bytes!"
        );
    }

    #[test]
    fn get_file_wrong_key_fails() {
        let (db, metadata_key, _tmp) = setup_db();
        let id = insert_file(&db, &metadata_key, 1, "f1", "a.txt", None, 10).unwrap();

        let wrong_key = [0xFFu8; 32];
        let err = get_file(&db, &wrong_key, id).unwrap_err();
        assert!(
            matches!(err, DbError::Sql(_)),
            "expected Sql error from AEAD failure, got {:?}",
            err
        );
    }
}
