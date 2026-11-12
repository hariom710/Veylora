import { useState, useEffect, useRef, type ReactNode } from 'react';
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
  folderRename,
  folderList,
  fileMove,
  fileFavorite,
  fileFavorites,
} from '@/lib/ipc';
import { onFilesDropped } from '@/lib/dragDrop';
import { formatBytes, extOf } from '@/lib/format';
import { sortFiles, type SortKey } from '@/lib/sort';
import type { FileInfo, FolderInfo } from '@/lib/types';

interface FileManagerProps {
  token: string | null;
  onLock: () => void;
}

const VIEWER_BY_EXT: Record<string, 'pdf' | 'video' | 'image'> = {};
['pdf'].forEach((e) => (VIEWER_BY_EXT[e] = 'pdf'));
['mp4', 'mkv', 'webm', 'mov'].forEach((e) => (VIEWER_BY_EXT[e] = 'video'));
['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp'].forEach((e) => (VIEWER_BY_EXT[e] = 'image'));

// One item in the right-click menu.
function MenuItem({
  onClick,
  danger,
  children,
}: {
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onClick}
      className={`block w-full text-left px-3 py-1.5 text-sm transition-colors ${
        danger ? 'text-danger hover:bg-danger/10' : 'text-foreground hover:bg-white/5'
      }`}
    >
      {children}
    </button>
  );
}

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
  // Sort order for the card grid (name = A→Z, date/size = largest first).
  const [sort, setSort] = useState<SortKey>('name');
  // Batch select mode: multi-selection for export/delete (item 8).
  const [selectMode, setSelectMode] = useState(false);
  const [checked, setChecked] = useState<Set<number>>(new Set());
  // Right-click menu: either a file or a folder target (item 2).
  const [menu, setMenu] = useState<
    | { x: number; y: number; file: FileInfo; folder?: undefined }
    | { x: number; y: number; folder: FolderInfo; file?: undefined }
    | null
  >(null);
  // Move-to-folder dialog target (item 7).
  const [moveTarget, setMoveTarget] = useState<FileInfo | null>(null);
  const [moveFolders, setMoveFolders] = useState<FolderInfo[]>([]);
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

  async function handleFolderRename(folder: FolderInfo) {
    const next = window.prompt('Folder name', folder.name);
    if (!next || next.trim() === folder.name) return;
    const result = await folderRename(folder.id, next.trim());
    if (result.ok) {
      await load(currentFolder);
    } else {
      setError((result.error as any)?.message || 'Rename failed');
    }
  }

  // Move dialog offers the root plus top-level folders (one level deep —
  // shortcut: upgrade to a tree picker when nested moves matter).
  async function openMoveDialog(file: FileInfo) {
    setMoveTarget(file);
    const result = await folderList(null);
    if (result.ok) setMoveFolders(result.data);
  }

  async function doMove(folderId: number | null) {
    if (!moveTarget) return;
    const result = await fileMove(moveTarget.id, folderId);
    setMoveTarget(null);
    if (result.ok) {
      setMessage(`Moved "${moveTarget.filename}"`);
      await load(currentFolder);
    } else {
      setError((result.error as any)?.message || 'Move failed');
    }
  }

  function toggleCheck(id: number) {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function toggleSelectMode() {
    setSelectMode((v) => !v);
    setChecked(new Set());
  }

  async function batchDelete() {
    const ids = [...checked];
    const n = ids.length;
    if (n === 0) return;
    const confirmed = window.confirm(
      `Delete ${n} file${n === 1 ? '' : 's'} permanently? This cannot be undone.`,
    );
    if (!confirmed) return;
    setBusy(true);
    let failed = 0;
    for (const id of ids) {
      const result = await fileDelete(id);
      if (!result.ok) failed++;
    }
    setBusy(false);
    setChecked(new Set());
    setSelectMode(false);
    await load(currentFolder);
    if (failed > 0) setError(`Failed to delete ${failed} of ${n} file${n === 1 ? '' : 's'}`);
    else setMessage(`Deleted ${n} file${n === 1 ? '' : 's'}`);
  }

  async function batchExport() {
    const targets = files.filter((f) => checked.has(f.id));
    if (targets.length === 0) return;
    const confirmed = window.confirm(
      'Exporting will write unencrypted copies to disk. Anyone with access to that disk can read them. Continue?',
    );
    if (!confirmed) return;
    setBusy(true);
    let exported = 0;
    let failed = 0;
    for (const file of targets) {
      const dest = await saveDialog({ defaultPath: file.filename });
      // Cancelling one save dialog aborts the rest of the batch.
      if (!dest) break;
      const result = await fileExport(file.id, dest);
      if (result.ok) exported++;
      else failed++;
    }
    setBusy(false);
    setChecked(new Set());
    setSelectMode(false);
    if (failed > 0) setError(`Exported ${exported}, failed ${failed}`);
    else if (exported > 0) setMessage(`Exported ${exported} file${exported === 1 ? '' : 's'}`);
  }

  const filteredFiles = sortFiles(
    files.filter((f) => f.filename.toLowerCase().includes(search.toLowerCase())),
    sort,
  );

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="max-w-7xl mx-auto">
        <div className="flex items-center justify-between mb-4">
          <h1 className="text-title-1">File Manager</h1>
          <div className="flex gap-4 items-center">
            <Input
              placeholder="Search files..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-64 bg-elevated"
              ref={searchRef}
            />
            <select
              aria-label="Sort files"
              value={sort}
              onChange={(e) => setSort(e.target.value as SortKey)}
              className="h-10 rounded-button border border-border bg-surface px-3 text-sm text-foreground"
            >
              <option value="name">Name</option>
              <option value="date">Date added</option>
              <option value="size">Size</option>
            </select>
            <Button onClick={handleImport} disabled={busy} data-testid="import-button">
              {busy ? 'Importing...' : 'Import'}
            </Button>
            <Button variant="outline" onClick={handleNewFolder}>
              New Folder
            </Button>
            <Button variant="outline" onClick={toggleSelectMode}>
              {selectMode ? 'Cancel selection' : 'Select'}
            </Button>
            <Button variant="destructive" onClick={onLock}>
              Lock Vault
            </Button>
          </div>
        </div>

        {/* Batch actions — only in select mode, so single-item buttons keep
            unambiguous accessible names for tests and keyboard users. */}
        {selectMode && (
          <div className="flex items-center gap-3 mb-4 text-sm">
            <span className="text-muted" data-testid="selected-count">
              {checked.size} selected
            </span>
            <Button
              size="sm"
              variant="outline"
              disabled={checked.size === 0 || busy}
              onClick={batchExport}
            >
              Export selected
            </Button>
            <Button
              size="sm"
              variant="destructive"
              disabled={checked.size === 0 || busy}
              onClick={batchDelete}
            >
              Delete selected
            </Button>
          </div>
        )}

        {/* Breadcrumb */}
        <div className="flex items-center gap-2 mb-4 text-sm text-muted">
          {crumbs.map((crumb, i) => (
            <span key={`${crumb.id}-${i}`} className="flex items-center gap-2">
              {i > 0 && <span>/</span>}
              <button
                className={
                  i === crumbs.length - 1 ? 'text-foreground font-medium' : 'hover:text-foreground'
                }
                onClick={() => crumbTo(i)}
              >
                {crumb.name}
              </button>
            </span>
          ))}
        </div>

        {error && (
          <div className="mb-4 text-sm text-danger bg-danger/10 p-3 rounded-input">
            {error}
          </div>
        )}
        {message && (
          <div className="mb-4 text-sm text-success bg-success/10 p-3 rounded-input">{message}</div>
        )}

        {/* Drop hint — drops are handled window-wide via Tauri's drag event */}
        <div className="mb-6 border border-dashed border-border rounded-card p-4 text-center text-sm text-muted-foreground">
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
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setMenu({ x: e.clientX, y: e.clientY, folder });
                    }}
                    className="text-left bg-elevated/60 border border-border rounded-card p-3 hover:bg-elevated transition-colors"
                  >
                    <div className="text-xl">📁</div>
                    <div className="text-sm text-foreground truncate">{folder.name}</div>
                  </button>
                ))}
              </div>
            )}

            {/* Fixed 160×200 file cards (Phase 2 brief). */}
            <div className="grid gap-4 [grid-template-columns:repeat(auto-fill,160px)]">
              {filteredFiles.map((file) => (
                <div
                  key={file.id}
                  className={`bg-surface border rounded-card p-3 cursor-pointer w-[160px] h-[200px] flex flex-col overflow-hidden ${
                    selectedId === file.id || (selectMode && checked.has(file.id))
                      ? 'border-accent ring-1 ring-accent'
                      : 'border-border'
                  }`}
                  onClick={() => {
                    if (selectMode) toggleCheck(file.id);
                    else setSelectedId(file.id);
                  }}
                  onDoubleClick={() => openFile(file)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    setMenu({ x: e.clientX, y: e.clientY, file });
                  }}
                  data-testid={`file-row-${file.id}`}
                >
                  <div className="flex items-start gap-1">
                    {selectMode && (
                      <input
                        type="checkbox"
                        aria-label={`Select ${file.filename}`}
                        checked={checked.has(file.id)}
                        onClick={(e) => e.stopPropagation()}
                        onChange={() => toggleCheck(file.id)}
                        className="mt-0.5 shrink-0"
                      />
                    )}
                    <div className="flex-1 min-w-0 font-semibold text-sm text-foreground break-all line-clamp-2">
                      {file.filename}
                    </div>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleFavorite(file);
                      }}
                      aria-label={favIds.has(file.id) ? 'Unstar file' : 'Star file'}
                      title={favIds.has(file.id) ? 'Remove from favorites' : 'Add to favorites'}
                      className={`text-lg leading-none shrink-0 ${
                        favIds.has(file.id) ? 'text-warning' : 'text-muted-foreground hover:text-warning'
                      }`}
                    >
                      ★
                    </button>
                  </div>
                  <div className="text-xs text-muted mt-1">{formatBytes(file.encrypted_size)}</div>
                  <div className="text-[11px] text-muted-foreground mt-1 line-clamp-1">
                    Imported {new Date(Number(file.created_at) * 1000).toLocaleDateString()}
                  </div>
                  <div className="mt-auto pt-2 grid grid-cols-2 gap-1">
                    {token && VIEWER_BY_EXT[extOf(file.filename)] && (
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-7 px-1 text-xs"
                        onClick={(e) => {
                          e.stopPropagation();
                          openFile(file);
                        }}
                      >
                        Open
                      </Button>
                    )}
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-7 px-1 text-xs"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleExport(file);
                      }}
                    >
                      Export
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      className="h-7 px-1 text-xs"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleRename(file);
                      }}
                    >
                      Rename
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      className="h-7 px-1 text-xs"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDelete(file);
                      }}
                    >
                      Delete
                    </Button>
                  </div>
                </div>
              ))}
              {filteredFiles.length === 0 && (
                <div className="text-muted text-center py-8 col-span-full">
                  {search ? `No files match “${search}”` : 'No files in this folder'}
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        {/* Right-click menu — a pointer affordance only: every action here
            also exists as a visible button, so keyboard users lose nothing. */}
        {menu && (
          <>
            <div
              className="fixed inset-0 z-40"
              onClick={() => setMenu(null)}
              onContextMenu={(e) => {
                e.preventDefault();
                setMenu(null);
              }}
            />
            <div
              role="menu"
              className="fixed z-50 min-w-44 bg-surface border border-border rounded-input py-1"
              style={{ left: menu.x, top: menu.y }}
            >
              {menu.file ? (
                <>
                  <MenuItem
                    onClick={() => {
                      const file = menu.file!;
                      setMenu(null);
                      handleRename(file);
                    }}
                  >
                    Rename…
                  </MenuItem>
                  <MenuItem
                    onClick={() => {
                      const file = menu.file!;
                      setMenu(null);
                      handleExport(file);
                    }}
                  >
                    Export…
                  </MenuItem>
                  <MenuItem
                    onClick={() => {
                      const file = menu.file!;
                      setMenu(null);
                      openMoveDialog(file);
                    }}
                  >
                    Move to folder…
                  </MenuItem>
                  <MenuItem
                    onClick={() => {
                      const file = menu.file!;
                      setMenu(null);
                      toggleFavorite(file);
                    }}
                  >
                    {favIds.has(menu.file.id) ? 'Remove favorite' : 'Add to favorites'}
                  </MenuItem>
                  <MenuItem
                    danger
                    onClick={() => {
                      const file = menu.file!;
                      setMenu(null);
                      handleDelete(file);
                    }}
                  >
                    Delete
                  </MenuItem>
                </>
              ) : (
                <MenuItem
                  onClick={() => {
                    const folder = menu.folder!;
                    setMenu(null);
                    handleFolderRename(folder);
                  }}
                >
                  Rename folder…
                </MenuItem>
              )}
            </div>
          </>
        )}

        {/* Move-to-folder picker (root + top-level folders — see openMoveDialog). */}
        {moveTarget && (
          <div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center">
            <div
              role="dialog"
              aria-label={`Move ${moveTarget.filename}`}
              className="w-80 bg-surface border border-border rounded-card p-6"
            >
              <h2 className="text-body font-semibold text-foreground mb-3">
                Move “{moveTarget.filename}” to…
              </h2>
              <div className="space-y-1 max-h-64 overflow-y-auto">
                <button
                  type="button"
                  className="w-full text-left px-3 py-2 rounded-input text-sm text-foreground hover:bg-white/5"
                  onClick={() => doMove(null)}
                >
                  All Files (root)
                </button>
                {moveFolders.map((folder) => (
                  <button
                    key={folder.id}
                    type="button"
                    className="w-full text-left px-3 py-2 rounded-input text-sm text-foreground hover:bg-white/5"
                    onClick={() => doMove(folder.id)}
                  >
                    {folder.name}
                  </button>
                ))}
                {moveFolders.length === 0 && (
                  <p className="text-sm text-muted px-3 py-2">
                    No folders yet — root is the only destination.
                  </p>
                )}
              </div>
              <Button variant="outline" className="w-full mt-4" onClick={() => setMoveTarget(null)}>
                Cancel
              </Button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
