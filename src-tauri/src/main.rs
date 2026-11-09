#![allow(missing_docs)]

use tauri::generate_context;
use std::sync::Arc;
use std::path::Path;

use veylora::auth::session::SessionManager;
use veylora::streaming::protocol::register_vault_protocol;
use veylora::database::Db;
use veylora::storage::vault_dir::VaultDir;
use veylora::commands::{
    vault_create, vault_open, vault_unlock_pin, vault_unlock_password,
    vault_unlock_recovery, vault_lock, vault_status, vault_get_meta,
    vault_get_session_token, CommandError,
    VaultInfo, UnlockResult, StatusInfo, VaultPublicMeta,
    vault_backup_create, vault_backup_verify, vault_backup_restore,
    vault_backup_history, BackupInfo, RestoreInfo, BackupHistoryEntry,
};

#[tauri::command]
fn vault_status_cmd(
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::StatusInfo, String> {
    veylora::commands::vault_status(state.inner().clone())
}

#[tauri::command]
fn vault_lock_cmd(
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<(), String> {
    veylora::commands::vault_lock(state.inner().clone())
}

#[tauri::command]
fn vault_get_meta_cmd(
    path: String,
) -> Result<veylora::commands::VaultPublicMeta, String> {
    veylora::commands::vault_get_meta(path)
}

#[tauri::command]
fn vault_get_session_token_cmd(
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<String, veylora::commands::CommandError> {
    veylora::commands::vault_get_session_token(state.inner().as_ref())
}

#[tauri::command]
fn vault_create_cmd(
    name: String,
    path: String,
    pin: String,
    password: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::VaultInfo, String> {
    veylora::commands::vault_create(name, path, pin, password, state.inner().clone())
}

#[tauri::command]
fn vault_open_cmd(
    path: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::VaultInfo, String> {
    veylora::commands::vault_open(path, state.inner().clone())
}

#[tauri::command]
fn vault_unlock_pin_cmd(
    pin: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::UnlockResult, String> {
    veylora::commands::vault_unlock_pin(pin, state.inner().clone())
}

#[tauri::command]
fn vault_unlock_password_cmd(
    password: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::UnlockResult, String> {
    veylora::commands::vault_unlock_password(password, state.inner().clone())
}

#[tauri::command]
fn vault_unlock_recovery_cmd(
    recovery_key: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::UnlockResult, String> {
    veylora::commands::vault_unlock_recovery(recovery_key, state.inner().clone())
}

#[tauri::command]
fn vault_backup_create_cmd(
    dest_path: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::BackupInfo, String> {
    veylora::commands::vault_backup_create(dest_path, state.inner().clone())
}

#[tauri::command]
fn vault_backup_verify_cmd(
    backup_path: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::BackupInfo, String> {
    veylora::commands::vault_backup_verify(backup_path, state.inner().clone())
}

#[tauri::command]
fn vault_backup_restore_cmd(
    backup_path: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::RestoreInfo, String> {
    veylora::commands::vault_backup_restore(backup_path, state.inner().clone())
}

#[tauri::command]
fn vault_backup_history_cmd(
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<Vec<veylora::commands::BackupHistoryEntry>, String> {
    veylora::commands::vault_backup_history(state.inner().clone())
}

fn main() {
    let manager: Arc<SessionManager> = Arc::new(SessionManager::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(manager.clone())
        .register_uri_scheme_protocol("vault", move |_app, request| {
            let current_path = manager.current_path().unwrap_or_default();
            let vault_dir_path = Path::new(&current_path).parent().unwrap_or(Path::new(""));
            let db = match Db::open(vault_dir_path) {
                Ok(d) => d,
                Err(_) => return tauri::http::Response::builder()
                    .status(500)
                    .body(Vec::new())
                    .unwrap(),
            };
            let dir = VaultDir::new(vault_dir_path.to_path_buf());
            veylora::streaming::protocol::handle_protocol_request(
                &manager, &db, &dir, request,
            )
        })
        .invoke_handler(tauri::generate_handler![
            vault_status_cmd,
            vault_lock_cmd,
            vault_get_meta_cmd,
            vault_get_session_token_cmd,
            vault_create_cmd,
            vault_open_cmd,
            vault_unlock_pin_cmd,
            vault_unlock_password_cmd,
            vault_unlock_recovery_cmd,
            vault_backup_create_cmd,
            vault_backup_verify_cmd,
            vault_backup_restore_cmd,
            vault_backup_history_cmd,
        ])
        .run(generate_context!())
        .expect("error while running tauri application");
}
