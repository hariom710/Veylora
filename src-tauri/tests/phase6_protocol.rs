//! Phase 6 integration tests: vault:// protocol handler.
//! Tests that handle_protocol_request returns correct bytes for known files.

use std::time::{SystemTime, UNIX_EPOCH};
use veylora::auth::session::Session;
use veylora::auth::session::SessionManager;
use veylora::auth::UnlockMethod;
use veylora::crypto::hkdf::LABEL_METADATA_KEY;
use veylora::crypto::mvk::Mvk;
use veylora::crypto::recovery::generate_recovery_key;
use veylora::database::Db;
use veylora::storage::vault_dir::VaultDir;
use veylora::streaming::hex_encode;
use veylora::streaming::protocol::handle_protocol_request;
use veylora::vault::VaultMeta;

fn setup_vault_dir() -> std::path::PathBuf {
    let tmp = std::env::current_dir().unwrap().join(format!(
        "target/debug/test-phase6-{}-{}",
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

    let meta_json = meta.serialize().expect("serialize vault.meta");
    let tmp_path = vdir.meta_path.with_extension("meta.tmp");
    std::fs::write(&tmp_path, &meta_json).expect("write vault.meta");
    std::fs::rename(&tmp_path, &vdir.meta_path).expect("rename vault.meta");

    let db = Db::open(&vdir.db_path).expect("open vault.db");

    (db, mvk, mvk_bytes, vdir)
}

/// Create a test file record plus its real format-v1 encrypted blob.
/// Returns the hex-encoded file_id string (32 hex chars = 16 bytes).
fn create_test_file(
    db: &Db,
    mvk: &Mvk,
    vdir: &VaultDir,
    plaintext: &[u8],
    filename: &str,
) -> String {
    let metadata_key = veylora::crypto::hkdf::derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY)
        .expect("metadata key derivation failed");

    let mut file_id_bytes = [0u8; 16];
    getrandom::getrandom(&mut file_id_bytes).expect("file_id RNG");
    let file_id_hex = hex_encode(&file_id_bytes);

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

    // The protocol handler must decrypt real bytes — write the actual blob.
    let file_key =
        veylora::storage::format::FileKey::derive(mvk, &file_id_bytes).expect("derive file key");
    let mut reader = std::io::Cursor::new(plaintext);
    veylora::storage::blob::write_blob(&vdir.root, &file_id_bytes, &mut reader, &file_key)
        .expect("write blob");

    file_id_hex
}

fn unlock_vault(_db: &Db, mvk: &Mvk, vdir: &VaultDir) -> (SessionManager, String) {
    let manager = SessionManager::new();
    let session = Session::new(mvk.clone(), 1, UnlockMethod::Pin);
    manager.set_session(session, vdir.meta_path.to_string_lossy().to_string());
    let token = manager.token();
    (manager, token)
}

#[test]
fn phase6_protocol_handler_serves_file_bytes() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _mvk_bytes, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"Hello World this is a test file for veylora phase6";
    let file_id_hex = create_test_file(&db, &mvk, &vdir, plaintext, "test.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!(
        "vault://file/{}?token={}&range=0-{}",
        file_id_hex,
        token,
        plaintext.len() - 1
    );

    let request = tauri::http::Request::builder()
        .uri(url)
        .body(Vec::new())
        .unwrap();

    let response = handle_protocol_request(&manager, &db, &vdir, request);

    assert_eq!(response.status(), 200);
    let body = response.body();
    assert_eq!(
        body,
        &plaintext[..],
        "should return the real decrypted plaintext"
    );
}

#[test]
fn phase6_protocol_handler_range_request() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _mvk_bytes, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"0123456789abcdef0123456789abcdef0123456789abcdef";
    let file_id_hex = create_test_file(&db, &mvk, &vdir, plaintext, "test.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!("vault://file/{}?token={}&range=4-8", file_id_hex, token);

    let request = tauri::http::Request::builder()
        .uri(url)
        .body(Vec::new())
        .unwrap();

    let response = handle_protocol_request(&manager, &db, &vdir, request);

    assert_eq!(response.status(), 200);
    let body = response.body();
    assert_eq!(
        body,
        &plaintext[4..9],
        "should return bytes 4-8 of the file"
    );
}

#[test]
fn phase6_protocol_handler_locked_session_rejects() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _mvk_bytes, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"secret data";
    let file_id_hex = create_test_file(&db, &mvk, &vdir, plaintext, "test.txt");

    let manager = SessionManager::new(); // locked by default

    let url = format!("vault://file/{}?token=wrongtoken&range=0-10", file_id_hex);

    let request = tauri::http::Request::builder()
        .uri(url)
        .body(Vec::new())
        .unwrap();

    let response = handle_protocol_request(&manager, &db, &vdir, request);

    assert_eq!(response.status(), 403);
    assert_eq!(
        response.body().len(),
        0,
        "locked session should return empty body"
    );
}

#[test]
fn phase6_protocol_handler_unknown_file_id() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _mvk_bytes, vdir) = create_test_vault(&tmp_dir);

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!(
        "vault://file/00000000000000000000000000000000?token={}&range=0-10",
        token
    );

    let request = tauri::http::Request::builder()
        .uri(url)
        .body(Vec::new())
        .unwrap();

    let response = handle_protocol_request(&manager, &db, &vdir, request);

    assert_eq!(response.status(), 404);
}

#[test]
fn phase6_protocol_handler_mid_stream_lock_aborts() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _mvk_bytes, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"multi chunk test data";
    let file_id_hex = create_test_file(&db, &mvk, &vdir, plaintext, "multi_chunk.txt");

    let (manager, token) = unlock_vault(&db, &mvk, &vdir);

    let url = format!(
        "vault://file/{}?token={}&range=0-{}",
        file_id_hex,
        token,
        plaintext.len() - 1
    );

    let request = tauri::http::Request::builder()
        .uri(url)
        .body(Vec::new())
        .unwrap();

    // Lock the session before the request - handler should abort immediately
    manager.lock();

    let response = handle_protocol_request(&manager, &db, &vdir, request);

    assert_eq!(response.status(), 403);
    assert_eq!(
        response.body().len(),
        0,
        "mid-stream lock should return empty body"
    );
}

/// End-to-end: lock invalidates token, unlock rotates it.
/// 1. Create vault + file
/// 2. Unlock -> fetch with token1 -> 200
/// 3. Lock -> fetch with token1 -> 403
/// 4. Unlock again -> token2 != token1
/// 5. Fetch with token2 -> 200
#[test]
fn phase6_e2e_lock_invalidates_token_unlock_rotates() {
    let tmp_dir = setup_vault_dir();
    let (db, mvk, _mvk_bytes, vdir) = create_test_vault(&tmp_dir);

    let plaintext = b"e2e test file content for phase6";
    let file_id_hex = create_test_file(&db, &mvk, &vdir, plaintext, "e2e.txt");

    // --- First unlock ---
    let (manager, token1) = unlock_vault(&db, &mvk, &vdir);

    let url = format!(
        "vault://file/{}?token={}&range=0-{}",
        file_id_hex,
        token1,
        plaintext.len() - 1
    );
    let request = tauri::http::Request::builder()
        .uri(url.clone())
        .body(Vec::new())
        .unwrap();
    let response = handle_protocol_request(&manager, &db, &vdir, request);
    assert_eq!(
        response.status(),
        200,
        "first fetch should succeed with token1"
    );
    assert_eq!(
        response.body(),
        &plaintext[..],
        "body should match plaintext with token1"
    );

    // --- Lock ---
    manager.lock();
    assert!(manager.is_locked(), "vault should be locked after lock()");

    // Fetch with token1 after lock -> 403
    let request2 = tauri::http::Request::builder()
        .uri(url)
        .body(Vec::new())
        .unwrap();
    let response2 = handle_protocol_request(&manager, &db, &vdir, request2);
    assert_eq!(
        response2.status(),
        403,
        "fetch after lock should return 403 with token1"
    );

    // --- Unlock again (token rotates) ---
    let session = Session::new(mvk.clone(), 1, UnlockMethod::Pin);
    manager.set_session(session, vdir.meta_path.to_string_lossy().to_string());
    let token2 = manager.token();
    assert_ne!(token1, token2, "token should rotate on unlock");
    assert!(
        !manager.is_locked(),
        "vault should be unlocked after set_session"
    );

    // Fetch with token2 -> 200
    let url2 = format!(
        "vault://file/{}?token={}&range=0-{}",
        file_id_hex,
        token2,
        plaintext.len() - 1
    );
    let request3 = tauri::http::Request::builder()
        .uri(url2)
        .body(Vec::new())
        .unwrap();
    let response3 = handle_protocol_request(&manager, &db, &vdir, request3);
    assert_eq!(
        response3.status(),
        200,
        "second fetch should succeed with token2"
    );
    assert_eq!(
        response3.body(),
        &plaintext[..],
        "body should match plaintext with token2"
    );
}
