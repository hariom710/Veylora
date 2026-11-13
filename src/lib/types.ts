export type Result<T, E = string> =
  | { ok: true; data: T }
  | { ok: false; error: E };

export interface VaultInfo {
  name: string;
  path: string;
  vault_id: number;
  /** Encoded recovery key, non-empty exactly once (at vault creation). */
  recovery_key: string;
}

export interface UnlockResult {
  vault_id: number;
  unlock_method: string;
}

export interface StatusInfo {
  locked: boolean;
  vault_id: number | null;
  file_count: number;
  storage_used: number;
  vault_open: boolean;
}

export interface AppStatus {
  logged_in: boolean;
  email: string | null;
  role: string | null;
  logged_in_since: number | null;
}

export interface AppUser {
  id: number;
  email: string;
  role: string;
  created_at: number;
  last_login: number | null;
}

export interface FileInfo {
  id: number;
  filename: string;
  file_id: string;
  folder_id: number | null;
  encrypted_size: number;
  created_at: string;
  modified_at: string;
}

export interface FolderInfo {
  id: number;
  parent_id: number | null;
  name: string;
  created_at: string;
}

export interface Listing {
  files: FileInfo[];
  folders: FolderInfo[];
}

export interface VaultPublicMeta {
  name: string;
  path: string;
  version: number;
  kdf_params_public: string;
}

export interface FileRecord {
  id: number;
  vault_id: number;
  file_id: string;
  filename: string;
  folder_id: number | null;
  encrypted_size: number;
  encryption_version: number;
  created_at: string;
  modified_at: string;
}

export interface FolderRecord {
  id: number;
  vault_id: number;
  parent_id: number | null;
  name: string;
  created_at: string;
}

export type CommandError =
  | { type: 'Locked' }
  | { type: 'VaultNotFound' }
  | { type: 'Internal'; message: string };

export interface BackupInfo {
  path: string;
  file_count: number;
  total_size: number;
  created_at_unix: number;
  verified_fields: string[];
}

export interface RestoreInfo {
  files_restored: number;
  bytes_written: number;
}

export interface BackupHistoryEntry {
  path: string;
  status: string;
  created_at_unix: number;
}

/** Human-readable text for an IPC error; only `Internal` carries a message. */
export function commandErrorMessage(error: CommandError): string {
  if (error.type === 'Internal') return error.message;
  return error.type === 'Locked' ? 'Vault is locked' : 'Vault not found at path';
}
