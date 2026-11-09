export type Result<T, E = string> =
  | { ok: true; data: T }
  | { ok: false; error: E };

export interface VaultInfo {
  name: string;
  path: string;
  vault_id: number;
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
