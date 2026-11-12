import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
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
  folderRename: vi.fn(),
  folderList: vi.fn(),
  fileMove: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

import {
  fileList,
  fileImport,
  fileDelete,
  fileRename,
  fileFavorites,
  fileMove,
  folderList,
  folderRename,
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
    vi.mocked(fileList).mockResolvedValue({
      ok: true,
      data: { files: FIXTURE_FILES, folders: [] },
    });
    vi.mocked(fileFavorites).mockResolvedValue({ ok: true, data: [] });
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

    vi.mocked(openDialog).mockResolvedValue(['/tmp/p1.pdf', '/tmp/v1.mp4']);
    vi.mocked(fileImport).mockResolvedValue({ ok: true, data: [{ id: 4 }, { id: 5 }] } as any);

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

    vi.mocked(fileDelete).mockResolvedValue({ ok: true, data: undefined });
    await user.click(screen.getAllByRole('button', { name: /Delete/i })[0]);

    expect(fileDelete).toHaveBeenCalledWith(1);
    confirmSpy.mockRestore();
  });

  it('renames through the right-click context menu', async () => {
    const user = userEvent.setup();
    const promptSpy = vi.spyOn(window, 'prompt').mockReturnValue('new-name.pdf');
    vi.mocked(fileRename).mockResolvedValue({ ok: true, data: undefined });
    renderManager();
    await screen.findByText(/document\.pdf/i);

    fireEvent.contextMenu(screen.getByTestId('file-row-1'), { clientX: 10, clientY: 20 });
    await user.click(await screen.findByRole('menuitem', { name: 'Rename…' }));

    expect(fileRename).toHaveBeenCalledWith(1, 'new-name.pdf');
    promptSpy.mockRestore();
  });

  it('renames a folder through the right-click context menu', async () => {
    const user = userEvent.setup();
    vi.mocked(fileList).mockResolvedValue({
      ok: true,
      data: {
        files: [],
        folders: [{ id: 7, parent_id: null, name: 'Docs', created_at: '0' }],
      },
    });
    const promptSpy = vi.spyOn(window, 'prompt').mockReturnValue('Renamed');
    vi.mocked(folderRename).mockResolvedValue({ ok: true, data: undefined });
    renderManager();
    await screen.findByText('Docs');

    fireEvent.contextMenu(screen.getByRole('button', { name: /Docs/ }), { clientX: 10, clientY: 20 });
    await user.click(await screen.findByRole('menuitem', { name: 'Rename folder…' }));

    expect(folderRename).toHaveBeenCalledWith(7, 'Renamed');
    promptSpy.mockRestore();
  });

  it('moves a file into a folder through the context menu', async () => {
    const user = userEvent.setup();
    vi.mocked(folderList).mockResolvedValue({
      ok: true,
      data: [{ id: 7, parent_id: null, name: 'Docs', created_at: '0' }],
    });
    vi.mocked(fileMove).mockResolvedValue({ ok: true, data: undefined });
    renderManager();
    await screen.findByText(/document\.pdf/i);

    fireEvent.contextMenu(screen.getByTestId('file-row-1'), { clientX: 10, clientY: 20 });
    await user.click(await screen.findByRole('menuitem', { name: 'Move to folder…' }));
    await user.click(await screen.findByRole('button', { name: 'Docs' }));

    expect(fileMove).toHaveBeenCalledWith(1, 7);
    expect(await screen.findByText(/Moved "document\.pdf"/)).toBeInTheDocument();
  });

  it('batch-deletes the selected files after one confirmation', async () => {
    const user = userEvent.setup();
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true);
    renderManager();
    await screen.findByText(/document\.pdf/i);

    await user.click(screen.getByRole('button', { name: 'Select' }));
    await user.click(screen.getByLabelText('Select document.pdf'));
    await user.click(screen.getByLabelText('Select video.mp4'));
    expect(screen.getByTestId('selected-count').textContent).toBe('2 selected');

    vi.mocked(fileDelete).mockResolvedValue({ ok: true, data: undefined });
    await user.click(screen.getByRole('button', { name: 'Delete selected' }));

    expect(fileDelete).toHaveBeenCalledTimes(2);
    expect(fileDelete).toHaveBeenCalledWith(1);
    expect(fileDelete).toHaveBeenCalledWith(2);
    confirmSpy.mockRestore();
  });

  it('context menu supports arrow keys and closes on Escape', async () => {
    const user = userEvent.setup();
    renderManager();
    await screen.findByText(/document\.pdf/i);

    fireEvent.contextMenu(screen.getByTestId('file-row-1'), { clientX: 10, clientY: 20 });
    await screen.findByRole('menu');
    expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Rename…' }));

    await user.keyboard('{ArrowDown}');
    expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Export…' }));
    await user.keyboard('{ArrowUp}');
    expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Rename…' }));

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('shows a search-specific empty state instead of the folder message', async () => {
    const user = userEvent.setup();
    renderManager();
    await screen.findByText(/document\.pdf/i);

    await user.type(screen.getByPlaceholderText(/Search files/i), 'zzz');
    expect(screen.getByText(/No files match “zzz”/)).toBeInTheDocument();
    expect(screen.queryByText(/No files in this folder/)).not.toBeInTheDocument();
  });
});
