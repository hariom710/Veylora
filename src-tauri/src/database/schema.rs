//! Table definitions and migration SQL for vault.db.
//!
//! Migration v1 creates all tables. Sensitive columns (filenames, folder
//! names, tags, timestamps) are stored as nonce||ciphertext BLOB pairs.

/// Migration v1: create all tables.
pub const MIGRATION_V1: &str = r#"
CREATE TABLE IF NOT EXISTS vaults (
    id INTEGER PRIMARY KEY,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY,
    vault_id INTEGER NOT NULL,
    parent_id INTEGER,
    name_nonce BLOB NOT NULL,
    name_ciphertext BLOB NOT NULL,
    created_at_nonce BLOB NOT NULL,
    created_at_ciphertext BLOB NOT NULL,
    FOREIGN KEY (parent_id) REFERENCES folders(id)
);

CREATE TABLE IF NOT EXISTS encrypted_files (
    id INTEGER PRIMARY KEY,
    vault_id INTEGER NOT NULL,
    file_id TEXT NOT NULL UNIQUE,
    filename_nonce BLOB NOT NULL,
    filename_ciphertext BLOB NOT NULL,
    folder_id INTEGER,
    encrypted_size INTEGER NOT NULL,
    encryption_version INTEGER NOT NULL,
    created_at_nonce BLOB NOT NULL,
    created_at_ciphertext BLOB NOT NULL,
    modified_at_nonce BLOB NOT NULL,
    modified_at_ciphertext BLOB NOT NULL,
    FOREIGN KEY (folder_id) REFERENCES folders(id)
);

CREATE TABLE IF NOT EXISTS favorites (
    id INTEGER PRIMARY KEY,
    vault_id INTEGER NOT NULL,
    file_id TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS application_settings (
    key TEXT PRIMARY KEY,
    value_nonce BLOB,
    value_ciphertext BLOB
);

CREATE TABLE IF NOT EXISTS backup_history (
    id INTEGER PRIMARY KEY,
    created_at INTEGER NOT NULL,
    backup_path TEXT NOT NULL,
    encrypted_size INTEGER NOT NULL,
    verified INTEGER NOT NULL
);
"#;

/// Migration v2: add auth_attempts table for PIN/password throttling.
pub const MIGRATION_V2: &str = r#"
CREATE TABLE IF NOT EXISTS auth_attempts (
    vault_id INTEGER PRIMARY KEY,
    failed_count INTEGER NOT NULL DEFAULT 0,
    locked_until_unix INTEGER NOT NULL DEFAULT 0,
    last_attempt_unix INTEGER NOT NULL DEFAULT 0
);
"#;

/// List of all migrations in order.
pub const MIGRATIONS: &[(&str, &str)] = &[("1", MIGRATION_V1), ("2", MIGRATION_V2)];

use rusqlite::OptionalExtension;

/// Ensure the schema_migrations table exists and apply pending migrations.
pub fn apply_migrations(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (version TEXT PRIMARY KEY)",
    )?;

    for (version, sql) in MIGRATIONS {
        let already: Option<String> = conn
            .query_row(
                "SELECT version FROM schema_migrations WHERE version = ?1",
                rusqlite::params![version],
                |row| row.get(0),
            )
            .optional()?;
        if already.is_some() {
            continue;
        }

        conn.execute_batch(sql)?;
        conn.execute(
            "INSERT INTO schema_migrations (version) VALUES (?1)",
            rusqlite::params![version],
        )?;
    }

    Ok(())
}
