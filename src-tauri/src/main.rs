#![allow(missing_docs)]
// Release builds are a GUI app: no console window when launched from the
// shortcut. Debug keeps the console so tauri dev logs stay visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::generate_context;
use std::sync::Arc;

use veylora::auth::session::SessionManager;

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

// ---- Phase 9: Layer-1 app login (app_users, no vault access) ----

#[tauri::command]
fn app_status_cmd(
    state: tauri::State<'_, Arc<veylora::app_auth::AppAuth>>,
) -> Result<veylora::app_auth::AppStatus, String> {
    Ok(state.status())
}

#[tauri::command]
fn app_login_cmd(
    email: String,
    password: String,
    state: tauri::State<'_, Arc<veylora::app_auth::AppAuth>>,
) -> Result<veylora::app_auth::AppStatus, String> {
    state.login(&email, &password)
}

#[tauri::command]
fn app_signup_cmd(
    email: String,
    password: String,
    state: tauri::State<'_, Arc<veylora::app_auth::AppAuth>>,
) -> Result<(), String> {
    state.signup(&email, &password)
}

#[tauri::command]
fn app_logout_cmd(
    state: tauri::State<'_, Arc<veylora::app_auth::AppAuth>>,
) -> Result<(), String> {
    state.logout();
    Ok(())
}

#[tauri::command]
fn app_change_password_cmd(
    old_password: String,
    new_password: String,
    state: tauri::State<'_, Arc<veylora::app_auth::AppAuth>>,
) -> Result<(), String> {
    state.change_password(&old_password, &new_password)
}

#[tauri::command]
fn app_list_users_cmd(
    state: tauri::State<'_, Arc<veylora::app_auth::AppAuth>>,
) -> Result<Vec<veylora::app_auth::AppUser>, String> {
    state.list_users()
}

#[tauri::command]
fn app_delete_user_cmd(
    id: u64,
    state: tauri::State<'_, Arc<veylora::app_auth::AppAuth>>,
) -> Result<(), String> {
    state.delete_user(id)
}

#[tauri::command]
fn app_create_user_cmd(
    email: String,
    password: String,
    role: String,
    state: tauri::State<'_, Arc<veylora::app_auth::AppAuth>>,
) -> Result<(), String> {
    state.create_user(&email, &password, &role)
}

// ---- Slice B: file/folder commands (import, list, export, rename, delete) ----

#[tauri::command]
fn file_import_cmd(
    paths: Vec<String>,
    folder_id: Option<u64>,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<Vec<veylora::commands::files::FileInfo>, String> {
    veylora::commands::files::file_import(paths, folder_id, state.inner().clone())
}

#[tauri::command]
fn file_list_cmd(
    folder_id: Option<u64>,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::files::Listing, String> {
    veylora::commands::files::file_list(folder_id, state.inner().clone())
}

#[tauri::command]
fn file_export_cmd(
    id: u64,
    dest_path: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<u64, String> {
    veylora::commands::files::file_export(id, dest_path, state.inner().clone())
}

#[tauri::command]
fn file_delete_cmd(
    id: u64,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<(), String> {
    veylora::commands::files::file_delete(id, state.inner().clone())
}

#[tauri::command]
fn file_rename_cmd(
    id: u64,
    new_filename: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<(), String> {
    veylora::commands::files::file_rename(id, new_filename, state.inner().clone())
}

#[tauri::command]
fn folder_list_cmd(
    parent_id: Option<u64>,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<Vec<veylora::commands::files::FolderInfo>, String> {
    veylora::commands::files::folder_list(parent_id, state.inner().clone())
}

#[tauri::command]
fn folder_create_cmd(
    name: String,
    parent_id: Option<u64>,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<veylora::commands::files::FolderInfo, String> {
    veylora::commands::files::folder_create(name, parent_id, state.inner().clone())
}

// ---- Slice C: favorites / recent / activity / settings / danger zone ----

#[tauri::command]
fn file_favorite_cmd(
    id: u64,
    favorite: bool,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<(), String> {
    veylora::commands::files::file_favorite(id, favorite, state.inner().clone())
}

#[tauri::command]
fn file_favorites_cmd(
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<Vec<veylora::commands::files::FileInfo>, String> {
    veylora::commands::files::file_favorites(state.inner().clone())
}

#[tauri::command]
fn file_recent_cmd(
    limit: u64,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<Vec<veylora::commands::files::FileInfo>, String> {
    veylora::commands::files::file_recent(limit, state.inner().clone())
}

#[tauri::command]
fn vault_activity_cmd(state: tauri::State<'_, Arc<SessionManager>>) -> Result<(), String> {
    veylora::commands::vault_activity(state.inner().clone())
}

#[tauri::command]
fn auto_lock_set_cmd(secs: u64) -> Result<(), String> {
    veylora::commands::auto_lock_set(secs)
}

#[tauri::command]
fn vault_change_pin_cmd(
    current_pin: String,
    new_pin: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<(), String> {
    veylora::commands::vault_change_pin(current_pin, new_pin, state.inner().clone())
}

#[tauri::command]
fn vault_change_password_cmd(
    current_password: String,
    new_password: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<(), String> {
    veylora::commands::vault_change_password(
        current_password,
        new_password,
        state.inner().clone(),
    )
}

#[tauri::command]
fn vault_backup_history_clear_cmd(
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<(), String> {
    veylora::commands::vault_backup_history_clear(state.inner().clone())
}

#[tauri::command]
fn vault_delete_cmd(
    pin: String,
    state: tauri::State<'_, Arc<SessionManager>>,
) -> Result<(), String> {
    veylora::commands::vault_delete(pin, state.inner().clone())
}

fn main() {
    let manager: Arc<SessionManager> = Arc::new(SessionManager::new());
    let app_auth: Arc<veylora::app_auth::AppAuth> =
        Arc::new(veylora::app_auth::AppAuth::new().expect("APPDATA not available"));

    // Auto-lock watcher: previously dead code (never spawned). Reads the
    // timeout from Settings on every tick (0 = never).
    veylora::auth::auto_lock::spawn_watcher(manager.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(manager.clone())
        .manage(app_auth)
        .register_uri_scheme_protocol("vault", move |_app, request| {
            // wry reverts http://vault.localhost/... to vault://localhost/...;
            // the streaming parser expects vault://file/..., so drop the host.
            let (mut parts, body) = request.into_parts();
            let rewritten = parts
                .uri
                .to_string()
                .replacen("vault://localhost/", "vault://", 1);
            if let Ok(uri) = rewritten.parse() {
                parts.uri = uri;
            }
            let origin = parts.headers.get(tauri::http::header::ORIGIN).cloned();
            let request = tauri::http::Request::from_parts(parts, body);

            // Same status map as streaming/protocol.rs so vaultFetch's 403
            // handling (treat as unauthorized) behaves identically.
            let (status, body): (u16, Vec<u8>) = match veylora::commands::files::file_preview(
                request.uri().to_string(),
                manager.clone(),
            ) {
                Ok(bytes) => (200, bytes),
                Err(e) => {
                    let code = match e.as_str() {
                        "vault is locked" | "no vault open" | "invalid token" => 403,
                        "file not found" => 404,
                        "invalid URL" | "invalid file id" | "invalid range" => 400,
                        _ if e.starts_with("range out of bounds") => 416,
                        _ => 500,
                    };
                    (code, Vec::new())
                }
            };
            let mut response = tauri::http::Response::builder()
                .status(status)
                .body(body)
                .unwrap();
            // The app window is cross-origin to http://vault.localhost; without
            // ACAO the browser hides decrypted bytes from JS fetch (PDF/preview).
            if let Some(origin) = origin {
                response.headers_mut().insert(
                    tauri::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    origin,
                );
            }
            response
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
            app_status_cmd,
            app_login_cmd,
            app_signup_cmd,
            app_logout_cmd,
            app_change_password_cmd,
            app_list_users_cmd,
            app_delete_user_cmd,
            app_create_user_cmd,
            file_import_cmd,
            file_list_cmd,
            file_export_cmd,
            file_delete_cmd,
            file_rename_cmd,
            folder_list_cmd,
            folder_create_cmd,
            file_favorite_cmd,
            file_favorites_cmd,
            file_recent_cmd,
            vault_activity_cmd,
            auto_lock_set_cmd,
            vault_change_pin_cmd,
            vault_change_password_cmd,
            vault_backup_history_clear_cmd,
            vault_delete_cmd,
        ])
        .run(generate_context!())
        .expect("error while running tauri application");
}
