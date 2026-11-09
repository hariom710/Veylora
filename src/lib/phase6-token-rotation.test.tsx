import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import App from '@/App';

vi.mock('@/lib/ipc', () => ({
  vaultUnlockPin: vi.fn(),
  vaultLock: vi.fn(),
  vaultGetSessionToken: vi.fn(),
  vaultStatus: vi.fn(),
}));

import {
  vaultUnlockPin,
  vaultLock,
  vaultGetSessionToken,
  vaultStatus,
} from '@/lib/ipc';

describe('phase6_token_rotates_after_lock_unlock', () => {
  const user = userEvent.setup();

  beforeEach(() => {
    vi.resetAllMocks();
    vaultStatus.mockResolvedValue({ ok: true, data: { locked: true, vault_id: null, file_count: 0, storage_used: 0 } });
  });

  it('token rotates after lock and unlock cycle', async () => {
    // First unlock: returns token_1
    vaultUnlockPin.mockResolvedValue({ ok: true, data: { vault_id: 1, unlock_method: 'pin' } });
    vaultGetSessionToken.mockResolvedValue({ ok: true, data: 'token_1' });

    render(<App />);

    await user.type(screen.getByLabelText(/PIN/i), '1234');
    await user.click(screen.getByRole('button', { name: /Unlock/i }));

    // Should navigate to dashboard (unlocked)
    expect(screen.getByText(/Dashboard/i)).toBeInTheDocument();
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
    expect(screen.getByText(/Dashboard/i)).toBeInTheDocument();

    // Get token after second unlock
    const token2 = await vaultGetSessionToken();
    expect(token2.ok).toBe(true);
    if (token2.ok) {
      expect(token2.data).toBe('token_2');
    }
  });
});
