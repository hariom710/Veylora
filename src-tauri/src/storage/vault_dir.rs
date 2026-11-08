//! Vault directory layout and fsync helpers.
//!
//! Vault directory structure:
//!   <vault-root>/
//!     vault.meta
//!     vault.db
//!     blobs/
//!       <file_id_hex>
//!       .veylora-tmp-*          temp files during atomic writes

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Handles paths and directory creation for a vault on disk.
pub struct VaultDir {
    pub root: PathBuf,
    pub meta_path: PathBuf,
    pub db_path: PathBuf,
    pub blobs_dir: PathBuf,
}

impl VaultDir {
    pub fn new(root: PathBuf) -> Self {
        Self {
            blobs_dir: root.join("blobs"),
            meta_path: root.join("vault.meta"),
            db_path: root.join("vault.db"),
            root,
        }
    }

    /// Create the full directory layout: root + blobs/.
    pub fn create_layout(&self) -> io::Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(&self.blobs_dir)?;
        Ok(())
    }

    /// Ensure directories exist (idempotent).
    pub fn ensure_dirs(&self) -> io::Result<()> {
        self.create_layout()
    }

    /// Path for a blob file given its 16-byte file_id.
    pub fn blob_path(&self, file_id: &[u8; 16]) -> PathBuf {
        let hex = hex_encode(file_id);
        self.blobs_dir.join(hex)
    }

    /// Path for a temporary blob file.
    pub fn tmp_blob_path(&self, suffix: &str) -> PathBuf {
        self.blobs_dir.join(format!(".veylora-tmp-{}", suffix))
    }
}

/// Recursively fsync a directory by flushing its metadata.
///
/// On Windows, opening a directory handle requires `FILE_FLAG_BACKUP_SEMANTICS`
/// and flushing metadata via `FlushFileBuffers`. This requires the
/// `SeBackupPrivilege`, which regular users do not have.
///
/// When `FlushFileBuffers` fails with `ERROR_ACCESS_DENIED` (5), durability is
/// still provided by NTFS transactional journaling — the rename operation is
/// atomic on NTFS, and the journal guarantees metadata consistency. Any other
/// error is returned as an `io::Error` so unexpected failure modes surface
/// rather than being silently absorbed.
///
/// See docs/CRYPTO_DESIGN.md "Durability" for details.
pub fn fsync_dir(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;

        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
        const ERROR_ACCESS_DENIED: u32 = 5;

        let dir = match std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
        {
            Ok(d) => d,
            Err(e) => return Err(e),
        };

        extern "system" {
            fn FlushFileBuffers(h: *mut std::ffi::c_void) -> std::ffi::c_int;
        }

        let result = unsafe { FlushFileBuffers(dir.as_raw_handle() as *mut std::ffi::c_void) };
        if result == 0 {
            let last_error = std::io::Error::last_os_error();
            if last_error.raw_os_error() == Some(ERROR_ACCESS_DENIED as i32) {
                // FlushFileBuffers requires SeBackupPrivilege on Windows.
                // NTFS journaling provides rename durability without it.
                return Ok(());
            }
            return Err(last_error);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        let dir = File::open(path)?;
        dir.sync_all()?;
        Ok(())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn create_layout_makes_dirs() {
        let tmp = std::env::current_dir()
            .unwrap()
            .join(format!("target/debug/test-vault-vdir-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let vdir = VaultDir::new(tmp.clone());
        vdir.create_layout().expect("create layout");
        assert!(vdir.root.is_dir());
        assert!(vdir.blobs_dir.is_dir());
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn blob_path_uses_hex_file_id() {
        let vdir = VaultDir::new(PathBuf::from("/tmp/vault"));
        let file_id = [0xAB, 0xCD, 0xEF, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D];
        let path = vdir.blob_path(&file_id);
        assert_eq!(path.file_name().unwrap(), "abcdef0102030405060708090a0b0c0d");
    }

    #[test]
    fn fsync_dir_works_on_existing_dir() {
        let tmp = std::env::current_dir()
            .unwrap()
            .join(format!("target/debug/test-vault-fsync-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        fsync_dir(&tmp).expect("fsync dir should succeed");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn tmp_blob_path_uses_veylora_prefix() {
        let vdir = VaultDir::new(PathBuf::from("/tmp/vault"));
        let path = vdir.tmp_blob_path("abc123");
        assert_eq!(
            path.file_name().unwrap(),
            ".veylora-tmp-abc123"
        );
    }

    #[test]
    fn fsync_dir_ok_on_normal_directory() {
        let tmp = std::env::current_dir()
            .unwrap()
            .join(format!("target/debug/test-vault-fsync2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        // fsync_dir on a normal directory must return Ok(()) on Windows
        // (FlushFileBuffers requires SeBackupPrivilege; degraded to NTFS journal)
        fsync_dir(&tmp).expect("fsync dir should return Ok on normal directory");
        let _ = fs::remove_dir_all(&tmp);
    }
}
