import { invoke } from '@tauri-apps/api/core';
import type {
  Result,
  VaultInfo,
  UnlockResult,
  StatusInfo,
  VaultPublicMeta,
  CommandError,
} from './types';

type CommandMap = {
  vault_create: {
    args: { name: string; path: string; pin: string; password: string };
    result: VaultInfo;
  };
  vault_open: {
    args: { path: string };
    result: VaultInfo;
  };
  vault_unlock_pin: {
    args: { pin: string };
    result: UnlockResult;
  };
  vault_unlock_password: {
    args: { password: string };
    result: UnlockResult;
  };
  vault_unlock_recovery: {
    args: { recovery_key: string };
    result: UnlockResult;
  };
  vault_lock: {
    args: {};
    result: void;
  };
  vault_status: {
    args: {};
    result: StatusInfo;
  };
  vault_get_meta: {
    args: { path: string };
    result: VaultPublicMeta;
  };
  vault_get_session_token_cmd: {
    args: {};
    result: string;
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
    const data = await invokeCmd('vault_create', {
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
    const data = await invokeCmd('vault_open', { path });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultUnlockPin(
  pin: string,
): Promise<Result<UnlockResult, CommandError>> {
  try {
    const data = await invokeCmd('vault_unlock_pin', { pin });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultUnlockPassword(
  password: string,
): Promise<Result<UnlockResult, CommandError>> {
  try {
    const data = await invokeCmd('vault_unlock_password', { password });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultUnlockRecovery(
  recoveryKey: string,
): Promise<Result<UnlockResult, CommandError>> {
  try {
    const data = await invokeCmd('vault_unlock_recovery', { recovery_key: recoveryKey });
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultLock(): Promise<Result<void, CommandError>> {
  try {
    await invokeCmd('vault_lock', {});
    return { ok: true, data: undefined };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultStatus(): Promise<Result<StatusInfo, CommandError>> {
  try {
    const data = await invokeCmd('vault_status', {});
    return { ok: true, data };
  } catch (e) {
    return { ok: false, error: parseError(e) };
  }
}

export async function vaultGetMeta(
  path: string,
): Promise<Result<VaultPublicMeta, CommandError>> {
  try {
    const data = await invokeCmd('vault_get_meta', { path });
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
