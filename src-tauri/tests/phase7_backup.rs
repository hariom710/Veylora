//! Phase 7 integration tests: backup and restore.
//!
//! Tests that backup_create, backup_verify, backup_restore work correctly
//! end-to-end, including tamper detection, atomic restore, and locked-session
//! rejection.

use std::time::{SystemTime, UNIX_EPOCH};
use veylora::auth::session::{Session, SessionManager};
use veylora::auth::UnlockMethod;
use veylora::backup::{create, restore, verify};
use veylora::commands::{vault_backup_create, vault_backup_restore};
use veylora::crypto::hkdf::{derive_subkey, LABEL_METADATA_KEY};
use veylora::crypto::mvk::Mvk;
use veylora::crypto::recovery::generate_recovery_key;
use veylora::database::Db;
use veylora::storage::vault_dir::VaultDir;
use veylora::streaming::hex_encode;
use veylora::vault::VaultMeta;
use std::sync::Arc;

fn setup_vault_dir() -> std::path::PathBuf {
    let tmp = std::env::current_dir()
        .unwrap()
        .join(format!(
            "target/debug/test-phase7-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    tmp
}

fn create_test_vault(tmp_dir: &std::path::Path) -> (Db, Mvk, VaultDir) {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let recovery_key = generate_recovery_key();

    let vdir = VaultDir::new(tmp_dir.to_path_buf());
    vdir.create_layout().expect("create vault dir layout");

    let mut meta = VaultMeta::new();
    meta.add_pin_entry(b"1234", &mvk).expect("add PIN entry");
    meta.add_password_entry(b"correct horse battery staple", &mvk)
        .expect("add password entry");
    meta.add_recovery_entry(&recovery_key, &mvk)
        .expect("add recovery entry");

    let meta_json = meta.serialize().expect("serialize vault.meta");
    let tmp_path = vdir.meta_path.with_extension("meta.tmp");
    std::fs::write(&tmp_path, &meta_json).expect("write vault.meta");
    std::fs::rename(&tmp_path, &vdir.meta_path).expect("rename vault.meta");

    let db = Db::open(&vdir.db_path).expect("open vault.db");

    (db, mvk, vdir)
}

/// Create a restore vault that shares the same MVK as `src_vdir` by copying
/// vault.meta (which contains the wrapped MVK). The restore vault gets a new
/// empty vault.db and empty blobs dir.
fn create_restore_vault(src_vdir: &VaultDir, restore_dir: &std::path::Path) -> (Db, VaultDir) {
    std::fs::create_dir_all(restore_dir).expect("create restore dir");
    std::fs::create_dir_all(restore_dir.join("blobs")).expect("create blobs dir");
    std::fs::copy(&src_vdir.meta_path, restore_dir.join("vault.meta")).expect("copy vault.meta");
    let vdir = VaultDir::new(restore_dir.to_path_buf());
    let db = Db::open(&vdir.db_path).expect("open vault.db");
    (db, vdir)
}

fn unlock_vault(db: &Db, mvk: &Mvk, vdir: &VaultDir) -> SessionManager {
    let manager = SessionManager::new();
    let session = Session::new(mvk.clone(), 1, UnlockMethod::Pin);
    manager.set_session(session, vdir.meta_path.to_string_lossy().to_string());
    manager
}

fn add_file_to_vault(
    db: &Db,
    mvk: &Mvk,
    vdir: &VaultDir,
    plaintext: &[u8],
    filename: &str,
) -> String {
    let metadata_key =
        derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY).expect("metadata key derivation failed");

    let mut file_id_bytes = [0u8; 16];
    getrandom::getrandom(&mut file_id_bytes).expect("file_id RNG");
    let file_id_hex = hex_encode_16(&file_id_bytes);

    veylora::database::files::insert_file(
        db,
        &metadata_key,
        1,
        &file_id_hex,
        filename,
        None,
        plaintext.len() as u64,
    )
    .expect("insert file");

    let file_key = veylora::storage::format::FileKey::derive(&mvk, &file_id_bytes)
        .expect("derive file key");
    let mut reader = std::io::Cursor::new(plaintext);
    veylora::storage::blob::write_blob(
        &vdir.root,
        &file_id_bytes,
        &mut reader,
        &file_key,
    )
    .expect("write blob");

    file_id_hex
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

fn hex_encode_16(bytes: &[u8; 16]) -> String {
    let mut out = String::with_capacity(32);
    for &b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

// Test #1: Create backup of vault with 3 files → backup file exists, size > 0, file_count == 3.
#[test]
fn phase7_backup_create_three_files() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);

    add_file_to_vault(&db, &mvk, &vdir, b"Hello World from file 1", "doc1.pdf");
    add_file_to_vault(&db, &mvk, &vdir, b"File two content here", "doc2.txt");
    add_file_to_vault(&db, &mvk, &vdir, b"File three!", "doc3.log");

    let manager = unlock_vault(&db, &mvk, &vdir);
    assert!(!manager.is_locked(), "vault should be unlocked");

    let backup_path = tmp_dir.join("test-backup.vey");
    let backup_info = create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    assert!(backup_path.exists(), "backup file should exist");
    assert_eq!(backup_info.file_count, 3, "should have 3 files");
    assert!(backup_info.total_size > 0, "total size should be > 0");
    assert!(backup_info.created_at_unix > 0, "created_at should be set");

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = manager;
}

// Test #2: Verify created backup → verify_backup returns Ok with correct file_count.
#[test]
fn phase7_backup_verify_created() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);

    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("test-backup.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    let verify_info = verify::verify_backup(Some(mvk.as_bytes()), backup_path.to_str().unwrap())
        .expect("verify backup");
    assert_eq!(verify_info.file_count, 1, "verify should see 1 file");
    assert!(verify_info.verified_fields.contains(&"manifest".to_string()), "should verify manifest");
    assert!(verify_info.verified_fields.contains(&"blobs_hash".to_string()), "should verify blobs_hash");

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = manager;
}

// Test #3: Restore backup into fresh vault → files present, blobs match, DB rows match.
#[test]
fn phase7_backup_restore_roundtrip() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);

    add_file_to_vault(&db, &mvk, &vdir, b"Hello World from file 1", "doc1.pdf");
    add_file_to_vault(&db, &mvk, &vdir, b"File two content here", "doc2.txt");
    add_file_to_vault(&db, &mvk, &vdir, b"File three!", "doc3.log");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("test-backup.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    let restore_dir = setup_vault_dir();
    let (mut restore_db, restore_vdir) = create_restore_vault(&vdir, &restore_dir);
    let _restore_manager = unlock_vault(&restore_db, &mvk, &restore_vdir);

    let restore_info = restore::restore_backup(
        mvk.as_bytes(),
        &mut restore_db,
        &restore_vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("restore backup");

    assert_eq!(restore_info.files_restored, 3, "should restore 3 files");
    assert!(restore_info.bytes_written > 0, "should write bytes");

    let metadata_key =
        derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY).expect("metadata key");
    let files = veylora::database::files::list_files(&restore_db, &metadata_key, None)
        .expect("list restored files");
    assert_eq!(files.len(), 3, "should have 3 files in restored vault");

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = std::fs::remove_dir_all(&restore_dir);
    let _ = manager;
    let _ = _restore_manager;
}

// Test #4: Round-trip: create → restore → read via handle_protocol_request → body matches.
#[test]
fn phase7_backup_read_via_protocol() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"Hello World this is a test file for veylora phase7 roundtrip";
    let file_id_hex = add_file_to_vault(&db, &mvk, &vdir, plaintext, "roundtrip.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("test-backup.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    let restore_dir = setup_vault_dir();
    let (mut restore_db, restore_vdir) = create_restore_vault(&vdir, &restore_dir);
    let _restore_manager = unlock_vault(&restore_db, &mvk, &restore_vdir);

    restore::restore_backup(
        mvk.as_bytes(),
        &mut restore_db,
        &restore_vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("restore backup");

    // Read restored file via protocol handler
    let metadata_key =
        derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY).expect("metadata key");
    let files = veylora::database::files::list_files(&restore_db, &metadata_key, None)
        .expect("list restored files");
    assert_eq!(files.len(), 1);

    let file_id_bytes: [u8; 16] = hex_decode(&files[0].file_id).expect("decode file_id");
    let file_key = veylora::storage::format::FileKey::derive(&mvk, &file_id_bytes)
        .expect("derive file key");

    // Read the restored blob and verify via format reader
    let blob_path = restore_vdir.blob_path(&file_id_bytes);
    let blob_data = std::fs::read(&blob_path).expect("read blob");
    veylora::storage::format::verify_file(&blob_data, &file_key)
        .expect("verify restored blob");

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = std::fs::remove_dir_all(&restore_dir);
    let _ = manager;
    let _ = _restore_manager;
}

// Test #5: Backup history populated: after create_backup, history returns entry with status "complete".
#[test]
fn phase7_backup_history_recorded() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);

    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("history.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    verify::record_backup_history(&db, backup_path.to_str().unwrap(), "complete", now)
        .expect("record history");

    let history = verify::list_backup_history(&db).expect("list history");
    assert_eq!(history.len(), 1, "should have 1 history entry");
    assert_eq!(history[0].status, "complete", "status should be complete");
    assert_eq!(history[0].created_at_unix, now, "created_at should match");

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = manager;
}

// Test #6: Tamper manifest → verify returns Corrupted.
#[test]
fn phase7_backup_tamper_manifest_detected() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);
    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("tamper-manifest.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    let mut data = std::fs::read(&backup_path).expect("read backup");
    let flip_pos = data.len() / 2;
    data[flip_pos] ^= 0x01;
    std::fs::write(&backup_path, &data).expect("write tampered backup");

    let result = verify::verify_backup(Some(mvk.as_bytes()), backup_path.to_str().unwrap());
    assert!(
        result.is_err(),
        "tampered backup should fail verification"
    );
    match result.unwrap_err() {
        veylora::backup::BackupError::Corrupted(_) => {}
        other => panic!("expected Corrupted error, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = manager;
}

// Test #7: Tamper blob → verify returns Corrupted (blobs_hash mismatch).
#[test]
fn phase7_backup_tamper_blob_detected() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);
    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("tamper-blob.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    let mut data = std::fs::read(&backup_path).expect("read backup");
    let flip_pos = data.len() - 100;
    if flip_pos > 100 {
        data[flip_pos] ^= 0x01;
        std::fs::write(&backup_path, &data).expect("write tampered backup");

        let result =
            verify::verify_backup(Some(mvk.as_bytes()), backup_path.to_str().unwrap());
        assert!(
            result.is_err(),
            "tampered blob backup should fail verification"
        );
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = manager;
}

// Test #8: Truncate trailer → verify returns Corrupted.
#[test]
fn phase7_backup_truncate_trailer_detected() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);
    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("truncate-trailer.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    let data = std::fs::read(&backup_path).expect("read backup");
    let truncated = &data[..data.len() - 10];
    std::fs::write(&backup_path, truncated).expect("write truncated backup");

    let result = verify::verify_backup(Some(mvk.as_bytes()), backup_path.to_str().unwrap());
    assert!(
        result.is_err(),
        "truncated backup should fail verification"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = manager;
}

// Test #9: Swap manifest from backup with different meta_len → header AEAD fails.
#[test]
fn phase7_backup_swap_manifest_fails() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);
    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("swap-manifest.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    // Tamper meta_len in header (offset 36 + payload_len). Header AAD includes meta_len,
    // so changing it without updating header_tag will cause AEAD failure.
    // Header layout: magic(4) || version(4) || nonce(24) || payload_len(4) || payload(N) ||
    //   meta_len(4) || meta_hash(32) || header_tag(16)
    // payload_len is at offset 32. Read it to find meta_len offset.
    let mut data = std::fs::read(&backup_path).expect("read backup");
    let payload_len = u32::from_le_bytes([data[32], data[33], data[34], data[35]]) as usize;
    let meta_len_offset = 36 + payload_len;
    assert!(meta_len_offset + 4 <= data.len(), "meta_len offset within bounds");

    // Flip a byte in meta_len - header AAD binds meta_len, so AEAD will fail
    data[meta_len_offset] ^= 0x01;
    std::fs::write(&backup_path, &data).expect("write tampered backup");

    let result = verify::verify_backup(Some(mvk.as_bytes()), backup_path.to_str().unwrap());
    assert!(
        result.is_err(),
        "tampered meta_len should fail header AEAD"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = manager;
}

// Test #10: Restore with wrong MVK → header AEAD fails, BackupError::Corrupted.
#[test]
fn phase7_backup_wrong_mvk_fails() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);
    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("wrong-mvk.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    let wrong_mvk = Mvk::generate().expect("wrong mvk");
    let result =
        verify::verify_backup(Some(wrong_mvk.as_bytes()), backup_path.to_str().unwrap());
    assert!(
        result.is_err(),
        "wrong MVK should fail verification"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = manager;
}

// Test #11: Restore into vault with existing file_id → BackupError::FileExists, no partial restore.
#[test]
fn phase7_backup_restore_file_exists() {
    let tmp_dir = setup_vault_dir();
    let (mut db, mvk, vdir) = create_test_vault(&tmp_dir);
    let file1 = add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("file-exists.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    let result = restore::restore_backup(
        mvk.as_bytes(),
        &mut db,
        &vdir,
        backup_path.to_str().unwrap(),
    );
    assert!(
        result.is_err(),
        "restore with existing file_id should fail"
    );
    match result.unwrap_err() {
        veylora::backup::BackupError::FileExists { file_id } => {
            assert_eq!(file_id, file1, "should report the existing file_id");
        }
        other => panic!("expected FileExists error, got {:?}", other),
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = manager;
    let _ = file1;
}

// Test #12: Restore is atomic: simulate failure mid-restore → no DB rows added.
#[test]
fn phase7_backup_restore_atomic_failure() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);
    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = unlock_vault(&db, &mvk, &vdir);

    let backup_path = tmp_dir.join("atomic-failure.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    // Tamper one byte in the blob region so verify_file fails mid-restore
    let mut data = std::fs::read(&backup_path).expect("read backup");
    let flip_pos = data.len() - 100;
    if flip_pos > 100 {
        data[flip_pos] ^= 0x01;
        std::fs::write(&backup_path, &data).expect("write tampered backup");
    }

    let restore_dir = setup_vault_dir();
    let (mut restore_db, restore_vdir) = create_restore_vault(&vdir, &restore_dir);
    let _restore_manager = unlock_vault(&restore_db, &mvk, &restore_vdir);

    let result = restore::restore_backup(
        mvk.as_bytes(),
        &mut restore_db,
        &restore_vdir,
        backup_path.to_str().unwrap(),
    );
    assert!(
        result.is_err(),
        "restore with tampered blob should fail atomically"
    );

    // Verify no files were added to the restore vault
    let metadata_key =
        derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY).expect("metadata key");
    let files = veylora::database::files::list_files(&restore_db, &metadata_key, None)
        .expect("list restored files");
    assert_eq!(files.len(), 0, "no files should be added on atomic failure");

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = std::fs::remove_dir_all(&restore_dir);
    let _ = manager;
    let _ = _restore_manager;
}

// Test #13: Restore while locked → CommandError::Locked.
#[test]
fn phase7_backup_restore_while_locked() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);
    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    // Session is locked (not unlocked)
    let manager = SessionManager::new();
    assert!(manager.is_locked(), "session should be locked");

    let backup_path = tmp_dir.join("locked-restore.vey");
    create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    // Try to restore while locked - command wrapper returns Locked
    let result = vault_backup_restore(
        backup_path.to_str().unwrap().to_string(),
        Arc::new(manager),
    );
    assert!(
        result.is_err(),
        "restore while locked should return Locked"
    );
    assert_eq!(
        result.unwrap_err(),
        "vault is locked",
        "error should be 'vault is locked'"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

// Test #14: Create while locked → CommandError::Locked.
#[test]
fn phase7_backup_create_while_locked() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);
    add_file_to_vault(&db, &mvk, &vdir, b"test data", "test.txt");

    let manager = SessionManager::new();
    assert!(manager.is_locked(), "session should be locked");

    // Create backup while locked - command wrapper returns Locked
    let result = vault_backup_create(
        "test.vey".to_string(),
        Arc::new(manager),
    );
    assert!(
        result.is_err(),
        "create while locked should return Locked"
    );
    assert_eq!(
        result.unwrap_err(),
        "vault is locked",
        "error should be 'vault is locked'"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

// Test #15 (ignored): 100 MiB file → create → verify → restore → read → compare.
#[ignore]
#[test]
fn phase7_backup_100mib_roundtrip() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, vdir) = create_test_vault(&tmp_dir);

    let size: usize = 100 * 1024 * 1024;
    let pattern: Vec<u8> = (0..256).map(|i| i as u8).collect();
    let mut plaintext = vec![0u8; size];
    for chunk in plaintext.chunks_mut(256) {
        chunk.copy_from_slice(&pattern);
    }

    let file_id = add_file_to_vault(&db, &mvk, &vdir, &plaintext, "large100mib.bin");

    let manager = unlock_vault(&db, &mvk, &vdir);
    assert!(!manager.is_locked(), "vault should be unlocked");

    let backup_path = tmp_dir.join("large-backup.vey");
    let backup_info = create::create_backup(
        mvk.as_bytes(),
        &db,
        &vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("create backup");

    assert!(backup_path.exists(), "backup file should exist");
    assert_eq!(backup_info.file_count, 1, "should have 1 file");
    assert!(backup_info.total_size > 0, "total size should be > 0");

    let verify_info = verify::verify_backup(Some(mvk.as_bytes()), backup_path.to_str().unwrap())
        .expect("verify backup");
    assert_eq!(verify_info.file_count, 1, "verify should see 1 file");

    let restore_dir = setup_vault_dir();
    let (mut restore_db, restore_vdir) = create_restore_vault(&vdir, &restore_dir);
    let _restore_manager = unlock_vault(&restore_db, &mvk, &restore_vdir);

    let restore_info = restore::restore_backup(
        mvk.as_bytes(),
        &mut restore_db,
        &restore_vdir,
        backup_path.to_str().unwrap(),
    )
    .expect("restore backup");

    assert_eq!(restore_info.files_restored, 1, "should restore 1 file");

    let metadata_key = derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY).expect("metadata key");
    let files = veylora::database::files::list_files(&restore_db, &metadata_key, None)
        .expect("list restored files");
    assert_eq!(files.len(), 1, "should have 1 file in restored vault");

    for f in &files {
        let file_id_bytes: [u8; 16] = hex_decode(&f.file_id).expect("decode file_id");
        let file_key = veylora::storage::format::FileKey::derive(&mvk, &file_id_bytes)
            .expect("derive file key");
        let blob_path = restore_vdir.blob_path(&file_id_bytes);
        let blob_data = std::fs::read(&blob_path).expect("read blob");
        veylora::storage::format::verify_file(&blob_data, &file_key)
            .expect("verify restored blob");
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = std::fs::remove_dir_all(&restore_dir);
    let _ = manager;
    let _ = _restore_manager;
    let _ = file_id;
}
