import { invoke } from '@tauri-apps/api/core';
import type {
  Result,
  VaultInfo,
  UnlockResult,
  StatusInfo,
  VaultPublicMeta,
  CommandError,
  AppStatus,
  AppUser,
  FileInfo,
  FolderInfo,
  Listing,
} from './types';

type CommandMap = {
  vault_create_cmd: {
    args: { name: string; path: string; pin: string; password: string };
    result: VaultInfo;
  };
  vault_open_cmd: {
    args: { path: string };
    result: VaultInfo;
  };
  vault_unlock_pin_cmd: {
    args: { pin: string };
    result: UnlockResult;
  };
  vault_unlock_password_cmd: {
    args: { password: string };
    result: UnlockResult;
  };
  vault_unlock_recovery_cmd: {
    args: { recovery_key: string };
    result: UnlockResult;
  };
  vault_lock_cmd: {
    args: {};
    result: void;
  };
  vault_status_cmd: {
    args: {};
    result: StatusInfo;
  };
  vault_get_meta_cmd: {
    args: { path: string };
    result: VaultPublicMeta;
  };
  vault_get_session_token_cmd: {
    args: {};
    result: string;
  };
  app_status_cmd: {
    args: {};
    result: AppStatus;
  };
  app_login_cmd: {
    args: { email: string; password: string };
    result: AppStatus;
  };
  app_signup_cmd: {
    args: { email: string; password: string };
    result: void;
  };
  app_logout_cmd: {
    args: {};
    result: void;
  };
  app_change_password_cmd: {
    args: { oldPassword: string; newPassword: string };
    result: void;
  };
  app_list_users_cmd: {
    args: {};
    result: AppUser[];
  };
  app_delete_user_cmd: {
    args: { id: number };
    result: void;
  };
  app_create_user_cmd: {
    args: { email: string; password: string; role: string };
    result: void;
  };
  file_import_cmd: {
    args: { paths: string[]; folderId: number | null };
    result: FileInfo[];
  };
  file_list_cmd: {
    args: { folderId: number | null };
    result: Listing;
  };
  file_export_cmd: {
    args: { id: number; destPath: string };
    result: number;
  };
  file_delete_cmd: {
    args: { id: number };
    result: void;
  };
  file_rename_cmd: {
    args: { id: number; newFilename: string };
    result: void;
  };
  folder_list_cmd: {
    args: { parentId: number | null };
    result: FolderInfo[];
  };
  folder_create_cmd: {
    args: { name: string; parentId: number | null };
    result: FolderInfo;
  };
  file_favorite_cmd: {
    args: { id: number; favorite: boolean };
    result: void;
  };
  file_favorites_cmd: {
    args: {};
    result: FileInfo[];
  };
  file_recent_cmd: {
    args: { limit: number };
    result: FileInfo[];
  };
  vault_activity_cmd: {
    args: {};
    result: void;
  };
  auto_lock_set_cmd: {
    args: { secs: number };
    result: void;
  };
  vault_change_pin_cmd: {
    args: { currentPin: string; newPin: string };
    result: void;
  };
  vault_change_password_cmd: {
    args: { currentPassword: string; newPassword: string };
    result: void;
  };
  vault_backup_history_clear_cmd: {
    args: {};
    result: void;
  };
  vault_delete_cmd: {
    args: { pin: string };
    result: void;
  };
};

async function invokeCmd<K extends keyof CommandMap>(
  command: K,
  args: CommandMap[K]['args'],
): Promise<CommandMap[K]['result']> {
  return invoke(command, args);
}

export async function vaultCreate(
  name: string,
  path: string,
  pin: string,
  password: string,
): Promise<Result<VaultInfo, CommandError>> {
  try {
    const data = await invokeCmd('vault_create_cmd', {
      name,
      path,
      pin,
      password,
    });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultOpen(
  path: string,
): Promise<Result<VaultInfo, CommandError>> {
  try {
    const data = await invokeCmd('vault_open_cmd', { path });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultUnlockPin(
  pin: string,
): Promise<Result<UnlockResult, CommandError>> {
  try {
    const data = await invokeCmd('vault_unlock_pin_cmd', { pin });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultUnlockPassword(
  password: string,
): Promise<Result<UnlockResult, CommandError>> {
  try {
    const data = await invokeCmd('vault_unlock_password_cmd', { password });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultUnlockRecovery(
  recoveryKey: string,
): Promise<Result<UnlockResult, CommandError>> {
  try {
    // Tauri 2 expects camelCase keys (plain #[tauri::command] default).
    const data = await invokeCmd('vault_unlock_recovery_cmd', { recoveryKey });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultLock(): Promise<Result<void, CommandError>> {
  try {
    await invokeCmd('vault_lock_cmd', {});
    return { ok: true, data: undefined };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultStatus(): Promise<Result<StatusInfo, CommandError>> {
  try {
    const data = await invokeCmd('vault_status_cmd', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultGetMeta(
  path: string,
): Promise<Result<VaultPublicMeta, CommandError>> {
  try {
    const data = await invokeCmd('vault_get_meta_cmd', { path });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultGetSessionToken(): Promise<Result<string, CommandError>> {
  try {
    const data = await invokeCmd('vault_get_session_token_cmd', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultBackupCreate(
  destPath: string,
): Promise<Result<BackupInfo, CommandError>> {
  try {
    const data = await invokeCmd('vault_backup_create_cmd', { destPath });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultBackupVerify(
  backupPath: string,
): Promise<Result<BackupInfo, CommandError>> {
  try {
    const data = await invokeCmd('vault_backup_verify_cmd', { backupPath });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultBackupRestore(
  backupPath: string,
): Promise<Result<RestoreInfo, CommandError>> {
  try {
    const data = await invokeCmd('vault_backup_restore_cmd', { backupPath });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultBackupHistory(): Promise<Result<BackupHistoryEntry[], CommandError>> {
  try {
    const data = await invokeCmd('vault_backup_history_cmd', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

// ---- Phase 9: Layer-1 app login ----

export async function appStatus(): Promise<Result<AppStatus, CommandError>> {
  try {
    const data = await invokeCmd('app_status_cmd', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function appLogin(
  email: string,
  password: string,
): Promise<Result<AppStatus, CommandError>> {
  try {
    const data = await invokeCmd('app_login_cmd', { email, password });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function appSignup(
  email: string,
  password: string,
): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('app_signup_cmd', { email, password });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function appLogout(): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('app_logout_cmd', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function appChangePassword(
  oldPassword: string,
  newPassword: string,
): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('app_change_password_cmd', {
      oldPassword,
      newPassword,
    });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function appListUsers(): Promise<Result<AppUser[], CommandError>> {
  try {
    const data = await invokeCmd('app_list_users_cmd', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function appDeleteUser(id: number): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('app_delete_user_cmd', { id });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function appCreateUser(
  email: string,
  password: string,
  role: string,
): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('app_create_user_cmd', { email, password, role });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

// ---- Slice B: files and folders ----

export async function fileImport(
  paths: string[],
  folderId: number | null = null,
): Promise<Result<FileInfo[], CommandError>> {
  try {
    const data = await invokeCmd('file_import_cmd', { paths, folderId });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function fileList(
  folderId: number | null = null,
): Promise<Result<Listing, CommandError>> {
  try {
    const data = await invokeCmd('file_list_cmd', { folderId });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function fileExport(
  id: number,
  destPath: string,
): Promise<Result<number, CommandError>> {
  try {
    const data = await invokeCmd('file_export_cmd', { id, destPath });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function fileDelete(id: number): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('file_delete_cmd', { id });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function fileRename(
  id: number,
  newFilename: string,
): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('file_rename_cmd', { id, newFilename });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function folderList(
  parentId: number | null = null,
): Promise<Result<FolderInfo[], CommandError>> {
  try {
    const data = await invokeCmd('folder_list_cmd', { parentId });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function folderCreate(
  name: string,
  parentId: number | null = null,
): Promise<Result<FolderInfo, CommandError>> {
  try {
    const data = await invokeCmd('folder_create_cmd', { name, parentId });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

// ---- Slice C: favorites / recent / activity / settings / danger zone ----

export async function fileFavorite(
  id: number,
  favorite: boolean,
): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('file_favorite_cmd', { id, favorite });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function fileFavorites(): Promise<Result<FileInfo[], CommandError>> {
  try {
    const data = await invokeCmd('file_favorites_cmd', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function fileRecent(
  limit: number,
): Promise<Result<FileInfo[], CommandError>> {
  try {
    const data = await invokeCmd('file_recent_cmd', { limit });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultActivity(): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('vault_activity_cmd', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function autoLockSet(secs: number): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('auto_lock_set_cmd', { secs });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultChangePin(
  currentPin: string,
  newPin: string,
): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('vault_change_pin_cmd', { currentPin, newPin });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultChangePassword(
  currentPassword: string,
  newPassword: string,
): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('vault_change_password_cmd', {
      currentPassword,
      newPassword,
    });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultBackupHistoryClear(): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('vault_backup_history_clear_cmd', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultDelete(pin: string): Promise<Result<void, CommandError>> {
  try {
    const data = await invokeCmd('vault_delete_cmd', { pin });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

function parseError(e: unknown): CommandError {
  if (typeof e === 'string') {
    if (e === 'vault is locked') return { type: 'Locked' };
    if (e === 'vault not found at path') return { type: 'VaultNotFound' };
    return { type: 'Internal', message: e };
  }
  if (typeof e === 'object' && e !== null && 'type' in e) {
    return e as CommandError;
  }
  return { type: 'Internal', message: String(e) };
}
