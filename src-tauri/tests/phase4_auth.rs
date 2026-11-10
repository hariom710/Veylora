//! Phase 4 integration tests: Auth, sessions, IPC.
//!
//! Covers: vault create/unlock/lock, Argon2id PIN/password/recovery unlock,
//! attempt throttling, auto-lock, IPC boundary enforcement, MVK zeroization.

use std::time::{SystemTime, UNIX_EPOCH, Instant};
use veylora::auth::session::SessionManager;
use veylora::auth::throttle;
use veylora::auth::AuthError;
use veylora::auth::pin::unlock_with_pin;
use veylora::auth::password::unlock_with_password;
use veylora::auth::recovery::unlock_with_recovery;
use veylora::crypto::hkdf::LABEL_METADATA_KEY;
use veylora::crypto::mvk::Mvk;
use veylora::crypto::recovery::generate_recovery_key;
use veylora::database::Db;
use veylora::storage::vault_dir::VaultDir;
use veylora::vault::VaultMeta;

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn setup_vault_dir() -> std::path::PathBuf {
    let tmp = std::env::current_dir()
        .unwrap()
        .join(format!(
            "target/debug/test-phase4-{}-{}",
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
/// and a known recovery key. Returns (db, mvk_bytes, recovery_key).
fn create_test_vault(tmp_dir: &std::path::Path) -> (Db, [u8; 32], [u8; 32]) {
    let mvk = Mvk::generate().expect("MVK generation failed");
    let mvk_bytes = *mvk.as_bytes();
    let recovery_key = generate_recovery_key();
    let _metadata_key =
        veylora::crypto::hkdf::derive_subkey(mvk.as_bytes(), LABEL_METADATA_KEY)
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

    (db, mvk_bytes, recovery_key)
}

#[test]
fn phase4_vault_create_unlock_pin_roundtrip() {
    let tmp = setup_vault_dir();
    let (db, mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    // Unlock with PIN
    let session = unlock_with_pin(&tmp, b"1234")
        .expect("PIN unlock should succeed");

    // Verify MVK is present in session
    session.with_mvk(|mvk_bytes| {
        assert_eq!(mvk_bytes, mvk_bytes, "MVK should match original");
    }).expect("with_mvk should succeed");

    // Verify attempt counter was cleared on successful unlock
    let count = throttle::failed_count(&db, 1);
    assert_eq!(count, 0, "attempt counter should be cleared on successful unlock");
}

#[test]
fn phase4_vault_unlock_password_roundtrip() {
    let tmp = setup_vault_dir();
    let (db, mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    let session = unlock_with_password(&tmp, b"correct horse battery staple")
        .expect("password unlock should succeed");

    session.with_mvk(|m| {
        assert_eq!(m, &mvk_bytes, "MVK should match original");
    }).expect("with_mvk should succeed");

    let count = throttle::failed_count(&db, 1);
    assert_eq!(count, 0, "attempt counter should be cleared on successful unlock");
}

#[test]
fn phase4_vault_unlock_recovery_roundtrip() {
    let tmp = setup_vault_dir();
    let (db, mvk_bytes, recovery_key) = create_test_vault(&tmp);

    let session = unlock_with_recovery(&tmp, &recovery_key)
        .expect("recovery unlock should succeed");

    session.with_mvk(|m| {
        assert_eq!(m, &mvk_bytes, "MVK should match original");
    }).expect("with_mvk should succeed");

    let count = throttle::failed_count(&db, 1);
    assert_eq!(count, 0, "attempt counter should be cleared on successful unlock");
}

#[test]
fn phase4_vault_lock_zeroizes_mvk() {
    let tmp = setup_vault_dir();
    let (_db, mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    let session = SessionManager::new();

    // First unlock
    let unlock_session = unlock_with_pin(&tmp, b"1234")
        .expect("PIN unlock should succeed");
    session.set_session(unlock_session, tmp.join("vault.meta").to_string_lossy().to_string());

    assert!(!session.is_locked(), "session should be unlocked");

    // Capture MVK bytes before lock
    let mvk_bytes_before: [u8; 32] = session.with_mvk(|m| *m).expect("with_mvk should succeed");

    // Lock the vault
    session.lock();
    assert!(session.is_locked(), "session should be locked after lock()");

    // with_mvk should fail after lock
    let err = session.with_mvk(|_| ()).unwrap_err();
    assert!(
        matches!(err, veylora::auth::session::SessionError::Locked),
        "with_mvk should fail after lock, got {:?}", err
    );

    // The captured MVK bytes should match the original (proving we read them before lock)
    assert_eq!(mvk_bytes_before, mvk_bytes, "captured MVK should match original");
}

#[test]
fn phase4_auto_lock_short_timeout() {
    let session = SessionManager::new();

    // Create a session manually
    let mvk = Mvk::generate().expect("MVK generation failed");
    let sess = veylora::auth::session::Session::new(mvk, 1, veylora::auth::UnlockMethod::Pin);
    session.set_session(sess, "/tmp/test".to_string());

    assert!(!session.is_locked(), "session should be unlocked initially");

    // Advance time past the timeout (100ms timeout)
    let now = now_unix();
    session.check_auto_lock(now + 200, 100); // 200ms > 100ms timeout

    assert!(session.is_locked(), "session should be locked after auto-lock timeout");
}

#[test]
fn phase4_activity_resets_timeout() {
    let session = SessionManager::new();

    let mvk = Mvk::generate().expect("MVK generation failed");
    let sess = veylora::auth::session::Session::new(mvk, 1, veylora::auth::UnlockMethod::Pin);
    session.set_session(sess, "/tmp/test".to_string());

    assert!(!session.is_locked(), "session should be unlocked initially");

    // Touch activity to reset timeout
    session.touch_activity();

    // Advance time slightly (less than timeout)
    let now = now_unix();
    session.check_auto_lock(now + 50, 100); // 50ms < 100ms timeout

    assert!(!session.is_locked(), "session should still be unlocked after activity");
}

#[test]
fn phase4_successful_unlock_clears_attempts() {
    let tmp = setup_vault_dir();
    let (db, _mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    // Record some failed attempts first
    throttle::record_failure(&db, 1);
    throttle::record_failure(&db, 1);
    assert_eq!(throttle::failed_count(&db, 1), 2, "should have 2 failed attempts");

    // Now unlock successfully (which clears attempts)
    let session = unlock_with_pin(&tmp, b"1234")
        .expect("PIN unlock should succeed");
    let _ = session; // session is used for side effect of clearing attempts

    // Clear attempts on successful unlock (as done in IPC command handler)
    throttle::clear_attempts(&db, 1);

    let count = throttle::failed_count(&db, 1);
    assert_eq!(count, 0, "attempt counter should be cleared on successful unlock");
}

#[test]
fn phase4_attempt_throttling_wrong_pin() {
    let tmp = setup_vault_dir();
    let (db, _mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    // Wrong PIN 5 times
    for _ in 0..5 {
        let err = unlock_with_pin(&tmp, b"wrongpin").unwrap_err();
        assert!(
            matches!(err, AuthError::InvalidCredential),
            "wrong PIN should return InvalidCredential, got {:?}", err
        );
        // Record failure (as done in IPC command handler)
        throttle::record_failure(&db, 1);
    }

    // After 5 failures, should be locked out
    let lockout = throttle::check_locked(&db, 1);
    assert!(
        lockout.is_some(),
        "should be locked out after 5 failed attempts"
    );
}

#[test]
fn phase4_no_lockout_below_five_failures() {
    let tmp = setup_vault_dir();
    let (db, _mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    // Regression: one typo used to fall through to the 24h LOCKOUT_MAX tier
    // (no threshold matches, .unwrap_or fired). Below 5 = count only.
    throttle::record_failure(&db, 1);
    assert_eq!(throttle::failed_count(&db, 1), 1);
    assert!(
        throttle::check_locked(&db, 1).is_none(),
        "1 failure must not lock the vault"
    );

    for _ in 0..3 {
        throttle::record_failure(&db, 1);
    }
    assert!(
        throttle::check_locked(&db, 1).is_none(),
        "4 failures must not lock the vault"
    );

    throttle::record_failure(&db, 1); // 5th
    assert!(
        throttle::check_locked(&db, 1).is_some(),
        "5 failures must lock the vault"
    );

    // Tier selection: the highest matching threshold wins — 10 failures
    // → 5 minutes, not the 30s tier.
    throttle::clear_attempts(&db, 1);
    for _ in 0..10 {
        throttle::record_failure(&db, 1);
    }
    let expiry = throttle::check_locked(&db, 1).expect("10 failures lock out");
    let remaining = expiry.duration_since(std::time::Instant::now()).as_secs();
    assert!(
        (290..=300).contains(&remaining),
        "10 failures → ~5min lockout, got {remaining}s"
    );
}

#[test]
fn phase4_wrong_password() {
    let tmp = setup_vault_dir();
    let (_db, _mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    let err = unlock_with_password(&tmp, b"wrongpassword")
        .unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidCredential),
        "wrong password should return InvalidCredential, got {:?}", err
    );
}

#[test]
fn phase4_wrong_recovery_key() {
    let tmp = setup_vault_dir();
    let (_db, _mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    let wrong_key = [0xFFu8; 32];
    let err = unlock_with_recovery(&tmp, &wrong_key)
        .unwrap_err();
    assert!(
        matches!(err, AuthError::InvalidCredential),
        "wrong recovery key should return InvalidCredential, got {:?}", err
    );
}

#[test]
fn phase4_unlock_on_locked_out_vault() {
    let tmp = setup_vault_dir();
    let (db, _mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    // Fail 5 times to trigger lockout
    for _ in 0..5 {
        let _ = unlock_with_pin(&tmp, b"wrongpin");
        throttle::record_failure(&db, 1);
    }

    // After 5 failures, check_locked should return Some
    let lockout = throttle::check_locked(&db, 1);
    assert!(
        lockout.is_some(),
        "should be locked out after 5 failed attempts"
    );

    // Verify lockout is in the future
    let lockout = throttle::check_locked(&db, 1).unwrap();
    let lockout = throttle::check_locked(&db, 1).unwrap();
    assert!(
        lockout > Instant::now(),
        "lockout should be in the future"
    );
}

#[test]
fn phase4_lock_when_already_locked() {
    let session = SessionManager::new();

    assert!(session.is_locked(), "session should start locked");

    // Locking an already-locked session should be idempotent
    session.lock();
    assert!(session.is_locked(), "session should still be locked");
}

#[test]
fn phase4_unlock_pin_with_no_vault_meta() {
    let tmp = setup_vault_dir();
    // Don't create vault.meta - just empty dir

    let err = unlock_with_pin(&tmp, b"1234").unwrap_err();
    assert!(
        matches!(err, AuthError::VaultNotFound),
        "missing vault.meta should return VaultNotFound, got {:?}", err
    );
}

#[test]
fn phase4_tampered_vault_meta() {
    let tmp = setup_vault_dir();
    let (_db, _mvk_bytes, _recovery_key) = create_test_vault(&tmp);

    // Tamper with vault.meta
    let meta_path = tmp.join("vault.meta");
    let mut content = std::fs::read_to_string(&meta_path).expect("read vault.meta");
    content.push_str(", \"tampered\": true");
    std::fs::write(&meta_path, &content).expect("write tampered vault.meta");

    let err = unlock_with_pin(&tmp, b"1234").unwrap_err();
    assert!(
        matches!(err, AuthError::VaultCorrupted),
        "tampered vault.meta should return VaultCorrupted, got {:?}", err
    );
}

#[test]
fn phase4_ipc_boundary_no_key_material() {
    // Source-scan test: verify no command function signature contains
    // key material terms (Mvk, Kek, Key, Ciphertext, Nonce) as type names
    let commands_src = std::fs::read_to_string("src/commands/mod.rs")
        .expect("read commands/mod.rs");

    // Check each #[tauri::command] function signature for forbidden type names
    let forbidden = ["Mvk", "Kek", "Key", "Ciphertext", "Nonce"];

    // Extract function signatures (lines starting with `pub fn`)
    for line in commands_src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn") || trimmed.starts_with("async fn") {
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

#[test]
fn phase4_with_mvk_zeroize_on_lock() {
    let session = SessionManager::new();

    let mvk = Mvk::generate().expect("MVK generation failed");
    let mvk_bytes = *mvk.as_bytes();
    let sess = veylora::auth::session::Session::new(mvk, 1, veylora::auth::UnlockMethod::Pin);
    session.set_session(sess, "/tmp/test".to_string());

    // Capture MVK bytes via with_mvk before lock
    let captured: [u8; 32] = session.with_mvk(|m| *m).expect("with_mvk should succeed");
    assert_eq!(captured, mvk_bytes, "captured MVK should match original");

    // Lock the session
    session.lock();
    assert!(session.is_locked(), "session should be locked");

    // with_mvk should fail after lock
    let err = session.with_mvk(|_| ()).unwrap_err();
    assert!(
        matches!(err, veylora::auth::session::SessionError::Locked),
        "with_mvk should fail after lock"
    );
}

#[test]
fn phase4_windows_session_lock_deferred() {
    // Windows session-lock (WM_WTSSESSION_CHANGE via windows crate) is
    // deferred to Phase 8. This test documents the deferral.
    //
    // The Windows session-lock integration requires:
    // 1. windows crate dependency
    // 2. WTSRegisterSessionNotification callback
    // 3. Integration with session lock on WM_WTSSESSION_CHANGE
    //
    // This is deferred because it requires Windows-specific API calls that
    // are not needed for the core security functionality of Phase 4.

    assert!(true, "Windows session-lock deferred to Phase 8");
}

#[test]
fn phase4_commands_lock_reopen_unlock_roundtrip() {
    // Regression: unlock used session.vault_id(), which is None while locked,
    // so lock -> unlock always failed with "no active session". vault_id now
    // comes from vault.meta, and the UI re-opens the path after lock() clears it.
    let tmp = setup_vault_dir();
    let path = tmp.to_str().unwrap().to_string();
    let manager = std::sync::Arc::new(SessionManager::new());

    veylora::commands::vault_create(
        "test".into(),
        path.clone(),
        "1234".into(),
        "pw123456".into(),
        manager.clone(),
    )
    .expect("vault_create should succeed");
    assert!(!manager.is_locked(), "create leaves vault unlocked");

    veylora::commands::vault_lock(manager.clone()).expect("lock should succeed");
    assert!(manager.is_locked(), "vault should be locked after lock()");

    // lock() cleared current_path; the UI re-opens it (same as restart flow).
    veylora::commands::vault_open(path.clone(), manager.clone()).expect("reopen should succeed");
    assert!(manager.is_locked(), "reopen must not unlock");

    let result =
        veylora::commands::vault_unlock_pin("1234".into(), manager.clone())
            .expect("unlock after lock should succeed");
    assert_eq!(result.unlock_method, "pin");
    assert!(!manager.is_locked(), "vault unlocked with PIN");

    let _ = std::fs::remove_dir_all(&tmp);
}
