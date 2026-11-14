import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
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
  vaultBackupHistory: vi.fn(),
}));

import {
  appStatus,
  vaultUnlockPin,
  vaultGetSessionToken,
  vaultStatus,
  vaultActivity,
  fileRecent,
  fileFavorites,
  vaultBackupHistory,
} from '@/lib/ipc';

const flush = () => act(async () => {});

describe('auto-lock poll reroutes to the vault picker', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetAllMocks();
    window.localStorage.clear();
    vi.mocked(appStatus).mockResolvedValue({
      ok: true,
      data: { logged_in: true, email: 'a@b.co', role: 'admin', logged_in_since: 1 },
    });
    // Boot sees an open, locked vault (Unlock screen); after the user
    // unlocks, the backend auto-locks on its own: session gone AND path
    // cleared — exactly what lock() does.
    vi.mocked(vaultStatus)
      .mockResolvedValueOnce({
        ok: true,
        data: { locked: true, vault_id: null, file_count: 0, storage_used: 0, vault_open: true },
      })
      .mockResolvedValue({
        ok: true,
        data: { locked: true, vault_id: null, file_count: 0, storage_used: 0, vault_open: false },
      });
    vi.mocked(vaultUnlockPin).mockResolvedValue({ ok: true, data: { vault_id: 1, unlock_method: 'pin' } });
    vi.mocked(vaultGetSessionToken).mockResolvedValue({ ok: true, data: 'token_1' });
    vi.mocked(vaultActivity).mockResolvedValue({ ok: true, data: undefined });
    vi.mocked(fileRecent).mockResolvedValue({ ok: true, data: [] });
    vi.mocked(fileFavorites).mockResolvedValue({ ok: true, data: [] });
    vi.mocked(vaultBackupHistory).mockResolvedValue({ ok: true, data: [] });
  });

  afterEach(() => {
    vi.useRealTimers();
    window.localStorage.clear();
  });

  it('shell polls status and lands on the picker when the backend locks itself', async () => {
    render(<App />);
    await flush();

    // Cold start resolves to the Unlock screen.
    expect(screen.getByRole('button', { name: 'Unlock' })).toBeInTheDocument();

    // Unlock into the shell (Dashboard).
    fireEvent.change(screen.getByLabelText('PIN'), { target: { value: '1234' } });
    fireEvent.click(screen.getByRole('button', { name: 'Unlock' }));
    await flush();
    expect(screen.getByRole('heading', { name: /Dashboard/i })).toBeInTheDocument();

    // Next poll tick: the backend reports locked + closed (idle auto-lock /
    // Win+L happen with no event back to the UI). The gate must leave the
    // shell for the vault picker.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });

    expect(screen.getByText('Open Existing Vault')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: /Dashboard/i })).not.toBeInTheDocument();
  });
});
