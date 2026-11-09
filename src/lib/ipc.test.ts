import { describe, it, expect, vi } from 'vitest';
import {
  vaultCreate,
  vaultOpen,
  vaultUnlockPin,
  vaultUnlockPassword,
  vaultUnlockRecovery,
  vaultLock,
  vaultStatus,
  vaultGetMeta,
  vaultGetSessionToken,
} from './ipc';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: any[]) => mockInvoke(...args),
}));

describe('ipc layer', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('wraps successful invoke in ok result', async () => {
    mockInvoke.mockResolvedValue({ vault_id: 1, name: 'test', path: '/test' });
    const result = await vaultCreate('test', '/test', '1234', 'pass');
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.vault_id).toBe(1);
    }
  });

  it('wraps failed invoke in error result', async () => {
    mockInvoke.mockRejectedValue('vault is locked');
    const result = await vaultStatus();
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.type).toBe('Locked');
    }
  });

  it('vaultUnlockPin calls invoke with correct args', async () => {
    mockInvoke.mockResolvedValue({ vault_id: 1, unlock_method: 'pin' });
    await vaultUnlockPin('1234');
    expect(mockInvoke).toHaveBeenCalledWith('vault_unlock_pin', { pin: '1234' });
  });

  it('vaultUnlockPassword calls invoke with correct args', async () => {
    mockInvoke.mockResolvedValue({ vault_id: 1, unlock_method: 'password' });
    await vaultUnlockPassword('mypassword');
    expect(mockInvoke).toHaveBeenCalledWith('vault_unlock_password', {
      password: 'mypassword',
    });
  });

  it('vaultUnlockRecovery calls invoke with correct args', async () => {
    mockInvoke.mockResolvedValue({ vault_id: 1, unlock_method: 'recovery' });
    await vaultUnlockRecovery('ABCDEF123456');
    expect(mockInvoke).toHaveBeenCalledWith('vault_unlock_recovery', {
      recovery_key: 'ABCDEF123456',
    });
  });

  it('vaultLock calls invoke with empty args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await vaultLock();
    expect(mockInvoke).toHaveBeenCalledWith('vault_lock', {});
  });

  it('vaultGetSessionToken returns token string', async () => {
    mockInvoke.mockResolvedValue('abc123token');
    const result = await vaultGetSessionToken();
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data).toBe('abc123token');
    }
  });

  it('maps unknown error to Internal type', async () => {
    mockInvoke.mockRejectedValue('some unknown error');
    const result = await vaultStatus();
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.type).toBe('Internal');
    }
  });
});
