//! Slice B integration tests: recovery key at vault creation, and the
//! file/folder command roundtrip (import → list → export → folder → rename
//! → delete) through the same command layer the IPC handlers call.

use std::path::PathBuf;
use std::sync::Arc;

use veylora::auth::session::SessionManager;
use veylora::commands::{self, files};

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::current_dir().unwrap().join(format!(
        "target/debug/test-sliceb-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_vault(root: &PathBuf, session: &Arc<SessionManager>) -> commands::VaultInfo {
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

#[test]
fn sliceb_create_returns_recovery_key_that_unlocks() {
    let session = Arc::new(SessionManager::new());
    let root = tmp_dir("recovery");

    let info = create_vault(&root, &session);
    assert!(
        !info.recovery_key.is_empty(),
        "vault_create must return the one-time recovery key"
    );
    assert_eq!(
        info.recovery_key.len(),
        56,
        "52 base32 chars + 4 hex checksum"
    );

    // Lock, re-open (lock clears current_path), unlock with ONLY the key.
    session.lock();
    commands::vault_open(
        root.join("vault").to_string_lossy().into_owned(),
        session.clone(),
    )
    .expect("re-open after lock");
    let result = commands::vault_unlock_recovery(info.recovery_key.clone(), session.clone())
        .expect("recovery unlock must succeed");
    assert_eq!(result.unlock_method, "recovery");

    // A corrupted key (bad checksum) must be rejected.
    session.lock();
    commands::vault_open(
        root.join("vault").to_string_lossy().into_owned(),
        session.clone(),
    )
    .expect("re-open again");
    let mut bad = info.recovery_key.clone();
    bad.replace_range(52..56, "0000");
    assert!(
        commands::vault_unlock_recovery(bad, session.clone()).is_err(),
        "corrupted recovery key must not unlock"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn sliceb_import_list_export_folder_rename_delete_roundtrip() {
    let session = Arc::new(SessionManager::new());
    let root = tmp_dir("roundtrip");
    create_vault(&root, &session);

    // Source file spanning multiple 64 KiB chunks.
    let src = root.join("p1.pdf");
    let original: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&src, &original).unwrap();
    let src_str = src.to_string_lossy().into_owned();

    // Import at root.
    let imported = files::file_import(vec![src_str.clone()], None, session.clone())
        .expect("import should succeed");
    assert_eq!(imported.len(), 1);
    assert_eq!(imported[0].filename, "p1.pdf");
    let row_id = imported[0].id;
    assert_eq!(imported[0].folder_id, None);

    let listing = files::file_list(None, session.clone()).expect("list root");
    assert_eq!(listing.files.len(), 1);
    assert!(
        listing.files[0].encrypted_size > original.len() as u64,
        "encrypted blob is larger than plaintext (crypto overhead)"
    );

    // Export roundtrip: decrypted bytes must equal the source.
    let dest = root.join("out.pdf");
    let written = files::file_export(
        row_id,
        dest.to_string_lossy().into_owned(),
        session.clone(),
    )
    .expect("export should succeed");
    assert_eq!(written as usize, original.len());
    assert_eq!(std::fs::read(&dest).unwrap(), original);

    // Preview roundtrip: the vault:// protocol path returns the exact source
    // bytes (this is what the image/PDF/video viewers load).
    let token = session.token();
    let fid = &imported[0].file_id;
    let full = files::file_preview(format!("vault://file/{fid}?token={token}"), session.clone())
        .expect("preview should succeed");
    assert_eq!(full, original, "preview bytes must equal the source file");
    // Inclusive range [4,8] = 5 bytes, identical slice of the source.
    let ranged = files::file_preview(
        format!("vault://file/{fid}?token={token}&range=4-8"),
        session.clone(),
    )
    .expect("ranged preview");
    assert_eq!(ranged, original[4..=8].to_vec());
    // Wrong token is rejected (mapped to 403 by the protocol layer).
    assert!(files::file_preview(format!("vault://file/{fid}?token=deadbeef"), session.clone())
        .is_err());

    // Folder creation + import into it.
    let folder = files::folder_create("Docs".to_string(), None, session.clone())
        .expect("folder_create");
    assert_eq!(folder.parent_id, None);
    let second = files::file_import(vec![src_str], Some(folder.id), session.clone())
        .expect("import into folder");
    assert_eq!(second[0].folder_id, Some(folder.id));

    let root_listing = files::file_list(None, session.clone()).unwrap();
    assert_eq!(root_listing.files.len(), 1, "root holds only the first file");
    assert_eq!(root_listing.folders.len(), 1);
    let folder_listing = files::file_list(Some(folder.id), session.clone()).unwrap();
    assert_eq!(folder_listing.files.len(), 1);

    // Rename re-encrypts the filename.
    files::file_rename(row_id, "renamed.pdf".to_string(), session.clone())
        .expect("rename");
    let listing = files::file_list(None, session.clone()).unwrap();
    assert_eq!(listing.files[0].filename, "renamed.pdf");
    assert!(
        files::file_rename(row_id, "   ".to_string(), session.clone()).is_err(),
        "empty filename rejected"
    );

    // Delete removes the row (and blob).
    files::file_delete(row_id, session.clone()).expect("delete");
    let listing = files::file_list(None, session.clone()).unwrap();
    assert!(listing.files.is_empty(), "root is empty after delete");

    // Locked vault rejects every file command.
    session.lock();
    let err = files::file_list(None, session.clone()).unwrap_err();
    assert!(err.contains("locked"), "unexpected error: {err}");

    let _ = std::fs::remove_dir_all(&root);
}
