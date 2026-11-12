//! Slice C integration tests: favorites, recent ordering, credential
//! rotation (PIN/password), backup-history clear, vault deletion, and the
//! configurable auto-lock timeout — through the same command layer the IPC
//! handlers call.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use veylora::auth::session::SessionManager;
use veylora::commands::{self, files};

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::current_dir().unwrap().join(format!(
        "target/debug/test-slicec-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_vault(root: &Path, session: &Arc<SessionManager>) -> commands::VaultInfo {
    let vault_path = root.join("vault");
    commands::vault_create(
        "test".to_string(),
        vault_path.to_string_lossy().into_owned(),
        "1234".to_string(),
        "password123".to_string(),
        session.clone(),
    )
    .expect("vault_create should succeed")
}

fn import_file(root: &Path, name: &str, session: &Arc<SessionManager>) -> u64 {
    let src = root.join(name);
    std::fs::write(&src, format!("content of {name}")).unwrap();
    let imported = files::file_import(
        vec![src.to_string_lossy().into_owned()],
        None,
        session.clone(),
    )
    .expect("import");
    imported[0].id
}

#[test]
fn slicec_favorite_toggle_and_list() {
    let session = Arc::new(SessionManager::new());
    let root = tmp_dir("favorite");
    create_vault(&root, &session);

    let id = import_file(&root, "star-me.txt", &session);

    // Star it.
    files::file_favorite(id, true, session.clone()).expect("star");
    // Idempotent: starring twice must not duplicate.
    files::file_favorite(id, true, session.clone()).expect("star again");
    let favs = files::file_favorites(session.clone()).expect("list favorites");
    assert_eq!(favs.len(), 1);
    assert_eq!(favs[0].filename, "star-me.txt");
    assert_eq!(favs[0].id, id);

    // Unstar it.
    files::file_favorite(id, false, session.clone()).expect("unstar");
    let favs = files::file_favorites(session.clone()).expect("list favorites");
    assert!(favs.is_empty(), "unstarred file must disappear");

    // Unknown row id must fail (404, not silent no-op).
    assert!(files::file_favorite(999_999, true, session.clone()).is_err());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn slicec_recent_returns_newest_first() {
    let session = Arc::new(SessionManager::new());
    let root = tmp_dir("recent");
    create_vault(&root, &session);

    let _a = import_file(&root, "oldest.txt", &session);
    let _b = import_file(&root, "middle.txt", &session);
    let id_c = import_file(&root, "newest.txt", &session);

    let recent = files::file_recent(10, session.clone()).expect("recent");
    assert_eq!(recent.len(), 3);
    // Newest import (highest row id) first.
    assert_eq!(recent[0].id, id_c);
    assert_eq!(recent[0].filename, "newest.txt");
    assert_eq!(recent[2].filename, "oldest.txt");

    // Limit is honored.
    let recent = files::file_recent(1, session.clone()).expect("recent limit");
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].filename, "newest.txt");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn slicec_change_pin_verifies_current_then_rotates() {
    let session = Arc::new(SessionManager::new());
    let root = tmp_dir("change-pin");
    create_vault(&root, &session);

    // Wrong current PIN must be rejected before anything is written.
    assert!(
        commands::vault_change_pin("9999".to_string(), "5678".to_string(), session.clone())
            .is_err()
    );
    // Invalid new PIN format rejected.
    assert!(
        commands::vault_change_pin("1234".to_string(), "12".to_string(), session.clone()).is_err()
    );

    commands::vault_change_pin("1234".to_string(), "5678".to_string(), session.clone())
        .expect("PIN change should succeed");

    // Lock + re-open (lock clears current_path), then unlock: old PIN fails,
    // new PIN works, password entry untouched.
    session.lock();
    commands::vault_open(
        root.join("vault").to_string_lossy().into_owned(),
        session.clone(),
    )
    .expect("re-open");
    assert!(commands::vault_unlock_pin("1234".to_string(), session.clone()).is_err());
    session.lock();
    commands::vault_open(
        root.join("vault").to_string_lossy().into_owned(),
        session.clone(),
    )
    .expect("re-open");
    let result = commands::vault_unlock_pin("5678".to_string(), session.clone())
        .expect("new PIN must unlock");
    assert_eq!(result.unlock_method, "pin");

    session.lock();
    commands::vault_open(
        root.join("vault").to_string_lossy().into_owned(),
        session.clone(),
    )
    .expect("re-open");
    commands::vault_unlock_password("password123".to_string(), session.clone())
        .expect("password entry must survive a PIN change");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn slicec_change_password_verifies_current_then_rotates() {
    let session = Arc::new(SessionManager::new());
    let root = tmp_dir("change-pw");
    create_vault(&root, &session);

    // Wrong current password rejected.
    assert!(commands::vault_change_password(
        "wrongpass1".to_string(),
        "newpassword1".to_string(),
        session.clone()
    )
    .is_err());
    // Too-short new password rejected.
    assert!(commands::vault_change_password(
        "password123".to_string(),
        "short".to_string(),
        session.clone()
    )
    .is_err());

    commands::vault_change_password(
        "password123".to_string(),
        "newpassword1".to_string(),
        session.clone(),
    )
    .expect("password change should succeed");

    session.lock();
    commands::vault_open(
        root.join("vault").to_string_lossy().into_owned(),
        session.clone(),
    )
    .expect("re-open");
    assert!(commands::vault_unlock_password("password123".to_string(), session.clone()).is_err());
    session.lock();
    commands::vault_open(
        root.join("vault").to_string_lossy().into_owned(),
        session.clone(),
    )
    .expect("re-open");
    commands::vault_unlock_password("newpassword1".to_string(), session.clone())
        .expect("new password must unlock");

    // PIN entry survives a password change.
    session.lock();
    commands::vault_open(
        root.join("vault").to_string_lossy().into_owned(),
        session.clone(),
    )
    .expect("re-open");
    commands::vault_unlock_pin("1234".to_string(), session.clone())
        .expect("PIN entry must survive a password change");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn slicec_backup_history_clear_on_empty_table() {
    let session = Arc::new(SessionManager::new());
    let root = tmp_dir("clear-history");
    create_vault(&root, &session);

    // Fresh vault has no history; clearing must still succeed.
    commands::vault_backup_history_clear(session.clone()).expect("clear");
    let history = commands::vault_backup_history(session.clone()).expect("history");
    assert!(history.is_empty());

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn slicec_delete_vault_requires_correct_pin() {
    let session = Arc::new(SessionManager::new());
    let root = tmp_dir("delete");
    let vault_path = root.join("vault");
    create_vault(&root, &session);

    // Wrong PIN: error, and NOT a single byte removed.
    assert!(commands::vault_delete("9999".to_string(), session.clone()).is_err());
    assert!(
        vault_path.exists(),
        "failed PIN check must leave the vault on disk"
    );

    // Correct PIN: vault directory gone, session locked.
    commands::vault_delete("1234".to_string(), session.clone()).expect("delete");
    assert!(!vault_path.exists(), "vault directory must be removed");
    assert!(session.is_locked(), "session must be locked after delete");

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn slicec_auto_lock_timeout_is_configurable() {
    use veylora::auth::auto_lock;

    // Default is 300s (5 minutes) per the UI spec.
    assert_eq!(auto_lock::timeout_secs(), 300);
    auto_lock::set_timeout(60);
    assert_eq!(auto_lock::timeout_secs(), 60);
    auto_lock::set_timeout(0); // 0 = never
    assert_eq!(auto_lock::timeout_secs(), 0);
    auto_lock::set_timeout(300); // restore default for other tests
}
