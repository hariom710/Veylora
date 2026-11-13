import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  vaultCreate,
  vaultUnlockPin,
  vaultUnlockPassword,
  vaultUnlockRecovery,
  vaultLock,
  vaultStatus,
  vaultGetSessionToken,
  appLogin,
  fileImport,
  fileExport,
  fileRename,
  folderCreate,
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
    expect(mockInvoke).toHaveBeenCalledWith('vault_unlock_pin_cmd', { pin: '1234' });
  });

  it('vaultUnlockPassword calls invoke with correct args', async () => {
    mockInvoke.mockResolvedValue({ vault_id: 1, unlock_method: 'password' });
    await vaultUnlockPassword('mypassword');
    expect(mockInvoke).toHaveBeenCalledWith('vault_unlock_password_cmd', {
      password: 'mypassword',
    });
  });

  it('vaultUnlockRecovery calls invoke with correct args', async () => {
    mockInvoke.mockResolvedValue({ vault_id: 1, unlock_method: 'recovery' });
    await vaultUnlockRecovery('ABCDEF123456');
    // Tauri 2 expects camelCase keys — snake_case fails with
    // "command ... missing required key recoveryKey".
    expect(mockInvoke).toHaveBeenCalledWith('vault_unlock_recovery_cmd', {
      recoveryKey: 'ABCDEF123456',
    });
  });

  it('appLogin calls invoke with camelCase command name and args', async () => {
    mockInvoke.mockResolvedValue({
      logged_in: true,
      email: 'a@b.co',
      role: 'user',
      logged_in_since: 1,
    });
    const result = await appLogin('a@b.co', 'secret123');
    expect(mockInvoke).toHaveBeenCalledWith('app_login_cmd', {
      email: 'a@b.co',
      password: 'secret123',
    });
    expect(result.ok).toBe(true);
  });

  it('fileImport/fileExport/fileRename send camelCase keys (defect #5)', async () => {
    mockInvoke.mockResolvedValue([]);
    await fileImport(['/tmp/a.pdf'], 7);
    expect(mockInvoke).toHaveBeenCalledWith('file_import_cmd', {
      paths: ['/tmp/a.pdf'],
      folderId: 7,
    });

    mockInvoke.mockResolvedValue(10);
    await fileExport(3, 'C:\\out\\a.pdf');
    expect(mockInvoke).toHaveBeenCalledWith('file_export_cmd', {
      id: 3,
      destPath: 'C:\\out\\a.pdf',
    });

    mockInvoke.mockResolvedValue(undefined);
    await fileRename(3, 'new.pdf');
    expect(mockInvoke).toHaveBeenCalledWith('file_rename_cmd', {
      id: 3,
      newFilename: 'new.pdf',
    });

    mockInvoke.mockResolvedValue([]);
    await folderCreate('Docs', null);
    expect(mockInvoke).toHaveBeenCalledWith('folder_create_cmd', {
      name: 'Docs',
      parentId: null,
    });
  });

  it('vaultLock calls invoke with empty args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await vaultLock();
    expect(mockInvoke).toHaveBeenCalledWith('vault_lock_cmd', {});
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
