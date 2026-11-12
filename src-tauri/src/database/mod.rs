//! SQLite database wrapper for vault metadata.
//!
//! Synchronous API (no async runtime). Opens vault.db with WAL mode,
//! applies migrations, and provides access to file/folder operations.

use std::path::Path;

pub mod crypto;
pub mod favorites;
pub mod files;
pub mod folders;
pub mod schema;

/// Handle to an open vault database.
pub struct Db {
    conn: rusqlite::Connection,
}

impl Db {
    /// Open (or create) vault.db at `path`, apply migrations, enable WAL.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = rusqlite::Connection::open(path).map_err(|e| DbError::Open(e.to_string()))?;

        // WAL mode + full synchronous for crash safety.
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")
            .map_err(|e| DbError::Pragma(e.to_string()))?;

        schema::apply_migrations(&conn).map_err(|e| DbError::Migration(e.to_string()))?;

        Ok(Db { conn })
    }

    /// Borrow the underlying connection for custom queries.
    pub fn conn(&self) -> &rusqlite::Connection {
        &self.conn
    }

    /// Borrow mutably for custom writes.
    pub fn conn_mut(&mut self) -> &mut rusqlite::Connection {
        &mut self.conn
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        // Connection closes on drop; checkpoint WAL for cleanliness.
        let _ = self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }
}

/// Errors from database operations.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("failed to open database: {0}")]
    Open(String),
    #[error("pragma failed: {0}")]
    Pragma(String),
    #[error("migration failed: {0}")]
    Migration(String),
    #[error("SQL error: {0}")]
    Sql(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("duplicate blob name: {0}")]
    DuplicateBlobName(String),
}

impl From<rusqlite::Error> for DbError {
    fn from(e: rusqlite::Error) -> Self {
        DbError::Sql(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn open_db_creates_tables() {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::current_dir().unwrap().join(format!(
            "target/debug/test-vault-db-{}-{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let db_path = tmp.join("vault.db");
        let db = Db::open(&db_path).expect("open db");
        assert!(db_path.exists());

        // Verify all 6 tables exist
        let tables: Vec<String> = db
            .conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        for expected in [
            "vaults",
            "folders",
            "encrypted_files",
            "favorites",
            "application_settings",
            "backup_history",
        ] {
            assert!(
                tables.contains(&expected.to_string()),
                "missing table: {}",
                expected
            );
        }

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn open_db_wal_mode() {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::current_dir().unwrap().join(format!(
            "target/debug/test-vault-wal-{}-{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let db_path = tmp.join("vault.db");
        let _db = Db::open(&db_path).expect("open db");

        // Check journal_mode
        let mode: String = db_path.parent().unwrap().to_string_lossy().to_string();
        let _ = mode;
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode, "wal");
        fs::remove_dir_all(&tmp).ok();
    }
}
