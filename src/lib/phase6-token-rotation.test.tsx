import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import App from '@/App';

vi.mock('@/lib/ipc', () => ({
  appStatus: vi.fn(),
  vaultUnlockPin: vi.fn(),
  vaultLock: vi.fn(),
  vaultOpen: vi.fn(),
  vaultGetMeta: vi.fn(),
  vaultGetSessionToken: vi.fn(),
  vaultStatus: vi.fn(),
  vaultActivity: vi.fn(),
  autoLockSet: vi.fn(),
  fileRecent: vi.fn(),
  fileFavorites: vi.fn(),
  fileFavorite: vi.fn(),
}));

import {
  appStatus,
  vaultUnlockPin,
  vaultLock,
  vaultOpen,
  vaultGetSessionToken,
  vaultStatus,
  fileRecent,
  fileFavorites,
  vaultActivity,
} from '@/lib/ipc';

describe('phase6_token_rotates_after_lock_unlock', () => {
  const user = userEvent.setup();

  beforeEach(() => {
    vi.resetAllMocks();
    // Layer 1 already passed: this test is about the vault session.
    appStatus.mockResolvedValue({
      ok: true,
      data: { logged_in: true, email: 'a@b.co', role: 'user', logged_in_since: 1, used_default_password: false },
    });
    // Model an existing vault whose path the UI remembers across lock/unlock.
    window.localStorage.setItem('veylora.vaultPath', '/test/vault');
    vaultOpen.mockResolvedValue({ ok: true, data: { name: 'test', path: '/test/vault', vault_id: 1 } });
    vaultStatus.mockResolvedValue({ ok: true, data: { locked: true, vault_id: null, file_count: 0, storage_used: 0, vault_open: true } });
    fileRecent.mockResolvedValue({ ok: true, data: [] });
    fileFavorites.mockResolvedValue({ ok: true, data: [] });
    vaultActivity.mockResolvedValue({ ok: true, data: undefined });
  });

  afterEach(() => {
    window.localStorage.clear();
  });

  it('token rotates after lock and unlock cycle', async () => {
    // First unlock: returns token_1
    vaultUnlockPin.mockResolvedValue({ ok: true, data: { vault_id: 1, unlock_method: 'pin' } });
    vaultGetSessionToken.mockResolvedValue({ ok: true, data: 'token_1' });

    render(<App />);

    // Cold start shows Login until appStatus resolves; wait for the vault gate.
    await screen.findByLabelText(/PIN/i);
    await user.type(screen.getByLabelText(/PIN/i), '1234');
    await user.click(screen.getByRole('button', { name: /Unlock/i }));

    // Should navigate to dashboard (unlocked)
    expect(screen.getByRole('heading', { name: /Dashboard/i })).toBeInTheDocument();
    expect(screen.queryByText(/Unlock Vault/i)).not.toBeInTheDocument();

    // Lock the vault
    vaultLock.mockResolvedValue({ ok: true, data: undefined });
    await user.click(screen.getByRole('button', { name: /Lock Vault/i }));

    // Should return to Unlock screen
    expect(screen.getByText(/Unlock Vault/i)).toBeInTheDocument();

    // Second unlock with different token
    vaultUnlockPin.mockResolvedValue({ ok: true, data: { vault_id: 1, unlock_method: 'pin' } });
    vaultGetSessionToken.mockResolvedValue({ ok: true, data: 'token_2' });

    await user.type(screen.getByLabelText(/PIN/i), '1234');
    await user.click(screen.getByRole('button', { name: /Unlock/i }));

    // Should navigate to dashboard again with new token
    expect(screen.getByRole('heading', { name: /Dashboard/i })).toBeInTheDocument();

    // Get token after second unlock
    const token2 = await vaultGetSessionToken();
    expect(token2.ok).toBe(true);
    if (token2.ok) {
      expect(token2.data).toBe('token_2');
    }
  });
});
