import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Settings from './index';

vi.mock('@/lib/ipc', () => ({
  vaultStatus: vi.fn(),
  vaultChangePin: vi.fn(),
  vaultChangePassword: vi.fn(),
  vaultBackupHistoryClear: vi.fn(),
  vaultDelete: vi.fn(),
  autoLockSet: vi.fn(),
}));

import {
  vaultStatus,
  vaultChangePin,
  vaultChangePassword,
  vaultDelete,
  autoLockSet,
} from '@/lib/ipc';

describe('Settings (Slice C)', () => {
  const user = userEvent.setup();

  beforeEach(() => {
    vi.resetAllMocks();
    window.localStorage.clear();
    vi.mocked(vaultStatus).mockResolvedValue({
      ok: true,
      data: { locked: false, vault_id: 1, file_count: 3, storage_used: 2048, vault_open: true },
    });
    vi.mocked(autoLockSet).mockResolvedValue({ ok: true, data: undefined });
    vi.mocked(vaultChangePin).mockResolvedValue({ ok: true, data: undefined });
    vi.mocked(vaultChangePassword).mockResolvedValue({ ok: true, data: undefined });
    vi.mocked(vaultDelete).mockResolvedValue({ ok: true, data: undefined });
  });

  afterEach(() => {
    window.localStorage.clear();
  });

  it('applies the persisted auto-lock timeout on mount and on change', async () => {
    render(<Settings />);

    // Mount re-applies the saved (or default 300s) timeout to the watcher.
    await vi.waitFor(() => expect(autoLockSet).toHaveBeenCalledWith(300));

    await user.selectOptions(screen.getByLabelText(/Lock the vault after inactivity/i), '60');
    await vi.waitFor(() => expect(autoLockSet).toHaveBeenCalledWith(60));
    // Choice is persisted for the next launch.
    expect(window.localStorage.getItem('veylora.autoLockSecs')).toBe('60');
  });

  it('rejects an invalid new PIN client-side without calling the backend', async () => {
    render(<Settings />);
    await screen.findByText(/Security/i);

    await user.type(screen.getByLabelText('Current PIN'), '1234');
    await user.type(screen.getByLabelText('New PIN'), '12');
    await user.click(screen.getByRole('button', { name: 'Change PIN' }));

    expect(await screen.findByText('New PIN must be 4-6 digits')).toBeInTheDocument();
    expect(vaultChangePin).not.toHaveBeenCalled();
  });

  it('submits a valid PIN change with current and new values', async () => {
    render(<Settings />);
    await screen.findByText(/Security/i);

    await user.type(screen.getByLabelText('Current PIN'), '1234');
    await user.type(screen.getByLabelText('New PIN'), '5678');
    await user.click(screen.getByRole('button', { name: 'Change PIN' }));

    expect(await screen.findByText('PIN changed')).toBeInTheDocument();
    expect(vaultChangePin).toHaveBeenCalledWith('1234', '5678');
  });

  it('requires the exact confirmation phrase before deleting the vault', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(<Settings />);
    await user.click(screen.getByRole('button', { name: 'Danger Zone' }));

    await user.type(screen.getByLabelText('Confirm PIN'), '1234');
    await user.type(screen.getByLabelText(/DELETE MY VAULT/i), 'delete my vault');
    await user.click(screen.getByRole('button', { name: /Delete Vault Permanently/i }));

    // Case/phrase mismatch → rejected before PIN even reaches the backend.
    expect(await screen.findByText('Type DELETE MY VAULT to confirm')).toBeInTheDocument();
    expect(vaultDelete).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it('deletes the vault when PIN and confirmation phrase are correct', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(<Settings />);
    await user.click(screen.getByRole('button', { name: 'Danger Zone' }));

    await user.type(screen.getByLabelText('Confirm PIN'), '1234');
    await user.type(screen.getByLabelText(/DELETE MY VAULT/i), 'DELETE MY VAULT');
    await user.click(screen.getByRole('button', { name: /Delete Vault Permanently/i }));

    await vi.waitFor(() => expect(vaultDelete).toHaveBeenCalledWith('1234'));
    confirmSpy.mockRestore();
  });
});
