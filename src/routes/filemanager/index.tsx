import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle, CardHeader } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import {
  fileList,
  fileImport,
  fileExport,
  fileDelete,
  fileRename,
  folderCreate,
  fileFavorite,
  fileFavorites,
} from '@/lib/ipc';
import { onFilesDropped } from '@/lib/dragDrop';
import { formatBytes, extOf } from '@/lib/format';
import type { FileInfo, FolderInfo } from '@/lib/types';

interface FileManagerProps {
  token: string | null;
  onLock: () => void;
}

const VIEWER_BY_EXT: Record<string, 'pdf' | 'video' | 'image'> = {};
['pdf'].forEach((e) => (VIEWER_BY_EXT[e] = 'pdf'));
['mp4', 'mkv', 'webm', 'mov'].forEach((e) => (VIEWER_BY_EXT[e] = 'video'));
['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp'].forEach((e) => (VIEWER_BY_EXT[e] = 'image'));

export default function FileManager({ token, onLock }: FileManagerProps) {
  const navigate = useNavigate();
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [folders, setFolders] = useState<FolderInfo[]>([]);
  // Breadcrumb stack; last entry is the current folder (id null = root).
  const [crumbs, setCrumbs] = useState<{ id: number | null; name: string }[]>([
    { id: null, name: 'All Files' },
  ]);
  const [search, setSearch] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Click-select: row selected for keyboard actions (Space opens, Delete deletes).
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [favIds, setFavIds] = useState<Set<number>>(new Set());
  const searchRef = useRef<HTMLInputElement>(null);

  const currentFolder = crumbs[crumbs.length - 1].id;

  async function load(folderId: number | null) {
    const result = await fileList(folderId);
    if (result.ok) {
      setFiles(result.data.files);
      setFolders(result.data.folders);
      setError(null);
    } else {
      setError((result.error as any)?.message || 'Failed to list files');
    }
    const favs = await fileFavorites();
    if (favs.ok) setFavIds(new Set(favs.data.map((f) => f.id)));
  }

  useEffect(() => {
    load(currentFolder);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentFolder]);

  async function importPaths(paths: string[]) {
    setBusy(true);
    setMessage(null);
    const result = await fileImport(paths, currentFolder);
    setBusy(false);
    if (result.ok) {
      setMessage(`Imported ${result.data.length} file${result.data.length === 1 ? '' : 's'}`);
      await load(currentFolder);
    } else {
      setError((result.error as any)?.message || 'Import failed');
    }
  }

  async function handleImport() {
    setError(null);
    const picked = await openDialog({ multiple: true });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    await importPaths(paths);
  }

  // Drag files from Explorer anywhere onto the window.
  useEffect(() => {
    const off = onFilesDropped((paths) => importPaths(paths));
    return off;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentFolder]);

  // App-level shortcuts dispatch these events (Ctrl+O / Ctrl+F).
  useEffect(() => {
    const doImport = () => handleImport();
    const focusSearch = () => searchRef.current?.focus();
    window.addEventListener('veylora:import', doImport);
    window.addEventListener('veylora:focus-search', focusSearch);
    return () => {
      window.removeEventListener('veylora:import', doImport);
      window.removeEventListener('veylora:focus-search', focusSearch);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentFolder]);

  // Space = open selected, Delete = delete selected (when a row is selected
  // and the user is not typing).
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (selectedId == null) return;
      const target = e.target as HTMLElement | null;
      if (target && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName)) return;
      const file = files.find((f) => f.id === selectedId);
      if (!file) return;
      if (e.key === ' ') {
        e.preventDefault();
        openFile(file);
      } else if (e.key === 'Delete') {
        e.preventDefault();
        handleDelete(file);
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId, files]);

  async function toggleFavorite(file: FileInfo) {
    const isFav = favIds.has(file.id);
    const result = await fileFavorite(file.id, !isFav);
    if (result.ok) {
      setFavIds((prev) => {
        const next = new Set(prev);
        if (isFav) next.delete(file.id);
        else next.add(file.id);
        return next;
      });
    } else {
      setError((result.error as any)?.message || 'Failed to update favorite');
    }
  }

  function openFile(file: FileInfo) {
    if (!token) return;
    const kind = VIEWER_BY_EXT[extOf(file.filename)];
    if (!kind) return;
    navigate(`/viewer/${kind}/${file.file_id}`);
  }

  async function handleExport(file: FileInfo) {
    const confirmed = window.confirm(
      'Exporting will write an unencrypted copy to disk. Anyone with access to that disk can read it. Continue?',
    );
    if (!confirmed) return;
    const dest = await saveDialog({ defaultPath: file.filename });
    if (!dest) return;
    const result = await fileExport(file.id, dest);
    if (result.ok) {
      setError(null);
      setMessage(`Exported to ${dest} (unencrypted copy)`);
    } else {
      setError((result.error as any)?.message || 'Export failed');
    }
  }

  async function handleRename(file: FileInfo) {
    const next = window.prompt('New filename', file.filename);
    if (!next || next.trim() === file.filename) return;
    const result = await fileRename(file.id, next.trim());
    if (result.ok) {
      await load(currentFolder);
    } else {
      setError((result.error as any)?.message || 'Rename failed');
    }
  }

  async function handleDelete(file: FileInfo) {
    const confirmed = window.confirm(
      `Delete "${file.filename}" permanently? This cannot be undone.`,
    );
    if (!confirmed) return;
    const result = await fileDelete(file.id);
    if (result.ok) {
      await load(currentFolder);
    } else {
      setError((result.error as any)?.message || 'Delete failed');
    }
  }

  async function handleNewFolder() {
    const name = window.prompt('Folder name');
    if (!name || !name.trim()) return;
    const result = await folderCreate(name.trim(), currentFolder);
    if (result.ok) {
      await load(currentFolder);
    } else {
      setError((result.error as any)?.message || 'Failed to create folder');
    }
  }

  function enterFolder(folder: FolderInfo) {
    setCrumbs((c) => [...c, { id: folder.id, name: folder.name }]);
  }

  function crumbTo(index: number) {
    setCrumbs((c) => c.slice(0, index + 1));
  }

  const filteredFiles = files.filter((f) =>
    f.filename.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="min-h-screen bg-slate-950 p-8">
      <div className="max-w-7xl mx-auto">
        <div className="flex items-center justify-between mb-4">
          <h1 className="text-2xl font-bold text-white">File Manager</h1>
          <div className="flex gap-4">
            <Input
              placeholder="Search files..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-64 bg-slate-800"
              ref={searchRef}
            />
            <Button onClick={handleImport} disabled={busy} data-testid="import-button">
              {busy ? 'Importing...' : 'Import'}
            </Button>
            <Button variant="outline" onClick={handleNewFolder}>
              New Folder
            </Button>
            <Button variant="destructive" onClick={onLock}>
              Lock Vault
            </Button>
          </div>
        </div>

        {/* Breadcrumb */}
        <div className="flex items-center gap-2 mb-4 text-sm text-slate-400">
          {crumbs.map((crumb, i) => (
            <span key={`${crumb.id}-${i}`} className="flex items-center gap-2">
              {i > 0 && <span>/</span>}
              <button
                className={
                  i === crumbs.length - 1 ? 'text-white font-medium' : 'hover:text-white'
                }
                onClick={() => crumbTo(i)}
              >
                {crumb.name}
              </button>
            </span>
          ))}
        </div>

        {error && (
          <div className="mb-4 text-sm text-destructive-foreground bg-destructive/10 p-3 rounded">
            {error}
          </div>
        )}
        {message && (
          <div className="mb-4 text-sm text-green-300 bg-green-500/10 p-3 rounded">{message}</div>
        )}

        {/* Drop hint — drops are handled window-wide via Tauri's drag event */}
        <div className="mb-6 border border-dashed border-slate-700 rounded-lg p-4 text-center text-sm text-slate-500">
          Drag files here from Windows Explorer to import
        </div>

        <Card>
          <CardHeader>
            <CardTitle>
              Files ({filteredFiles.length}) · Folders ({folders.length})
            </CardTitle>
          </CardHeader>
          <CardContent>
            {folders.length > 0 && (
              <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-3 mb-6">
                {folders.map((folder) => (
                  <button
                    key={folder.id}
                    onClick={() => enterFolder(folder)}
                    className="text-left bg-slate-800/60 border border-slate-700 rounded-lg p-3 hover:bg-slate-800 transition-colors"
                  >
                    <div className="text-xl">📁</div>
                    <div className="text-sm text-slate-200 truncate">{folder.name}</div>
                  </button>
                ))}
              </div>
            )}

            <div className="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-4 gap-4">
              {filteredFiles.map((file) => (
                <div
                  key={file.id}
                  className={`bg-slate-900 border rounded-lg p-4 cursor-pointer ${
                    selectedId === file.id
                      ? 'border-primary ring-1 ring-primary'
                      : 'border-slate-800'
                  }`}
                  onClick={() => setSelectedId(file.id)}
                  onDoubleClick={() => openFile(file)}
                  data-testid={`file-row-${file.id}`}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="font-bold text-slate-100 break-all">{file.filename}</div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleFavorite(file);
                      }}
                      aria-label={favIds.has(file.id) ? 'Unstar file' : 'Star file'}
                      title={favIds.has(file.id) ? 'Remove from favorites' : 'Add to favorites'}
                      className={`text-lg leading-none shrink-0 ${
                        favIds.has(file.id) ? 'text-yellow-400' : 'text-slate-600 hover:text-yellow-400'
                      }`}
                    >
                      ★
                    </button>
                  </div>
                  <div className="text-sm text-slate-400">{formatBytes(file.encrypted_size)}</div>
                  <div className="text-xs text-slate-500 mt-1">
                    Imported {new Date(Number(file.created_at) * 1000).toLocaleString()}
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    {token && VIEWER_BY_EXT[extOf(file.filename)] && (
                      <Button size="sm" variant="outline" onClick={() => openFile(file)}>
                        Open
                      </Button>
                    )}
                    <Button size="sm" variant="outline" onClick={() => handleExport(file)}>
                      Export
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => handleRename(file)}>
                      Rename
                    </Button>
                    <Button size="sm" variant="destructive" onClick={() => handleDelete(file)}>
                      Delete
                    </Button>
                  </div>
                </div>
              ))}
              {filteredFiles.length === 0 && (
                <div className="text-slate-400 text-center py-8 col-span-full">
                  No files in this folder
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
