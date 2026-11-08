//! Crash-recovery scan on vault open.
//!
//! - Deletes orphaned .veylora-tmp-* files in blobs/
//! - Runs PRAGMA integrity_check on vault.db; fails loudly if not "ok"
//! - Does NOT auto-repair; reports and refuses to open

use std::path::Path;

use crate::storage::blob::cleanup_tmp_files;
use crate::storage::vault_dir::VaultDir;

/// Report from a startup scan.
#[derive(Debug)]
pub struct ScanReport {
    pub tmp_files_removed: usize,
    pub db_integrity_ok: bool,
}

/// Errors from crash recovery.
#[derive(Debug, thiserror::Error)]
pub enum CrashError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("blob error: {0}")]
    Blob(#[from] crate::storage::blob::BlobError),
    #[error("database integrity check failed: {0}")]
    IntegrityFailed(String),
    #[error("database error: {0}")]
    Database(String),
}

/// Run the startup crash-recovery scan.
pub fn startup_scan(vault_root: &Path) -> Result<ScanReport, CrashError> {
    let vdir = VaultDir::new(vault_root.to_path_buf());

    // Remove orphaned temp files.
    let tmp_files_removed = cleanup_tmp_files(vault_root)?;

    // Run SQLite integrity check.
    let db_integrity_ok = check_db_integrity(&vdir.db_path)?;

    if !db_integrity_ok {
        return Err(CrashError::IntegrityFailed(
            "vault.db integrity_check did not return ok".to_string(),
        ));
    }

    Ok(ScanReport {
        tmp_files_removed,
        db_integrity_ok: true,
    })
}

/// Run PRAGMA integrity_check on the database file.
fn check_db_integrity(db_path: &Path) -> Result<bool, CrashError> {
    if !db_path.exists() {
        // No DB yet — nothing to check; caller will create it.
        return Ok(true);
    }

    let conn = rusqlite::Connection::open(db_path).map_err(|e| CrashError::Database(e.to_string()))?;
    let integrity_result = conn.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0));
    match integrity_result {
        Ok(result) => {
            if result != "ok" {
                return Ok(false);
            }
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn startup_scan_fresh_dir_ok() {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::current_dir()
            .unwrap()
            .join(format!("target/debug/test-vault-crash-{}-{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let report = startup_scan(&tmp).expect("scan fresh dir");
        assert!(report.db_integrity_ok);
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn startup_scan_removes_tmp_files() {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::current_dir()
            .unwrap()
            .join(format!("target/debug/test-vault-crash-tmp-{}-{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&tmp);
        let vdir = super::VaultDir::new(tmp.clone());
        vdir.create_layout().unwrap();
        fs::write(vdir.tmp_blob_path("orphan"), b"junk").unwrap();

        let report = startup_scan(&tmp).expect("scan with tmp");
        assert_eq!(report.tmp_files_removed, 1);
        assert!(!vdir.tmp_blob_path("orphan").exists());
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn startup_scan_corrupt_db_fails() {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::current_dir()
            .unwrap()
            .join(format!("target/debug/test-vault-crash-corrupt-{}-{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // Write garbage to vault.db
        fs::write(tmp.join("vault.db"), b"not a sqlite database").unwrap();

        let err = startup_scan(&tmp).unwrap_err();
        assert!(
            matches!(err, CrashError::IntegrityFailed(_)),
            "expected IntegrityFailed, got {:?}",
            err
        );
        fs::remove_dir_all(&tmp).ok();
    }
}
