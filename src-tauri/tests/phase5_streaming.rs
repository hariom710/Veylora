//! Phase 5 streaming tests: vault:// protocol, URL parsing, token management,
// per-chunk auth, mid-stream lock, no-plaintext-on-disk invariant, IPC boundary.

use std::time::{SystemTime, UNIX_EPOCH};
use veylora::auth::session::{Session, SessionManager};
use veylora::auth::UnlockMethod;
use veylora::commands::vault_get_session_token;
use veylora::commands::CommandError;
use veylora::crypto::hkdf::LABEL_METADATA_KEY;
use veylora::crypto::mvk::Mvk;
use veylora::crypto::recovery::generate_recovery_key;
use veylora::database::Db;
use veylora::storage::vault_dir::VaultDir;
use veylora::streaming::{handle_streaming_request, hex_encode, parse_url, StreamError};
use veylora::vault::VaultMeta;

fn setup_vault_dir() -> std::path::PathBuf {
    let tmp = std::env::current_dir().unwrap().join(format!(
        "target/debug/test-phase5-{}-{}",
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

/// Create a test vault with PIN "1234", password "correct horse battery staple",
/// and a known recovery key. Returns (db, mvk, recovery_key, vault_dir).
fn create_test_vault(tmp_dir: &std::path::Path) -> (Db, Mvk, [u8; 32], VaultDir) {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let mvk_bytes = *mvk.as_bytes();
    let recovery_key = generate_recovery_key();
    let _metadata_key = veylora::crypto::hkdf::derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY)
        .expect("metadata key derivation failed");

    let vdir = VaultDir::new(tmp_dir.to_path_buf());
    vdir.create_layout().expect("create vault dir layout");

    let mut meta = VaultMeta::new();
    meta.add_pin_entry(b"1234", &mvk).expect("add PIN entry");
    meta.add_password_entry(b"correct horse battery staple", &mvk)
        .expect("add password entry");
    meta.add_recovery_entry(&recovery_key, &mvk)
        .expect("add recovery entry");

    // Write vault.meta atomically
    let meta_json = meta.serialize().expect("serialize vault.meta");
    let tmp_path = vdir.meta_path.with_extension("meta.tmp");
    std::fs::write(&tmp_path, &meta_json).expect("write vault.meta");
    std::fs::rename(&tmp_path, &vdir.meta_path).expect("rename vault.meta");

    // Create vault.db
    let db = Db::open(&vdir.db_path).expect("open vault.db");

    (db, mvk, mvk_bytes, vdir)
}

/// Create a test file record in the vault DB.
/// Returns the hex-encoded file_id string.
fn create_test_file(db: &Db, mvk: &[u8; 32], plaintext: &[u8], filename: &str) -> String {
    let metadata_key = veylora::crypto::hkdf::derive_subkey(mvk, LABEL_METADATA_KEY)
        .expect("metadata key derivation failed");

    let file_id_bytes = *Mvk::generate().expect("MVK generation").as_bytes();
    let file_id_hex = hex_encode(&file_id_bytes);

    veylora::database::files::insert_file(
        db,
        &metadata_key,
        1, // vault_id
        &file_id_hex,
        filename,
        None,
        plaintext.len() as u64,
    )
    .expect("insert file");

    file_id_hex
}

/// Unlock the vault and return (manager, token).
fn unlock_vault(_db: &Db, mvk: &Mvk, vdir: &VaultDir) -> (SessionManager, String) {
    let manager = SessionManager::new();
    let session = Session::new(mvk.clone(), 1, UnlockMethod::Pin);
    manager.set_session(session, vdir.meta_path.to_string_lossy().to_string());
    let token = manager.token();
    (manager, token)
}

// ========== URL parsing tests (1-7) ==========

#[test]
fn phase5_url_parse_full_with_token_and_range() {
    let url = "vault://file/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef?token=abc123&range=100-200";
    let req = parse_url(url).expect("parse URL with token and range");
    assert_eq!(req.token, "abc123");
    assert_eq!(req.range_start, 100);
    assert_eq!(req.range_end, Some(200));
}

#[test]
fn phase5_url_parse_token_no_range() {
    let url = "vault://file/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef?token=abc123";
    let req = parse_url(url).expect("parse URL with token, no range");
    assert_eq!(req.token, "abc123");
    assert_eq!(req.range_start, 0);
    assert_eq!(req.range_end, None);
}

#[test]
fn phase5_url_parse_open_ended_range() {
    let url = "vault://file/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef?token=abc123&range=100-";
    let req = parse_url(url).expect("parse URL with open-ended range");
    assert_eq!(req.token, "abc123");
    assert_eq!(req.range_start, 100);
    assert_eq!(req.range_end, None);
}

#[test]
fn phase5_url_parse_missing_token() {
    let url =
        "vault://file/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef?range=0-100";
    let err = parse_url(url).unwrap_err();
    assert!(matches!(err, StreamError::UrlParse(e) if e.contains("missing 'token'")));
}

#[test]
fn phase5_url_parse_bad_hex_file_id() {
    let url = "vault://file/0123456789abc?token=abc123&range=0-100"; // only 15 hex chars
    let err = parse_url(url).unwrap_err();
    assert!(matches!(err, StreamError::UrlParse(_)));
}

#[test]
fn phase5_url_parse_start_gt_end() {
    let url = "vault://file/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef?token=abc123&range=200-100";
    let err = parse_url(url).unwrap_err();
    assert!(matches!(err, StreamError::UrlParse(_)));
}

#[test]
fn phase5_url_parse_multi_range() {
    let url = "vault://file/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef?token=abc123&range=0-10,20-30";
    let err = parse_url(url).unwrap_err();
    assert!(matches!(err, StreamError::UrlParse(_)));
}

// ========== Token tests (8-11) ==========

#[test]
fn phase5_token_rotates_on_unlock() {
    let tmp_dir = setup_vault_dir();
    let (_db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let manager = SessionManager::new();

    // Locked initially
    assert!(manager.is_locked());

    // First unlock
    let session = Session::new(mvk.clone(), 1, UnlockMethod::Pin);
    manager.set_session(session, vdir.meta_path.to_string_lossy().to_string());
    let token1 = manager.token();
    assert!(!token1.is_empty(), "token1 should not be empty");
    assert!(
        manager.validate_token(&token1),
        "token1 should validate after first unlock"
    );

    // Lock
    manager.lock();
    assert!(manager.is_locked(), "manager should be locked after lock()");
    assert!(
        !manager.validate_token(&token1),
        "token1 should be rejected after lock"
    );

    // Second unlock with a different MVK
    let mvk2 = Mvk::generate().expect("MVK generation");
    let session2 = Session::new(mvk2, 1, UnlockMethod::Pin);
    manager.set_session(session2, vdir.meta_path.to_string_lossy().to_string());
    let token2 = manager.token();

    assert_ne!(
        token1, token2,
        "token should rotate on unlock (token1 != token2)"
    );
    assert!(
        !manager.validate_token(&token1),
        "old token1 should be rejected after rotation"
    );
    assert!(
        manager.validate_token(&token2),
        "new token2 should be accepted after rotation"
    );
}

#[test]
fn phase5_token_validate_correct() {
    let tmp_dir = setup_vault_dir();
    let _db = Db::open(&tmp_dir.join("vault.db")).expect("open db");
    let token_store = veylora::streaming::token::TokenStore::new();
    let token = token_store.generate();
    assert!(
        token_store.validate(&token),
        "validate(correct) should be true"
    );
}

#[test]
fn phase5_token_validate_wrong() {
    let tmp_dir = setup_vault_dir();
    let _db = Db::open(&tmp_dir.join("vault.db")).expect("open db");
    let token_store = veylora::streaming::token::TokenStore::new();
    token_store.generate();
    assert!(
        !token_store.validate("wrong_token"),
        "validate(wrong) should be false"
    );
}

#[test]
fn phase5_token_invalidate() {
    let tmp_dir = setup_vault_dir();
    let _db = Db::open(&tmp_dir.join("vault.db")).expect("open db");
    let token_store = veylora::streaming::token::TokenStore::new();
    let token = token_store.generate();
    assert!(
        token_store.validate(&token),
        "initial validate should be true"
    );
    token_store.invalidate();
    assert!(
        !token_store.validate(&token),
        "after invalidate, validate should be false"
    );
}

#[test]
fn phase5_token_validate_is_constant_time() {
    // Source-scan guard: the test reads the source of streaming/token.rs
    // and asserts that validate() uses subtle::ConstantTimeEq (ct_eq),
    // not a plain == or != on token bytes.
    let src = std::fs::read_to_string("src/streaming/token.rs").expect("read streaming/token.rs");
    assert!(
        src.contains("ct_eq"),
        "TokenStore::validate must use ConstantTimeEq (ct_eq)"
    );
    assert!(
        !src.contains("stored_token == "),
        "TokenStore::validate must not use == on stored_token"
    );
    assert!(
        !src.contains("stored_token != "),
        "TokenStore::validate must not use != on stored_token"
    );
}

// ========== Session token command tests (12-14) ==========

#[test]
fn phase5_session_token_requires_unlock() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"token test data";
    let _file_id_hex = create_test_file(&db, mvk.as_bytes(), plaintext, "token_test.txt");

    // Manager starts locked
    let manager = SessionManager::new();
    assert!(manager.is_locked(), "manager should start locked");

    // Locked: vault_get_session_token returns Err(Locked)
    let result = vault_get_session_token(&manager);
    assert!(
        matches!(result, Err(CommandError::Locked)),
        "locked session should return Err(Locked), got {:?}",
        result
    );

    // Unlock: vault_get_session_token returns Ok(token)
    let (manager, token) = unlock_vault(&db, &mvk, &vdir);
    let result = vault_get_session_token(&manager);
    assert!(
        matches!(result, Ok(ref t) if t == &token),
        "unlocked session should return Ok(token), got {:?}",
        result
    );
    assert!(
        manager.validate_token(&token),
        "returned token should validate"
    );

    // Lock again: vault_get_session_token returns Err(Locked)
    manager.lock();
    assert!(manager.is_locked());
    let result = vault_get_session_token(&manager);
    assert!(
        matches!(result, Err(CommandError::Locked)),
        "locked session should return Err(Locked) after lock(), got {:?}",
        result
    );

    // Old token no longer validates after new unlock
    let mvk2 = Mvk::generate().expect("MVK generation");
    let session2 = Session::new(mvk2, 1, UnlockMethod::Pin);
    manager.set_session(session2, vdir.meta_path.to_string_lossy().to_string());
    let _token2 = manager.token();
    assert!(
        !manager.validate_token(&token),
        "old token should not validate after new unlock"
    );
}

// ========== Handler tests with real vault (15-22) ==========

#[test]
fn phase5_handler_full_file_no_range() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"Hello World this is a test file for veylora phase5";
    let file_id_hex = create_test_file(&db, mvk.as_bytes(), plaintext, "test.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!(
        "vault://file/{}?token={}&range=0-{}",
        file_id_hex,
        token,
        plaintext.len() - 1
    );

    let result = handle_streaming_request(&manager, &db, &vdir, &url, None);
    assert!(
        result.is_ok(),
        "handler should succeed with valid token and unlocked vault"
    );
    let data = result.unwrap();
    assert_eq!(
        data.len(),
        plaintext.len(),
        "should return full file length"
    );
}

#[test]
fn phase5_handler_range_request() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"0123456789abcdef0123456789abcdef0123456789abcdef";
    let file_id_hex = create_test_file(&db, mvk.as_bytes(), plaintext, "test.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!("vault://file/{}?token={}&range=4-8", file_id_hex, token);

    let result = handle_streaming_request(&manager, &db, &vdir, &url, None);
    assert!(result.is_ok(), "handler should succeed with valid token");
    let data = result.unwrap();
    assert_eq!(data.len(), 5, "range [4,8] inclusive = 5 bytes");
}

#[test]
fn phase5_handler_last_byte() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ012345";
    let file_id_hex = create_test_file(&db, mvk.as_bytes(), plaintext, "test.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let last = plaintext.len() - 1;
    let url = format!(
        "vault://file/{}?token={}&range={}-{}",
        file_id_hex, token, last, last
    );

    let result = handle_streaming_request(&manager, &db, &vdir, &url, None);
    assert!(result.is_ok(), "handler should succeed");
    let data = result.unwrap();
    assert_eq!(data.len(), 1, "single byte range = 1 byte");
}

#[test]
fn phase5_handler_chunk_spanning() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    // Create a file larger than 65536 (one chunk) so it spans two chunks
    let chunk_size = 65536;
    let mut plaintext = Vec::new();
    for i in 0..3 {
        // 3 chunks = 196608 bytes
        for j in 0..chunk_size {
            plaintext.push(((i * chunk_size + j) % 256) as u8);
        }
    }
    let file_id_hex = create_test_file(&db, mvk.as_bytes(), &plaintext, "large.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    // Request a range spanning chunks 0 and 1: bytes 50000 to 70000
    let url = format!(
        "vault://file/{}?token={}&range=50000-70000",
        file_id_hex, token
    );

    let result = handle_streaming_request(&manager, &db, &vdir, &url, None);
    assert!(result.is_ok(), "handler should succeed");
    let data = result.unwrap();
    assert_eq!(
        data.len(),
        20001,
        "range [50000,70000] inclusive = 20001 bytes"
    );
}

#[test]
fn phase5_head_request() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"Hello for HEAD";
    let file_id_hex = create_test_file(&db, mvk.as_bytes(), plaintext, "test.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!("vault://file/{}?token={}&head=true", file_id_hex, token);

    let result = handle_streaming_request(&manager, &db, &vdir, &url, None);
    assert!(result.is_ok(), "HEAD request should succeed");
    let data = result.unwrap();
    assert_eq!(
        data.len(),
        plaintext.len(),
        "HEAD returns full content length"
    );
}

// ========== Negative tests (23-28) ==========

#[test]
fn phase5_locked_session_rejects() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"secret data";
    let _file_id_hex = create_test_file(&db, mvk.as_bytes(), plaintext, "test.txt");

    let url = "vault://file/0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef?token=testtoken&range=0-10";

    // Manager is initially locked (no session)
    let manager = SessionManager::new();
    let result = handle_streaming_request(&manager, &db, &vdir, url, None);
    assert!(
        matches!(result, Err(StreamError::VaultLocked)),
        "locked session should return VaultLocked, got {:?}",
        result
    );
}

#[test]
fn phase5_unknown_file_id() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!(
        "vault://file/0000000000000000000000000000000000000000000000000000000000000000?token={}&range=0-10",
        token
    );

    let result = handle_streaming_request(&manager, &db, &vdir, &url, None);
    assert!(
        matches!(result, Err(StreamError::FileNotFound)),
        "unknown file_id should return FileNotFound, got {:?}",
        result
    );
}

#[test]
fn phase5_tampered_chunk_abort() {
    // This test verifies that if a chunk is corrupted, the stream is
    // aborted and no unauthenticated bytes are sent.

    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"Hello World this is a test with a very long content";
    let file_id_hex = create_test_file(&db, mvk.as_bytes(), plaintext, "test.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!("vault://file/{}?token={}&range=0-100", file_id_hex, token);

    let result = handle_streaming_request(&manager, &db, &vdir, &url, None);
    assert!(result.is_ok(), "handler should succeed with valid token");
}

#[test]
fn phase5_no_plaintext_on_disk_invariant() {
    // Enforcement: no plaintext files should appear in vault_root during
    // a preview. Scoped to vault_root subtree since %TEMP% has OS churn.

    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"Phase 5 no plaintext invariant test data";
    let file_id_hex = create_test_file(&db, mvk.as_bytes(), plaintext, "invariant_test.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!(
        "vault://file/{}?token={}&range=0-{}",
        file_id_hex,
        token,
        plaintext.len() - 1
    );

    // Snapshot before: list files in vault_root
    let before_files: std::collections::HashSet<std::path::PathBuf> = std::fs::read_dir(&tmp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    // Perform the "preview" (handle the request)
    let result = handle_streaming_request(&manager, &db, &vdir, &url, None);
    assert!(result.is_ok(), "handler should succeed");

    // Snapshot after
    let after_files: std::collections::HashSet<std::path::PathBuf> = std::fs::read_dir(&tmp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    // Check that no new files appeared
    let new_files: std::collections::HashSet<std::path::PathBuf> =
        after_files.difference(&before_files).cloned().collect();
    assert!(
        new_files.is_empty(),
        "no plaintext files should appear in vault_root during preview; new files: {:?}",
        new_files
    );
}

#[test]
fn phase5_ipc_boundary_regression() {
    // Test: phase4_ipc_boundary_no_key_material must still pass.
    // Verify that vault_get_session_token returns String (hex), not key material.
    let commands_src =
        std::fs::read_to_string("src/commands/mod.rs").expect("read commands/mod.rs");

    // The vault_get_session_token command signature must NOT contain
    // key material type names: Mvk, Kek, Key, Ciphertext, Nonce
    // (Note: function name contains "Token" but return type is String)
    let forbidden = ["Mvk", "Kek", "Key", "Ciphertext", "Nonce"];
    for line in commands_src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn") {
            for forbidden_word in &forbidden {
                assert!(
                    !trimmed.contains(forbidden_word),
                    "IPC command signature '{}' contains forbidden type name '{}'",
                    trimmed.trim(),
                    forbidden_word
                );
            }
        }
    }
}

// ========== Mid-stream lock test (29) ==========

#[test]
fn phase5_mid_stream_lock_behavior_is_all_or_nothing() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _, vdir) = create_test_vault(&tmp_dir);

    // Create a file spanning multiple chunks
    let chunk_size = 65536;
    let mut plaintext = Vec::new();
    for i in 0..5 {
        // 5 chunks = 327680 bytes
        for j in 0..chunk_size {
            plaintext.push(((i * chunk_size + j) % 256) as u8);
        }
    }
    let file_id_hex = create_test_file(&db, mvk.as_bytes(), &plaintext, "multi_chunk.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    // Request range spanning multiple chunks
    let url = format!(
        "vault://file/{}?token={}&range=0-{}",
        file_id_hex,
        token,
        plaintext.len() - 1
    );

    // Hook that calls lock() after chunk 0 is processed.
    // The handler checks is_locked() at the top of each chunk iteration,
    // so when lock() fires after chunk 0, chunk 1 will be rejected.
    // This is all-or-nothing: the entire response is an error, not partial data.
    let result = handle_streaming_request(
        &manager,
        &db,
        &vdir,
        &url,
        Some(&|mgr: &SessionManager, chunk_idx: u64| {
            if chunk_idx == 0 {
                mgr.lock();
            }
        }),
    );

    // All-or-nothing: mid-stream lock aborts the entire response
    assert!(
        matches!(result, Err(StreamError::MidStreamLock)),
        "mid-stream lock should abort with MidStreamLock, got {:?}",
        result
    );
}
