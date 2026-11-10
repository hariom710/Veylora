import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import FileManager from './index';

vi.mock('@/lib/ipc', () => ({
  fileList: vi.fn(),
  fileImport: vi.fn(),
  fileExport: vi.fn(),
  fileDelete: vi.fn(),
  fileRename: vi.fn(),
  fileFavorite: vi.fn(),
  fileFavorites: vi.fn(),
  folderCreate: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

import {
  fileList,
  fileImport,
  fileDelete,
  fileFavorites,
} from '@/lib/ipc';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

const FIXTURE_FILES = [
  { id: 1, filename: 'document.pdf', file_id: 'a'.repeat(32), folder_id: null, encrypted_size: 1024 * 500, created_at: '1700000000', modified_at: '1700000000' },
  { id: 2, filename: 'video.mp4', file_id: 'b'.repeat(32), folder_id: null, encrypted_size: 1024 * 1024 * 100, created_at: '1700000000', modified_at: '1700000000' },
  { id: 3, filename: 'photo.jpg', file_id: 'c'.repeat(32), folder_id: null, encrypted_size: 1024 * 2000, created_at: '1700000000', modified_at: '1700000000' },
];

function renderManager(onLock = vi.fn()) {
  render(
    <MemoryRouter>
      <FileManager token="abc123" onLock={onLock} />
    </MemoryRouter>,
  );
  return onLock;
}

describe('FileManager', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    fileList.mockResolvedValue({
      ok: true,
      data: { files: FIXTURE_FILES, folders: [] },
    });
    fileFavorites.mockResolvedValue({ ok: true, data: [] });
  });

  it('renders file list', async () => {
    renderManager();
    expect(await screen.findByText(/document\.pdf/i)).toBeInTheDocument();
    expect(screen.getByText(/video\.mp4/i)).toBeInTheDocument();
    expect(screen.getByText(/photo\.jpg/i)).toBeInTheDocument();
    expect(fileList).toHaveBeenCalledWith(null);
  });

  it('filters files by name', async () => {
    const user = userEvent.setup();
    renderManager();
    await screen.findByText(/document\.pdf/i);
    await user.type(screen.getByPlaceholderText(/Search files/i), 'video');
    expect(screen.getByText(/video\.mp4/i)).toBeInTheDocument();
    expect(screen.queryByText(/document\.pdf/i)).not.toBeInTheDocument();
  });

  it('calls onLock when Lock Vault button clicked', async () => {
    const user = userEvent.setup();
    const onLock = renderManager();
    await screen.findByText(/document\.pdf/i);
    await user.click(screen.getByRole('button', { name: /Lock Vault/i }));
    expect(onLock).toHaveBeenCalled();
  });

  it('imports files through the native picker', async () => {
    const user = userEvent.setup();
    renderManager();
    await screen.findByText(/document\.pdf/i);

    openDialog.mockResolvedValue(['/tmp/p1.pdf', '/tmp/v1.mp4']);
    fileImport.mockResolvedValue({ ok: true, data: [{ id: 4 }, { id: 5 }] });

    await user.click(screen.getByTestId('import-button'));

    expect(openDialog).toHaveBeenCalledWith({ multiple: true });
    expect(fileImport).toHaveBeenCalledWith(['/tmp/p1.pdf', '/tmp/v1.mp4'], null);
    // Listing refreshes after import.
    await waitFor(() => expect(fileList).toHaveBeenCalledTimes(2));
    expect(await screen.findByText(/Imported 2 files/i)).toBeInTheDocument();
  });

  it('deletes a file after confirmation', async () => {
    const user = userEvent.setup();
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    renderManager();
    await screen.findByText(/document\.pdf/i);

    fileDelete.mockResolvedValue({ ok: true, data: undefined });
    await user.click(screen.getAllByRole('button', { name: /Delete/i })[0]);

    expect(fileDelete).toHaveBeenCalledWith(1);
    confirmSpy.mockRestore();
  });
});
