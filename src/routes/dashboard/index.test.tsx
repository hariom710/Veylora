import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import Dashboard from './index';

vi.mock('@/lib/ipc', () => ({
  vaultStatus: vi.fn(),
  fileImport: vi.fn(),
  fileRecent: vi.fn(),
  fileFavorites: vi.fn(),
  vaultBackupHistory: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

import { vaultStatus, fileRecent, fileFavorites, vaultBackupHistory } from '@/lib/ipc';

describe('Dashboard', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(vaultStatus).mockResolvedValue({
      ok: true,
      data: { locked: false, vault_id: 1, file_count: 1, storage_used: 492, vault_open: true },
    });
    vi.mocked(fileRecent).mockResolvedValue({ ok: true, data: [] });
    vi.mocked(fileFavorites).mockResolvedValue({ ok: true, data: [] });
    vi.mocked(vaultBackupHistory).mockResolvedValue({ ok: true, data: [] });
  });

  // The release smoke matches these two strings on the rendered page —
  // they must stay exact and in a single text node.
  it('keeps the smoke strings intact', async () => {
    render(
      <MemoryRouter>
        <Dashboard />
      </MemoryRouter>,
    );
    expect(await screen.findByText('1 files encrypted')).toBeInTheDocument();
    expect(screen.getByText('Storage: 492 B')).toBeInTheDocument();
  });

  it('lists recent files with their size', async () => {
    vi.mocked(fileRecent).mockResolvedValue({
      ok: true,
      data: [
        {
          id: 1,
          filename: 'doc.pdf',
          file_id: 'a'.repeat(32),
          folder_id: null,
          encrypted_size: 492,
          created_at: '1700000000',
          modified_at: '1700000000',
        },
      ],
    });
    render(
      <MemoryRouter>
        <Dashboard />
      </MemoryRouter>,
    );
    expect(await screen.findByText('doc.pdf')).toBeInTheDocument();
    expect(screen.getByText('492 B')).toBeInTheDocument();
  });

  it('shows the empty states when nothing is imported', async () => {
    render(
      <MemoryRouter>
        <Dashboard />
      </MemoryRouter>,
    );
    expect(await screen.findByText('Nothing imported yet')).toBeInTheDocument();
    expect(screen.getByText('No starred files yet')).toBeInTheDocument();
  });
});
