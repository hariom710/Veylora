import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import App from '@/App';

vi.mock('@/lib/ipc', () => ({
  appStatus: vi.fn(),
  vaultStatus: vi.fn(),
  vaultOpen: vi.fn(),
  vaultGetSessionToken: vi.fn(),
  vaultUnlockPin: vi.fn(),
  vaultUnlockPassword: vi.fn(),
  vaultUnlockRecovery: vi.fn(),
  vaultLock: vi.fn(),
  vaultActivity: vi.fn(),
  autoLockSet: vi.fn(),
  fileRecent: vi.fn(),
  fileFavorites: vi.fn(),
  fileFavorite: vi.fn(),
}));

import { appStatus, vaultStatus, vaultOpen } from '@/lib/ipc';

describe('first run (no vault open)', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    window.localStorage.clear();
    // Layer 1 already passed: this test is about the vault gate.
    appStatus.mockResolvedValue({
      ok: true,
      data: { logged_in: true, email: 'a@b.co', role: 'user', logged_in_since: 1, used_default_password: false },
    });
    vaultStatus.mockResolvedValue({
      ok: true,
      data: { locked: true, vault_id: null, file_count: 0, storage_used: 0, vault_open: false },
    });
  });

  it('shows Create Vault instead of Unlock when no vault is open', async () => {
    render(<App />);

    expect(await screen.findByRole('heading', { name: /Create Vault/i })).toBeInTheDocument();
    expect(screen.queryByText(/Unlock Vault/i)).not.toBeInTheDocument();
    // No stored path: boot must not attempt to re-open anything.
    expect(vaultOpen).not.toHaveBeenCalled();
  });
});
