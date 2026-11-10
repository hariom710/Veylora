import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import Favorites from './index';

vi.mock('@/lib/ipc', () => ({
  fileFavorites: vi.fn(),
  fileFavorite: vi.fn(),
  fileExport: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

import { fileFavorites, fileFavorite, fileExport } from '@/lib/ipc';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';

const FIXTURE = [
  {
    id: 1,
    filename: 'starred.pdf',
    file_id: 'a'.repeat(32),
    folder_id: null,
    encrypted_size: 2048,
    created_at: '1700000000',
    modified_at: '1700000000',
  },
];

describe('Favorites (Slice C)', () => {
  const user = userEvent.setup();

  beforeEach(() => {
    vi.resetAllMocks();
    fileFavorites.mockResolvedValue({ ok: true, data: FIXTURE });
    fileFavorite.mockResolvedValue({ ok: true, data: undefined });
    fileExport.mockResolvedValue({ ok: true, data: 2048 });
    saveDialog.mockResolvedValue('C:\\Users\\me\\Desktop\\starred.pdf');
  });

  it('lists starred files', async () => {
    render(
      <MemoryRouter>
        <Favorites token="tok" />
      </MemoryRouter>,
    );
    expect(await screen.findByText('starred.pdf')).toBeInTheDocument();
    expect(fileFavorites).toHaveBeenCalled();
  });

  it('unstars a file and refreshes the list', async () => {
    fileFavorites.mockResolvedValueOnce({ ok: true, data: FIXTURE });
    render(
      <MemoryRouter>
        <Favorites token="tok" />
      </MemoryRouter>,
    );
    await screen.findByText('starred.pdf');

    // After unstar, the refreshed list comes back empty.
    fileFavorites.mockResolvedValue({ ok: true, data: [] });
    await user.click(screen.getByRole('button', { name: 'Unstar' }));

    expect(fileFavorite).toHaveBeenCalledWith(1, false);
    await waitFor(() => expect(screen.queryByText('starred.pdf')).not.toBeInTheDocument());
  });

  it('warns about the plaintext copy before exporting', async () => {
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    render(
      <MemoryRouter>
        <Favorites token="tok" />
      </MemoryRouter>,
    );
    await screen.findByText('starred.pdf');

    await user.click(screen.getByRole('button', { name: 'Export' }));

    expect(confirmSpy).toHaveBeenCalledWith(expect.stringMatching(/unencrypted/i));
    await waitFor(() =>
      expect(fileExport).toHaveBeenCalledWith(1, 'C:\\Users\\me\\Desktop\\starred.pdf'),
    );
    expect(await screen.findByText(/unencrypted copy/i)).toBeInTheDocument();
    confirmSpy.mockRestore();
  });
});
