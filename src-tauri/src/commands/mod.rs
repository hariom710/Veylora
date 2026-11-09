//! Tauri IPC command handlers (thin layer, no crypto).
//!
//! No command argument or return type may contain key material (MVK, KEK,
//! nonce, ciphertext, etc.). Plaintext file metadata (filenames, etc.) is
//! allowed because the UI needs to display it — the rule is about key
//! material, not user data.

#![allow(missing_docs)]

use std::sync::Arc;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use crate::auth::session::SessionManager;
use crate::auth::{AuthError, UnlockMethod};
use crate::database::Db;
use crate::storage::vault_dir::VaultDir;
use crate::vault::VaultMeta;

/// Information returned on vault create/open.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VaultInfo {
    pub name: String,
    pub path: String,
    pub vault_id: u64,
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
    name: String,
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

    // Write vault.meta atomically
    let meta_json = meta.serialize().map_err(|e| e.to_string())?;
    let meta_path = vdir.meta_path.clone();
    let tmp_path = vdir.meta_path.with_extension("meta.tmp");
    std::fs::write(&tmp_path, &meta_json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp_path, &meta_path).map_err(|e| e.to_string())?;
    crate::storage::vault_dir::fsync_dir(&vdir.blobs_dir).map_err(|e| e.to_string())?;

    // Create vault.db and apply migrations
    let db = crate::database::Db::open(&vdir.db_path)
        .map_err(|e| e.to_string())?;

    // Insert vault record
    db.conn().execute(
        "INSERT INTO vaults (id, created_at) VALUES (?1, ?2)",
        rusqlite::params![1, std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()],
    ).map_err(|e| e.to_string())?;

    let vault_id = vault_id_from_meta(&meta);
    session.set_session(
        crate::auth::session::Session::new(mvk, vault_id, UnlockMethod::Pin),
        path.clone(),
    );

    Ok(VaultInfo {
        name: path.clone(),
        path: path.clone(),
        vault_id,
    })
}

/// Open an existing vault (read metadata only, set current path).
#[allow(dead_code)]
pub fn vault_open(
    path: String,
    session: Arc<SessionManager>,
) -> Result<VaultInfo, String> {
    let vdir = VaultDir::new(std::path::Path::new(&path).to_path_buf());
    let meta_path = &vdir.meta_path;

    let meta_str = std::fs::read_to_string(meta_path)
        .map_err(|_| "vault not found".to_string())?;
    let meta: VaultMeta = serde_json::from_str(&meta_str)
        .map_err(|_| "vault corrupted".to_string())?;

    let vault_id = vault_id_from_meta(&meta);

    // Store the current path (no session yet — locked state)
    session.set_current_path(path.clone());

    Ok(VaultInfo {
        name: path.clone(),
        path: path.clone(),
        vault_id,
    })
}

/// Unlock with PIN.
#[allow(dead_code)]
pub fn vault_unlock_pin(
    pin: String,
    session: Arc<SessionManager>,
) -> Result<UnlockResult, String> {
    let path = session.current_path().ok_or_else(|| "no vault open".to_string())?;

    let db = crate::database::Db::open(
        &std::path::Path::new(&path).join("vault.db"),
    ).map_err(|e| e.to_string())?;

    let vault_id = session.vault_id().ok_or_else(|| "no active session".to_string())?;

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
    let path = session.current_path().ok_or_else(|| "no vault open".to_string())?;
    let db = crate::database::Db::open(
        &std::path::Path::new(&path).join("vault.db"),
    ).map_err(|e| e.to_string())?;

    let vault_id = session.vault_id().ok_or_else(|| "no active session".to_string())?;

    if let Some(lockout) = crate::auth::throttle::check_locked(&db, vault_id) {
        let remaining = lockout.duration_since(std::time::Instant::now()).as_secs();
        return Err(format!("locked out: try again in {}s", remaining));
    }

    match crate::auth::password::unlock_with_password(std::path::Path::new(&path), password.as_bytes()) {
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
    let path = session.current_path().ok_or_else(|| "no vault open".to_string())?;
    let db = crate::database::Db::open(
        &std::path::Path::new(&path).join("vault.db"),
    ).map_err(|e| e.to_string())?;

    let vault_id = session.vault_id().ok_or_else(|| "no active session".to_string())?;

    if let Some(lockout) = crate::auth::throttle::check_locked(&db, vault_id) {
        let remaining = lockout.duration_since(std::time::Instant::now()).as_secs();
        return Err(format!("locked out: try again in {}s", remaining));
    }

    let recovery_bytes = crate::crypto::recovery::decode_recovery_key(&recovery_key)
        .map_err(|e| e.to_string())?;

    match crate::auth::recovery::unlock_with_recovery(std::path::Path::new(&path), &recovery_bytes) {
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
pub fn vault_lock(
    session: Arc<SessionManager>,
) -> Result<(), String> {
    session.lock();
    Ok(())
}

/// Vault status: locked/unlocked, vault_id, file_count, storage_used.
#[allow(dead_code)]
pub fn vault_status(
    session: Arc<SessionManager>,
) -> Result<StatusInfo, String> {
    let locked = session.is_locked();
    let vault_id = session.vault_id();

    Ok(StatusInfo {
        locked,
        vault_id,
        file_count: 0,
        storage_used: 0,
    })
}

/// Public vault metadata (no keys).
#[allow(dead_code)]
pub fn vault_get_meta(
    path: String,
) -> Result<VaultPublicMeta, String> {
    let vdir = VaultDir::new(std::path::Path::new(&path).to_path_buf());
    let meta_path = &vdir.meta_path;

    let meta_str = std::fs::read_to_string(meta_path)
        .map_err(|_| "vault not found".to_string())?;
    let meta: VaultMeta = serde_json::from_str(&meta_str)
        .map_err(|_| "vault corrupted".to_string())?;

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
/// The token is NOT the MVK, KEK, or any derived key — it is a random
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
fn vault_id_from_meta(meta: &VaultMeta) -> u64 {
    let mut hasher = DefaultHasher::new();
    meta.hash(&mut hasher);
    hasher.finish()
}
