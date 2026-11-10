//! Favorites: a star list per vault. Keys on the blob file_id (the row id
//! would also work, but file_id is what the favorites table stores in v1).

use rusqlite::OptionalExtension;

use crate::database::{Db, DbError};

/// Star (favorite=true) or unstar a file. Idempotent.
pub fn set_favorite(db: &Db, vault_id: u64, file_id: &str, favorite: bool) -> Result<(), DbError> {
    if favorite {
        let existing: Option<i64> = db
            .conn()
            .query_row(
                "SELECT id FROM favorites WHERE vault_id = ?1 AND file_id = ?2",
                rusqlite::params![vault_id as i64, file_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(DbError::from)?;
        if existing.is_some() {
            return Ok(());
        }
        db.conn()
            .execute(
                "INSERT INTO favorites (vault_id, file_id) VALUES (?1, ?2)",
                rusqlite::params![vault_id as i64, file_id],
            )
            .map_err(DbError::from)?;
    } else {
        db.conn()
            .execute(
                "DELETE FROM favorites WHERE vault_id = ?1 AND file_id = ?2",
                rusqlite::params![vault_id as i64, file_id],
            )
            .map_err(DbError::from)?;
    }
    Ok(())
}

/// Blob ids starred in this vault, most recently starred first.
pub fn list_favorite_ids(db: &Db, vault_id: u64) -> Result<Vec<String>, DbError> {
    let mut stmt = db
        .conn()
        .prepare("SELECT file_id FROM favorites WHERE vault_id = ?1 ORDER BY id DESC")
        .map_err(DbError::from)?;
    let rows = stmt
        .query_map(rusqlite::params![vault_id as i64], |r| r.get::<_, String>(0))
        .map_err(DbError::from)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}
