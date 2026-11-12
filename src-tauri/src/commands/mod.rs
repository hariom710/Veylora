//! Tauri IPC command handlers (thin layer, no crypto).
//!
//! No command argument or return type may contain key material (MVK, KEK,
//! nonce, ciphertext, etc.). Plaintext file metadata (filenames, etc.) is
//! allowed because the UI needs to display it â€” the rule is about key
//! material, not user data.

#![allow(missing_docs)]

use crate::auth::session::SessionManager;
use crate::auth::UnlockMethod;
use crate::database::Db;
use crate::storage::vault_dir::VaultDir;
use crate::vault::VaultMeta;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub mod files; // Slice B: file/folder IPC commands (import, list, export, ...)

/// Information returned on vault create/open.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VaultInfo {
    pub name: String,
    pub path: String,
    pub vault_id: u64,
    /// Encoded recovery key, returned exactly once at creation.
    /// Empty for vault_open (the key cannot be recovered from nothing).
    pub recovery_key: String,
}

/// Result of an unlock operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UnlockResult {
    pub vault_id: u64,
    pub unlock_method: String,
}

/// Vault status returned by vault_status.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatusInfo {
    pub locked: bool,
    pub vault_id: Option<u64>,
    pub file_count: u64,
    pub storage_used: u64,
    /// Whether a vault path is open in this process (set by vault_create/vault_open).
    /// False on fresh start: the UI must offer create/open before unlock.
    pub vault_open: bool,
}

/// Public vault metadata (no salts, no wrapped keys).
#[derive(Debug, Clone, serde::Serialize)]
pub struct VaultPublicMeta {
    pub name: String,
    pub path: String,
    pub version: u32,
    pub kdf_params_public: String,
}

/// Create a new vault at `path` with PIN and password.
#[allow(dead_code)]
pub fn vault_create(
    _name: String,
    path: String,
    pin: String,
    password: String,
    session: Arc<SessionManager>,
) -> Result<VaultInfo, String> {
    let vdir = VaultDir::new(std::path::Path::new(&path).to_path_buf());
    vdir.create_layout().map_err(|e| e.to_string())?;

    let mvk = crate::crypto::mvk::Mvk::generate().map_err(|e| e.to_string())?;

    let pin_bytes = pin.as_bytes();
    let pw_bytes = password.as_bytes();

    let mut meta = VaultMeta::new();
    meta.add_pin_entry(pin_bytes, &mvk)
        .map_err(|e| e.to_string())?;
    meta.add_password_entry(pw_bytes, &mvk)
        .map_err(|e| e.to_string())?;
    // Third unlock path: 256-bit recovery key, HKDF-wrapped (frozen label).
    // Generated before serialize so the entry lands in vault.meta at creation.
    let recovery_key = crate::crypto::recovery::generate_recovery_key();
    meta.add_recovery_entry(&recovery_key, &mvk)
        .map_err(|e| e.to_string())?;
    let recovery_key_encoded = crate::crypto::recovery::encode_recovery_key(&recovery_key);

    // Write vault.meta atomically
    let meta_json = meta.serialize().map_err(|e| e.to_string())?;
    let meta_path = vdir.meta_path.clone();
    let tmp_path = vdir.meta_path.with_extension("meta.tmp");
    std::fs::write(&tmp_path, &meta_json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp_path, &meta_path).map_err(|e| e.to_string())?;
    crate::storage::vault_dir::fsync_dir(&vdir.blobs_dir).map_err(|e| e.to_string())?;

    // Create vault.db and apply migrations
    let db = crate::database::Db::open(&vdir.db_path).map_err(|e| e.to_string())?;

    // Insert vault record
    db.conn()
        .execute(
            "INSERT INTO vaults (id, created_at) VALUES (?1, ?2)",
            rusqlite::params![
                1,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64
            ],
        )
        .map_err(|e| e.to_string())?;

    let vault_id = vault_id_from_meta(&meta);
    session.set_session(
        crate::auth::session::Session::new(mvk, vault_id, UnlockMethod::Pin),
        path.clone(),
    );

    Ok(VaultInfo {
        name: path.clone(),
        path: path.clone(),
        vault_id,
        recovery_key: recovery_key_encoded,
    })
}

/// Open an existing vault (read metadata only, set current path).
#[allow(dead_code)]
pub fn vault_open(path: String, session: Arc<SessionManager>) -> Result<VaultInfo, String> {
    let vdir = VaultDir::new(std::path::Path::new(&path).to_path_buf());
    let meta_path = &vdir.meta_path;

    let meta_str = std::fs::read_to_string(meta_path).map_err(|_| "vault not found".to_string())?;
    let meta: VaultMeta =
        serde_json::from_str(&meta_str).map_err(|_| "vault corrupted".to_string())?;

    let vault_id = vault_id_from_meta(&meta);

    // Store the current path (no session yet â€” locked state)
    session.set_current_path(path.clone());

    Ok(VaultInfo {
        name: path.clone(),
        path: path.clone(),
        vault_id,
        recovery_key: String::new(),
    })
}

/// Unlock with PIN.
#[allow(dead_code)]
pub fn vault_unlock_pin(pin: String, session: Arc<SessionManager>) -> Result<UnlockResult, String> {
    let path = session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())?;

    let db = crate::database::Db::open(&std::path::Path::new(&path).join("vault.db"))
        .map_err(|e| e.to_string())?;

    let vault_id = vault_id_at(&path)?;

    // Check throttling
    if let Some(lockout) = crate::auth::throttle::check_locked(&db, vault_id) {
        let remaining = lockout.duration_since(std::time::Instant::now()).as_secs();
        return Err(format!("locked out: try again in {}s", remaining));
    }

    // Authenticate
    match crate::auth::pin::unlock_with_pin(std::path::Path::new(&path), pin.as_bytes()) {
        Ok(new_session) => {
            crate::auth::throttle::clear_attempts(&db, vault_id);
            session.set_session(new_session, path);
            Ok(UnlockResult {
                vault_id,
                unlock_method: "pin".to_string(),
            })
        }
        Err(_) => {
            crate::auth::throttle::record_failure(&db, vault_id);
            Err("invalid credential".to_string())
        }
    }
}

/// Unlock with password.
#[allow(dead_code)]
pub fn vault_unlock_password(
    password: String,
    session: Arc<SessionManager>,
) -> Result<UnlockResult, String> {
    let path = session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())?;
    let db = crate::database::Db::open(&std::path::Path::new(&path).join("vault.db"))
        .map_err(|e| e.to_string())?;

    let vault_id = vault_id_at(&path)?;

    if let Some(lockout) = crate::auth::throttle::check_locked(&db, vault_id) {
        let remaining = lockout.duration_since(std::time::Instant::now()).as_secs();
        return Err(format!("locked out: try again in {}s", remaining));
    }

    match crate::auth::password::unlock_with_password(
        std::path::Path::new(&path),
        password.as_bytes(),
    ) {
        Ok(new_session) => {
            crate::auth::throttle::clear_attempts(&db, vault_id);
            session.set_session(new_session, path);
            Ok(UnlockResult {
                vault_id,
                unlock_method: "password".to_string(),
            })
        }
        Err(_) => {
            crate::auth::throttle::record_failure(&db, vault_id);
            Err("invalid credential".to_string())
        }
    }
}

/// Unlock with recovery key.
#[allow(dead_code)]
pub fn vault_unlock_recovery(
    recovery_key: String,
    session: Arc<SessionManager>,
) -> Result<UnlockResult, String> {
    let path = session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())?;
    let db = crate::database::Db::open(&std::path::Path::new(&path).join("vault.db"))
        .map_err(|e| e.to_string())?;

    let vault_id = vault_id_at(&path)?;

    if let Some(lockout) = crate::auth::throttle::check_locked(&db, vault_id) {
        let remaining = lockout.duration_since(std::time::Instant::now()).as_secs();
        return Err(format!("locked out: try again in {}s", remaining));
    }

    let recovery_bytes =
        crate::crypto::recovery::decode_recovery_key(&recovery_key).map_err(|e| e.to_string())?;

    match crate::auth::recovery::unlock_with_recovery(std::path::Path::new(&path), &recovery_bytes)
    {
        Ok(new_session) => {
            crate::auth::throttle::clear_attempts(&db, vault_id);
            session.set_session(new_session, path);
            Ok(UnlockResult {
                vault_id,
                unlock_method: "recovery".to_string(),
            })
        }
        Err(_) => {
            crate::auth::throttle::record_failure(&db, vault_id);
            Err("invalid credential".to_string())
        }
    }
}

/// Lock the vault: zeroize MVK, drop session.
#[allow(dead_code)]
pub fn vault_lock(session: Arc<SessionManager>) -> Result<(), String> {
    session.lock();
    Ok(())
}

/// Vault status: locked/unlocked, vault_id, file_count, storage_used.
#[allow(dead_code)]
pub fn vault_status(session: Arc<SessionManager>) -> Result<StatusInfo, String> {
    let locked = session.is_locked();
    let vault_id = session.vault_id();

    // Real counts from the open vault (plaintext columns, no key needed).
    // No open vault (fresh start / just locked) or a query failure → zeros,
    // never an error: the status poll must always answer.
    let (file_count, storage_used) = match open_db_for_session(&session) {
        Ok(db) => crate::database::files::file_stats(&db).unwrap_or((0, 0)),
        Err(_) => (0, 0),
    };

    Ok(StatusInfo {
        locked,
        vault_id,
        file_count,
        storage_used,
        vault_open: session.current_path().is_some(),
    })
}

/// Public vault metadata (no keys).
#[allow(dead_code)]
pub fn vault_get_meta(path: String) -> Result<VaultPublicMeta, String> {
    let vdir = VaultDir::new(std::path::Path::new(&path).to_path_buf());
    let meta_path = &vdir.meta_path;

    let meta_str = std::fs::read_to_string(meta_path).map_err(|_| "vault not found".to_string())?;
    let meta: VaultMeta =
        serde_json::from_str(&meta_str).map_err(|_| "vault corrupted".to_string())?;

    let kdf_params = serde_json::to_string(&meta.pin.as_ref().map(|e| &e.params))
        .unwrap_or_else(|_| "null".to_string());

    Ok(VaultPublicMeta {
        name: path.clone(),
        path,
        version: meta.version,
        kdf_params_public: kdf_params,
    })
}

/// Error type for Tauri commands.
/// Used by vault_get_session_token and other commands to return typed errors.
#[derive(Debug, serde::Serialize)]
pub enum CommandError {
    /// The vault is currently locked; no session is active.
    Locked,
    /// The vault at the given path could not be found.
    VaultNotFound,
    /// An internal error; the message describes the cause.
    Internal(String),
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::Locked => write!(f, "vault is locked"),
            CommandError::VaultNotFound => write!(f, "vault not found at path"),
            CommandError::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Return the current session token as a hex string.
/// This is the single new command allowed in Phase 5.
/// The token is NOT the MVK, KEK, or any derived key â€” it is a random
/// 32-byte value that rotates on unlock and invalidates on lock.
/// The frontend receives it as a String (hex) and passes it as a query
/// param to vault:// requests. It does not contain key material, so the
/// IPC boundary source-scan test still passes (no forbidden type names).
///
/// Takes a shared SessionManager reference (the same one used by the
/// streaming handler and invalidated by lock()). In the real Tauri app,
/// this would be `tauri::State<'_, Arc<SessionManager>>`.
pub fn vault_get_session_token(
    manager: &crate::auth::session::SessionManager,
) -> Result<String, CommandError> {
    if manager.is_locked() {
        return Err(CommandError::Locked);
    }
    Ok(manager.token())
}

/// Derive vault_id from VaultMeta.
/// vault_id from vault.meta. Unlock uses this because while locked there is
/// no session to ask (lock() drops it), only the open path remains.
pub(crate) fn vault_id_at(path: &str) -> Result<u64, String> {
    let vdir = VaultDir::new(std::path::Path::new(path).to_path_buf());
    let meta_str =
        std::fs::read_to_string(&vdir.meta_path).map_err(|_| "vault not found".to_string())?;
    let meta: VaultMeta =
        serde_json::from_str(&meta_str).map_err(|_| "vault corrupted".to_string())?;
    Ok(vault_id_from_meta(&meta))
}

fn vault_id_from_meta(meta: &VaultMeta) -> u64 {
    let mut hasher = DefaultHasher::new();
    meta.hash(&mut hasher);
    hasher.finish()
}

// ============================================================================
// Phase 7: Backup / Restore commands
// ============================================================================

/// Information returned on backup create/verify.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupInfo {
    pub path: String,
    pub file_count: u64,
    pub total_size: u64,
    pub created_at_unix: i64,
    /// Which regions were cryptographically verified.
    pub verified_fields: Vec<String>,
}

/// Information returned on backup restore.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RestoreInfo {
    pub files_restored: u64,
    pub bytes_written: u64,
}

/// History entry from the backup_history table.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackupHistoryEntry {
    pub path: String,
    pub status: String,
    pub created_at_unix: i64,
}

/// Open the DB for the vault at `session.current_path()`.
pub(crate) fn open_db_for_session(session: &SessionManager) -> Result<Db, String> {
    let path = session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())?;
    crate::database::Db::open(&std::path::Path::new(&path).join("vault.db"))
        .map_err(|e| e.to_string())
}

/// Get the MVK from the active session.
pub(crate) fn with_mvk<F, R>(session: &SessionManager, f: F) -> Result<R, String>
where
    F: FnOnce(&[u8; 32]) -> R,
{
    session
        .with_mvk(f)
        .map_err(|_| "vault is locked".to_string())
}

/// Create a backup of the current vault at `dest_path`.
pub fn vault_backup_create(
    dest_path: String,
    session: Arc<SessionManager>,
) -> Result<BackupInfo, String> {
    if session.is_locked() {
        return Err("vault is locked".to_string());
    }

    let db = open_db_for_session(&session)?;
    let vdir_path = session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())?;
    let vdir =
        crate::storage::vault_dir::VaultDir::new(std::path::Path::new(&vdir_path).to_path_buf());

    let inner_result = with_mvk(&session, |mvk| {
        crate::backup::create::create_backup(mvk, &db, &vdir, &dest_path)
    })
    .map_err(|e| e.to_string())?;

    let bi = inner_result.map_err(|e| e.to_string())?;

    // Record in backup history.
    let _ = crate::backup::verify::record_backup_history(
        &db,
        &dest_path,
        "complete",
        bi.created_at_unix,
    );

    Ok(BackupInfo {
        path: bi.path,
        file_count: bi.file_count,
        total_size: bi.total_size,
        created_at_unix: bi.created_at_unix,
        verified_fields: bi.verified_fields,
    })
}

/// Verify a backup container's integrity.
/// If the session is unlocked, performs full verification (manifest + blobs).
/// If locked, performs partial verification (trailer structure only).
pub fn vault_backup_verify(
    backup_path: String,
    session: Arc<SessionManager>,
) -> Result<BackupInfo, String> {
    let mvk_opt = with_mvk(&session, |mvk| *mvk)
        .map_err(|e| e.to_string())
        .ok();

    match mvk_opt {
        Some(mvk) => {
            let bi = crate::backup::verify::verify_backup(Some(&mvk), &backup_path)
                .map_err(|e| e.to_string())?;
            Ok(BackupInfo {
                path: bi.path,
                file_count: bi.file_count,
                total_size: bi.total_size,
                created_at_unix: bi.created_at_unix,
                verified_fields: bi.verified_fields,
            })
        }
        None => {
            // Session locked â€” partial verification only.
            let bi = crate::backup::verify::verify_backup(None, &backup_path)
                .map_err(|e| e.to_string())?;
            Ok(BackupInfo {
                path: bi.path,
                file_count: bi.file_count,
                total_size: bi.total_size,
                created_at_unix: bi.created_at_unix,
                verified_fields: bi.verified_fields,
            })
        }
    }
}

/// Restore a backup into the current vault.
pub fn vault_backup_restore(
    backup_path: String,
    session: Arc<SessionManager>,
) -> Result<RestoreInfo, String> {
    if session.is_locked() {
        return Err("vault is locked".to_string());
    }

    let mut db = open_db_for_session(&session)?;
    let vdir_path = session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())?;
    let vdir =
        crate::storage::vault_dir::VaultDir::new(std::path::Path::new(&vdir_path).to_path_buf());

    let inner_result = with_mvk(&session, |mvk| {
        crate::backup::restore::restore_backup(mvk, &mut db, &vdir, &backup_path)
    })
    .map_err(|e| e.to_string())?;

    let ri = inner_result.map_err(|e| e.to_string())?;

    Ok(RestoreInfo {
        files_restored: ri.files_restored,
        bytes_written: ri.bytes_written,
    })
}

/// List backup history from the database.
pub fn vault_backup_history(
    session: Arc<SessionManager>,
) -> Result<Vec<BackupHistoryEntry>, String> {
    let db = open_db_for_session(&session)?;
    let entries = crate::backup::verify::list_backup_history(&db).map_err(|e| e.to_string())?;
    Ok(entries
        .into_iter()
        .map(|e| BackupHistoryEntry {
            path: e.path,
            status: e.status,
            created_at_unix: e.created_at_unix,
        })
        .collect())
}

// ---- Slice C: activity / auto-lock / credential rotation / danger zone ----

/// Record user activity (called by the frontend heartbeat and by vault
/// commands) so the auto-lock watcher treats the session as active.
pub fn vault_activity(session: Arc<SessionManager>) -> Result<(), String> {
    if session.is_locked() {
        return Err("vault is locked".to_string());
    }
    session.touch_activity();
    Ok(())
}

/// Set the auto-lock idle timeout in seconds (0 = never). Applies to the
/// running watcher immediately; persisted by the frontend and re-applied
/// on next launch.
pub fn auto_lock_set(secs: u64) -> Result<(), String> {
    crate::auth::auto_lock::set_timeout(secs);
    Ok(())
}

fn valid_pin(pin: &str) -> bool {
    (4..=6).contains(&pin.len()) && pin.bytes().all(|b| b.is_ascii_digit())
}

fn read_meta(path: &str) -> Result<(VaultMeta, VaultDir, std::path::PathBuf), String> {
    let vdir = VaultDir::new(std::path::Path::new(path).to_path_buf());
    let meta_str =
        std::fs::read_to_string(&vdir.meta_path).map_err(|_| "vault not found".to_string())?;
    let meta: VaultMeta =
        serde_json::from_str(&meta_str).map_err(|_| "vault corrupted".to_string())?;
    let root = std::path::PathBuf::from(path);
    Ok((meta, vdir, root))
}

/// Write vault.meta atomically (temp + rename + fsync), as vault_create does.
fn write_meta(vdir: &VaultDir, meta: &VaultMeta) -> Result<(), String> {
    let meta_json = meta.serialize().map_err(|e| e.to_string())?;
    let tmp_path = vdir.meta_path.with_extension("meta.tmp");
    std::fs::write(&tmp_path, &meta_json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp_path, &vdir.meta_path).map_err(|e| e.to_string())?;
    crate::storage::vault_dir::fsync_dir(&vdir.blobs_dir).map_err(|e| e.to_string())
}

/// Rotate the vault PIN: verify the current PIN, re-wrap the MVK under the
/// new one. Other wrapped entries (password, recovery) are untouched.
pub fn vault_change_pin(
    current_pin: String,
    new_pin: String,
    session: Arc<SessionManager>,
) -> Result<(), String> {
    if session.is_locked() {
        return Err("vault is locked".to_string());
    }
    let path = session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())?;
    if !valid_pin(&new_pin) {
        return Err("PIN must be 4-6 digits".to_string());
    }
    // Verify the current PIN first — never rotate a credential we couldn't
    // have authenticated with.
    crate::auth::pin::unlock_with_pin(std::path::Path::new(&path), current_pin.as_bytes())
        .map_err(|_| "Invalid PIN".to_string())?;

    with_mvk(&session, |mvk| -> Result<(), String> {
        let (mut meta, vdir, _) = read_meta(&path)?;
        let mvk_obj = crate::crypto::mvk::Mvk::from_array(*mvk);
        meta.add_pin_entry(new_pin.as_bytes(), &mvk_obj)
            .map_err(|e| e.to_string())?;
        write_meta(&vdir, &meta)
    })?
}

/// Rotate the vault password: verify current, re-wrap MVK under the new one.
pub fn vault_change_password(
    current_password: String,
    new_password: String,
    session: Arc<SessionManager>,
) -> Result<(), String> {
    if session.is_locked() {
        return Err("vault is locked".to_string());
    }
    let path = session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())?;
    if new_password.len() < 8 {
        return Err("Password must be at least 8 characters".to_string());
    }
    crate::auth::password::unlock_with_password(
        std::path::Path::new(&path),
        current_password.as_bytes(),
    )
    .map_err(|_| "Invalid password".to_string())?;

    with_mvk(&session, |mvk| -> Result<(), String> {
        let (mut meta, vdir, _) = read_meta(&path)?;
        let mvk_obj = crate::crypto::mvk::Mvk::from_array(*mvk);
        meta.add_password_entry(new_password.as_bytes(), &mvk_obj)
            .map_err(|e| e.to_string())?;
        write_meta(&vdir, &meta)
    })?
}

/// Clear the backup history table (Settings → Security). Does NOT touch
/// backup files on disk.
pub fn vault_backup_history_clear(session: Arc<SessionManager>) -> Result<(), String> {
    let db = open_db_for_session(&session)?;
    db.conn()
        .execute("DELETE FROM backup_history", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Danger zone: delete the entire vault directory after re-verifying the
/// PIN. The PIN check happens BEFORE any bytes are removed. Permanently
/// destroys all encrypted files — backup files elsewhere are untouched.
pub fn vault_delete(pin: String, session: Arc<SessionManager>) -> Result<(), String> {
    if session.is_locked() {
        return Err("vault is locked".to_string());
    }
    let path = session
        .current_path()
        .ok_or_else(|| "no vault open".to_string())?;

    // 1. Verify the credential before touching any data.
    crate::auth::pin::unlock_with_pin(std::path::Path::new(&path), pin.as_bytes())
        .map_err(|_| "Invalid PIN".to_string())?;

    // 2. Drop the session first: no open handles should sit inside the
    //    directory we are about to remove.
    session.lock();

    // 3. Remove everything.
    std::fs::remove_dir_all(&path).map_err(|e| format!("Failed to delete vault: {e}"))?;
    Ok(())
}
