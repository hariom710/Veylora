//! Folder metadata operations with encrypted names/timestamps.

use crate::database::crypto::encrypt_meta;
use crate::database::DbError;
use crate::database::Db;

/// A folder record with decrypted metadata fields.
#[derive(Debug, Clone)]
pub struct FolderRecord {
    pub id: u64,
    pub vault_id: u64,
    pub parent_id: Option<u64>,
    pub name: String,
    pub created_at: String,
}

const TABLE: &str = "folders";

/// Insert a new folder record. Encrypts name and created_at.
pub fn insert_folder(
    db: &Db,
    metadata_key: &[u8; 32],
    vault_id: u64,
    parent_id: Option<u64>,
    name: &str,
) -> Result<u64, DbError> {
    let now = now_unix();
    let (n_nonce, n_cipher) = encrypt_meta(
        metadata_key,
        TABLE,
        0,
        "name",
        name.as_bytes(),
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

    db.conn.execute(
        "INSERT INTO folders (vault_id, parent_id, name_nonce, name_ciphertext,
            created_at_nonce, created_at_ciphertext)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![vault_id as i64, parent_id.map(|v| v as i64), n_nonce, n_cipher, ca_nonce, ca_cipher],
    ).map_err(DbError::from)?;

    let id: i64 = db
        .conn
        .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
        .map_err(DbError::from)?;

    // Re-encrypt with correct row_id in AAD
    let (n_nonce, n_cipher) = encrypt_meta(
        metadata_key,
        TABLE,
        id as u64,
        "name",
        name.as_bytes(),
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

    db.conn.execute(
        "UPDATE folders SET name_nonce = ?2, name_ciphertext = ?3,
             created_at_nonce = ?4, created_at_ciphertext = ?5 WHERE id = ?1",
        rusqlite::params![id, n_nonce, n_cipher, ca_nonce, ca_cipher],
    ).map_err(DbError::from)?;

    Ok(id as u64)
}

/// Get a folder record by id.
pub fn get_folder(db: &Db, metadata_key: &[u8; 32], id: u64) -> Result<FolderRecord, DbError> {
    let row = db
        .conn
        .query_row(
            "SELECT vault_id, parent_id, name_nonce, name_ciphertext,
                    created_at_nonce, created_at_ciphertext
             FROM folders WHERE id = ?1",
            rusqlite::params![id as i64],
            |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, Option<i64>>(1)?.map(|v| v as u64),
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                ))
            },
        )
        .map_err(|e| {
            if e.to_string().contains("no rows") {
                DbError::NotFound(id.to_string())
            } else {
                DbError::from(e)
            }
        })?;

    let (vault_id, parent_id, n_nonce, n_cipher, ca_nonce, ca_cipher) = row;

    let name = decrypt_string(metadata_key, TABLE, id, "name", &n_nonce, &n_cipher)?;
    let created_at =
        decrypt_string(metadata_key, TABLE, id, "created_at", &ca_nonce, &ca_cipher)?;

    Ok(FolderRecord {
        id,
        vault_id,
        parent_id,
        name,
        created_at,
    })
}

/// List child folders of a parent (parent_id = None means root).
pub fn list_children(
    db: &Db,
    metadata_key: &[u8; 32],
    parent_id: Option<u64>,
) -> Result<Vec<FolderRecord>, DbError> {
    let mut rows: Vec<(u64, u64, Option<u64>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> = Vec::new();

    match parent_id {
        Some(pid) => {
            let mut stmt = db.conn.prepare(
                "SELECT id, vault_id, parent_id, name_nonce, name_ciphertext,
                        created_at_nonce, created_at_ciphertext
                 FROM folders WHERE parent_id = ?1",
            ).map_err(DbError::from)?;
            let file_rows = stmt.query_map(rusqlite::params![pid as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                    row.get::<_, Vec<u8>>(6)?,
                ))
            }).map_err(DbError::from)?;
            for r in file_rows {
                rows.push(r.map_err(DbError::from)?);
            }
        }
        None => {
            let mut stmt = db.conn.prepare(
                "SELECT id, vault_id, parent_id, name_nonce, name_ciphertext,
                        created_at_nonce, created_at_ciphertext
                 FROM folders WHERE parent_id IS NULL",
            ).map_err(DbError::from)?;
            let file_rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, i64>(1)? as u64,
                    row.get::<_, Option<i64>>(2)?.map(|v| v as u64),
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, Vec<u8>>(5)?,
                    row.get::<_, Vec<u8>>(6)?,
                ))
            }).map_err(DbError::from)?;
            for r in file_rows {
                rows.push(r.map_err(DbError::from)?);
            }
        }
    }

    let mut records = Vec::new();
    for (id, vault_id, parent_id, n_nonce, n_cipher, ca_nonce, ca_cipher) in rows {
        let name = decrypt_string(metadata_key, TABLE, id, "name", &n_nonce, &n_cipher)?;
        let created_at = decrypt_string(metadata_key, TABLE, id, "created_at", &ca_nonce, &ca_cipher)?;
        records.push(FolderRecord {
            id,
            vault_id,
            parent_id,
            name,
            created_at,
        });
    }
    Ok(records)
}

/// Rename a folder.
pub fn rename_folder(
    db: &Db,
    metadata_key: &[u8; 32],
    id: u64,
    new_name: &str,
) -> Result<(), DbError> {
    get_folder(db, metadata_key, id)?;

    let (nonce, ciphertext) = encrypt_meta(
        metadata_key,
        TABLE,
        id,
        "name",
        new_name.as_bytes(),
    )
    .map_err(|e| DbError::Sql(e.to_string()))?;

    db.conn.execute(
        "UPDATE folders SET name_nonce = ?2, name_ciphertext = ?3 WHERE id = ?1",
        rusqlite::params![id as i64, nonce, ciphertext],
    ).map_err(DbError::from)?;

    Ok(())
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
    let plaintext = crate::database::crypto::decrypt_meta(
        metadata_key, table, row_id, column, &nonce_arr, ciphertext,
    )
    .map_err(|e| DbError::Sql(e.to_string()))?;
    String::from_utf8(plaintext).map_err(|e| DbError::Sql(e.to_string()))
}

/// Current unix timestamp.
fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::hkdf::{derive_subkey, LABEL_METADATA_KEY};
    use crate::crypto::mvk::Mvk;
    use std::fs;
    use std::path::PathBuf;

    fn setup_db() -> (Db, [u8; 32], PathBuf) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::current_dir()
            .unwrap()
            .join(format!("target/debug/test-vault-folders-{}-{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let mvk = Mvk::generate().expect("mvk");
        let metadata_key = derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY).expect("derive");
        let db_path = tmp.join("vault.db");
        let db = Db::open(&db_path).expect("open");
        (db, metadata_key, tmp)
    }

    #[test]
    fn insert_and_get_folder_roundtrip() {
        let (db, metadata_key, _tmp) = setup_db();
        let id = insert_folder(&db, &metadata_key, 1, None, "Documents").expect("insert");
        let record = get_folder(&db, &metadata_key, id).expect("get");
        assert_eq!(record.name, "Documents");
        assert_eq!(record.parent_id, None);
    }

    #[test]
    fn list_children_by_parent() {
        let (db, metadata_key, _tmp) = setup_db();
        insert_folder(&db, &metadata_key, 1, None, "root").unwrap();
        insert_folder(&db, &metadata_key, 1, Some(1), "child1").unwrap();
        insert_folder(&db, &metadata_key, 1, Some(1), "child2").unwrap();

        let root = list_children(&db, &metadata_key, None).expect("list root");
        assert_eq!(root.len(), 1);

        let children = list_children(&db, &metadata_key, Some(1)).expect("list children");
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn rename_folder_changes_name() {
        let (db, metadata_key, _tmp) = setup_db();
        let id = insert_folder(&db, &metadata_key, 1, None, "old").unwrap();
        rename_folder(&db, &metadata_key, id, "new").expect("rename");
        let record = get_folder(&db, &metadata_key, id).expect("get after rename");
        assert_eq!(record.name, "new");
    }

    #[test]
    fn folder_metadata_encryption_hides_plaintext() {
        let (db, metadata_key, tmp) = setup_db();
        insert_folder(&db, &metadata_key, 1, None, "VerySecretFolder").unwrap();

        let db_path = tmp.join("vault.db");
        let raw = fs::read(&db_path).expect("read db");
        assert!(
            !raw.windows(b"VerySecretFolder".len()).any(|w| w == b"VerySecretFolder"),
            "plaintext folder name found in vault.db bytes!"
        );
    }
}
