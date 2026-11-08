//! Blob storage: read/write of encrypted blob files.
//!
//! Blob final path: blobs/<file_id_hex> (single contiguous file per vault entry).
//! No key material is stored here; the caller supplies the Phase 2 FileKey.
//!
//! All writes use the atomic write pattern:
//!   1. write to `blobs/.veylora-tmp-<random>` (same volume)
//!   2. `sync_all` the temp file
//!   3. `rename` over the final path (atomic on NTFS)
//!   4. `fsync_dir` the parent directory

use std::fs;
use std::io::{self, Read};
use std::path::Path;

use crate::storage::format::{self, FileKey, FormatError};
use crate::storage::vault_dir::{fsync_dir, VaultDir};

/// Errors from blob storage operations.
#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("format error: {0}")]
    Format(#[from] FormatError),
    #[error("blob not found: {0}")]
    NotFound(String),
    #[error("invalid byte range")]
    InvalidRange,
}

/// Write plaintext from `reader` into an encrypted blob at blobs/<file_id_hex>.
/// Uses the atomic write pattern: temp file -> fsync -> rename -> fsync dir.
/// Returns the encrypted size in bytes.
pub fn write_blob(
    vault_root: &Path,
    file_id: &[u8; 16],
    reader: &mut dyn Read,
    file_key: &FileKey,
) -> Result<u64, BlobError> {
    let vdir = VaultDir::new(vault_root.to_path_buf());
    vdir.ensure_dirs()?;

    let final_path = vdir.blob_path(file_id);

    // Write encrypted data to a temp file via Phase 2 writer.
    // Use create_new(true) with retry on AlreadyExists to avoid any
    // silent truncation of a pre-existing file. The temp name is
    // .veylora-tmp-<128-bit CSPRNG hex>; collision probability is 2^-128.
    let mut tmp_file = None;
    let mut tmp_path = None;
    for _attempt in 0..3 {
        let suffix = random_hex(16);
        let path = vdir.tmp_blob_path(&suffix);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(f) => {
                tmp_file = Some(f);
                tmp_path = Some(path);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(BlobError::Io(e)),
        }
    }
    let mut tmp_file = tmp_file.ok_or_else(|| {
        BlobError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            "failed to allocate temp path after 3 retries",
        ))
    })?;
    let tmp_path = tmp_path.unwrap();

    let encrypted_size = format::writer::write_file(reader, &mut tmp_file, file_key, *file_id)
        .map_err(BlobError::Format)?;

    // Fsync the temp file before renaming — guarantees the data is on disk.
    tmp_file.sync_all().map_err(BlobError::Io)?;
    drop(tmp_file);

    // Atomic rename over final path — same volume, atomic on NTFS.
    std::fs::rename(&tmp_path, &final_path).map_err(BlobError::Io)?;

    // Fsync the parent directory so the rename is durable.
    fsync_dir(&vdir.blobs_dir).map_err(BlobError::Io)?;

    Ok(encrypted_size)
}

/// Read byte range [start, end) from an encrypted blob.
/// Delegates to Phase 2 reader for decryption + authentication.
pub fn read_blob(
    vault_root: &Path,
    file_id: &[u8; 16],
    start: u64,
    end: u64,
    file_key: &FileKey,
) -> Result<Vec<u8>, BlobError> {
    let vdir = VaultDir::new(vault_root.to_path_buf());
    let path = vdir.blob_path(file_id);

    if !path.exists() {
        return Err(BlobError::NotFound(hex_encode(file_id)));
    }

    if start > end {
        return Err(BlobError::InvalidRange);
    }

    let data = fs::read(&path).map_err(BlobError::Io)?;
    format::reader::read_range(&data, start, end, file_key).map_err(BlobError::Format)
}

/// Delete a blob file.
pub fn delete_blob(vault_root: &Path, file_id: &[u8; 16]) -> Result<(), BlobError> {
    let vdir = VaultDir::new(vault_root.to_path_buf());
    let path = vdir.blob_path(file_id);
    if path.exists() {
        fs::remove_file(&path).map_err(BlobError::Io)?;
        fsync_dir(&vdir.blobs_dir).map_err(BlobError::Io)?;
    }
    Ok(())
}

/// List all blob file IDs (hex) in blobs/.
pub fn list_blobs(vault_root: &Path) -> Result<Vec<String>, BlobError> {
    let vdir = VaultDir::new(vault_root.to_path_buf());
    if !vdir.blobs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut ids = Vec::new();
    for entry in fs::read_dir(&vdir.blobs_dir).map_err(BlobError::Io)? {
        let entry = entry.map_err(BlobError::Io)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(".veylora-tmp-") {
            continue; // skip temp files
        }
        if name.len() == 32 {
            ids.push(name);
        }
    }
    ids.sort();
    Ok(ids)
}

/// Remove orphaned `.veylora-tmp-*` files from blobs/.
pub fn cleanup_tmp_files(vault_root: &Path) -> Result<usize, BlobError> {
    let vdir = VaultDir::new(vault_root.to_path_buf());
    if !vdir.blobs_dir.exists() {
        return Ok(0);
    }

    let mut removed = 0;
    for entry in fs::read_dir(&vdir.blobs_dir).map_err(BlobError::Io)? {
        let entry = entry.map_err(BlobError::Io)?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(".veylora-tmp-") {
            fs::remove_file(entry.path()).map_err(BlobError::Io)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Generate a random hex string of `len` bytes.
fn random_hex(len: usize) -> String {
    let mut bytes = vec![0u8; len / 2];
    getrandom::getrandom(&mut bytes).expect("OS RNG failure");
    hex_encode(&bytes)
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
    use crate::crypto::mvk::Mvk;
    use crate::storage::format::FileKey;
    use std::fs;
    use std::io::Cursor;

    fn setup_vault() -> (std::path::PathBuf, [u8; 16], FileKey) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::current_dir()
            .unwrap()
            .join(format!(
                "target/debug/test-vault-blob-{}-{}",
                std::process::id(),
                id
            ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let mvk = Mvk::generate().expect("mvk");
        let file_id = [0x42u8; 16];
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        (tmp, file_id, file_key)
    }

    #[test]
    fn write_and_read_blob_roundtrip() {
        let (tmp, file_id, file_key) = setup_vault();
        let plaintext = b"hello encrypted blob";
        let mut reader = Cursor::new(plaintext.as_slice());
        let encrypted_size = write_blob(&tmp, &file_id, &mut reader, &file_key).expect("write");
        assert!(encrypted_size > plaintext.len() as u64);

        let out = read_blob(&tmp, &file_id, 0, plaintext.len() as u64, &file_key).expect("read");
        assert_eq!(out, plaintext);

        // No temp files left behind
        let vdir = VaultDir::new(tmp.clone());
        let tmp_files: Vec<_> = fs::read_dir(&vdir.blobs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".veylora-tmp-"))
            .collect();
        assert!(tmp_files.is_empty(), "orphaned temp files remain after write");

        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_blob_range_multi_chunk() {
        let (tmp, file_id, file_key) = setup_vault();
        let plaintext: Vec<u8> = (0..crate::storage::format::CHUNK_SIZE * 3 + 7)
            .map(|i| (i % 256) as u8)
            .collect();
        let mut reader = Cursor::new(plaintext.as_slice());
        write_blob(&tmp, &file_id, &mut reader, &file_key).expect("write");

        let out = read_blob(&tmp, &file_id, 0, plaintext.len() as u64, &file_key).expect("read");
        assert_eq!(out, plaintext);

        // Range spanning chunks
        let out = read_blob(&tmp, &file_id, crate::storage::format::CHUNK_SIZE - 50, crate::storage::format::CHUNK_SIZE + 50, &file_key)
            .expect("read range");
        assert_eq!(out, plaintext[(crate::storage::format::CHUNK_SIZE - 50) as usize..(crate::storage::format::CHUNK_SIZE + 50) as usize]);
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_blob_nonexistent() {
        let (tmp, file_id, file_key) = setup_vault();
        let err = read_blob(&tmp, &file_id, 0, 100, &file_key).unwrap_err();
        assert!(matches!(err, BlobError::NotFound(_)));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_blob_invalid_range() {
        let (tmp, file_id, file_key) = setup_vault();
        let plaintext = b"hello";
        let mut reader = Cursor::new(plaintext.as_slice());
        write_blob(&tmp, &file_id, &mut reader, &file_key).unwrap();
        let err = read_blob(&tmp, &file_id, 100, 50, &file_key).unwrap_err();
        assert!(matches!(err, BlobError::InvalidRange));
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn cleanup_tmp_files_removes_orphans() {
        let (tmp, _file_id, _file_key) = setup_vault();
        let vdir = VaultDir::new(tmp.clone());
        vdir.ensure_dirs().unwrap();

        // Create orphan temp files
        fs::write(vdir.tmp_blob_path("orphan1"), b"junk").unwrap();
        fs::write(vdir.tmp_blob_path("orphan2"), b"junk").unwrap();

        // Create a real blob
        let file_id = [0x42u8; 16];
        let mvk = Mvk::generate().expect("mvk");
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        let plaintext = b"real blob";
        let mut reader = Cursor::new(plaintext.as_slice());
        write_blob(&tmp, &file_id, &mut reader, &file_key).unwrap();

        let removed = cleanup_tmp_files(&tmp).unwrap();
        assert_eq!(removed, 2);

        // Real blob untouched
        assert!(vdir.blob_path(&file_id).exists());
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn list_blobs_excludes_tmp() {
        let (tmp, _file_id, _file_key) = setup_vault();
        let vdir = VaultDir::new(tmp.clone());
        vdir.ensure_dirs().unwrap();

        fs::write(vdir.tmp_blob_path("orphan"), b"junk").unwrap();

        let file_id = [0x42u8; 16];
        let mvk = Mvk::generate().expect("mvk");
        let file_key = FileKey::derive(&mvk, &file_id).expect("derive");
        let plaintext = b"real blob";
        let mut reader = Cursor::new(plaintext.as_slice());
        write_blob(&tmp, &file_id, &mut reader, &file_key).unwrap();

        let blobs = list_blobs(&tmp).unwrap();
        assert_eq!(blobs, vec![hex_encode(&file_id)]);
        fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn delete_blob_removes_file() {
        let (tmp, file_id, file_key) = setup_vault();
        let plaintext = b"to be deleted";
        let mut reader = Cursor::new(plaintext.as_slice());
        write_blob(&tmp, &file_id, &mut reader, &file_key).unwrap();
        assert!(VaultDir::new(tmp.clone()).blob_path(&file_id).exists());

        delete_blob(&tmp, &file_id).unwrap();
        assert!(!VaultDir::new(tmp.clone()).blob_path(&file_id).exists());
        fs::remove_dir_all(&tmp).ok();
    }

    // FIX 3 — Simulated power-loss: write fails mid-way, no final file, tmp cleaned by startup_scan.
    #[test]
    fn power_loss_simulation() {
        use crate::storage::crash::startup_scan;

        let (tmp, file_id, file_key) = setup_vault();

        // A reader that fails after writing some bytes.
        struct FailingReader {
            remaining: usize,
        }
        impl Read for FailingReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.remaining == 0 {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "simulated power loss"));
                }
                let n = buf.len().min(self.remaining);
                self.remaining -= n;
                Ok(n)
            }
        }

        let mut failing = FailingReader { remaining: 100 }; // fail after 100 bytes
        let err = write_blob(&tmp, &file_id, &mut failing, &file_key).unwrap_err();
        match &err {
            BlobError::Io(_) | BlobError::Format(_) => {}, // expected
            other => panic!("expected Io or Format error, got {:?}", other),
        }

        // No final file at the blob path
        let vdir = VaultDir::new(tmp.clone());
        assert!(!vdir.blob_path(&file_id).exists(), "final path should not exist after failed write");
        let tmp_files: Vec<_> = fs::read_dir(&vdir.blobs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".veylora-tmp-"))
            .collect();
        assert!(!tmp_files.is_empty(), "orphaned temp file should exist after failed write");

        // startup_scan should clean it up
        let report = startup_scan(&tmp).expect("startup scan");
        assert_eq!(report.tmp_files_removed, 1, "startup_scan should remove the orphaned tmp file");

        // After cleanup, no temp files remain
        let tmp_files_after: Vec<_> = fs::read_dir(&vdir.blobs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".veylora-tmp-"))
            .collect();
        assert!(tmp_files_after.is_empty(), "temp files should be cleaned by startup_scan");

        // A subsequent write with the same file_id should succeed cleanly
        let plaintext = b"successful write after power loss";
        let mut reader = Cursor::new(plaintext.as_slice());
        write_blob(&tmp, &file_id, &mut reader, &file_key).expect("write after power loss");
        assert!(vdir.blob_path(&file_id).exists(), "blob should exist after successful write");

        fs::remove_dir_all(&tmp).ok();
    }
}
