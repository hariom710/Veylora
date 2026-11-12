import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Backup from '@/routes/backup';

vi.mock('@/lib/ipc', () => ({
  vaultBackupCreate: vi.fn(),
  vaultBackupVerify: vi.fn(),
  vaultBackupRestore: vi.fn(),
  vaultBackupHistory: vi.fn(),
}));

import {
  vaultBackupCreate,
  vaultBackupVerify,
  vaultBackupHistory,
} from '@/lib/ipc';

describe('Backup screen', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(vaultBackupHistory).mockResolvedValue({
      ok: true,
      data: [],
    });
  });

  it('renders history from mocked vault_backup_history', async () => {
    vi.mocked(vaultBackupHistory).mockResolvedValue({
      ok: true,
      data: [
        { path: '/backups/vault-2026.vey', status: 'complete', created_at_unix: 1000 },
        { path: '/backups/vault-2025.vey', status: 'failed', created_at_unix: 900 },
      ],
    });

    render(<Backup />);

    // Click refresh to load history
    await userEvent.click(screen.getByRole('button', { name: /Refresh History/i }));

    expect(screen.getByText(/vault-2026.vey — complete/)).toBeInTheDocument();
    expect(screen.getByText(/vault-2025.vey — failed/)).toBeInTheDocument();
  });

  it('calls vault_backup_create with selected path on create', async () => {
    vi.mocked(vaultBackupCreate).mockResolvedValue({
      ok: true,
      data: {
        path: '/backups/test.vey',
        file_count: 3,
        total_size: 3000,
        created_at_unix: 1000,
        verified_fields: ['blobs_hash', 'manifest'],
      },
    });

    render(<Backup />);

    const destInput = screen.getByPlaceholderText('Backup destination path');
    await userEvent.clear(destInput);
    await userEvent.type(destInput, '/backups/test.vey');

    await userEvent.click(screen.getByRole('button', { name: /Create Backup/i }));

    expect(vaultBackupCreate).toHaveBeenCalledWith('/backups/test.vey');
    expect(screen.getByText(/Backup created: 3 files/)).toBeInTheDocument();
  });

  it('shows corrupted backup error from vault_backup_verify', async () => {
    vi.mocked(vaultBackupVerify).mockResolvedValue({
      ok: false,
      error: { type: 'Internal', message: 'manifest hash mismatch' },
    });

    render(<Backup />);

    const destInput = screen.getByPlaceholderText('Backup destination path');
    await userEvent.clear(destInput);
    await userEvent.type(destInput, '/backups/corrupt.vey');

    await userEvent.click(screen.getByRole('button', { name: /Verify/i }));

    expect(screen.getByText(/Verification failed: manifest hash mismatch/)).toBeInTheDocument();
  });
});
