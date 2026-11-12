import { useState, useEffect, type ReactNode } from 'react';
import { useNavigate } from 'react-router-dom';
import { FileText, HardDrive, History, Upload } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { vaultStatus, fileImport, fileRecent, fileFavorites, vaultBackupHistory } from '@/lib/ipc';
import { onFilesDropped } from '@/lib/dragDrop';
import { formatBytes } from '@/lib/format';
import type { FileInfo } from '@/lib/types';

export default function Dashboard() {
  const navigate = useNavigate();
  const [status, setStatus] = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [recent, setRecent] = useState<FileInfo[]>([]);
  const [favorites, setFavorites] = useState<FileInfo[]>([]);
  // Last manual backup, from backup_history (newest first); '—' = never.
  const [lastBackup, setLastBackup] = useState<string>('—');

  useEffect(() => {
    loadStatus();
    fileRecent(5).then((r) => {
      if (r.ok) setRecent(r.data);
    });
    fileFavorites().then((r) => {
      if (r.ok) setFavorites(r.data);
    });
    vaultBackupHistory().then((r) => {
      if (r.ok && r.data.length > 0) {
        setLastBackup(new Date(r.data[0].created_at_unix * 1000).toLocaleString());
      }
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function loadStatus() {
    const result = await vaultStatus();
    if (result.ok) setStatus(result.data);
    setLoading(false);
  }

  async function importPaths(paths: string[]) {
    setBusy(true);
    setMessage(null);
    setError(null);
    const result = await fileImport(paths, null);
    setBusy(false);
    if (result.ok) {
      setMessage(`Imported ${result.data.length} file${result.data.length === 1 ? '' : 's'}`);
      await loadStatus();
    } else {
      setError((result.error as any)?.message || 'Import failed');
    }
  }

  async function handleImport() {
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
  }, []);

  function statCard(icon: ReactNode, label: string, value: string, note: string) {
    return (
      <Card>
        <CardHeader>
          <span className="flex h-9 w-9 items-center justify-center rounded-input bg-accent/15 text-accent">
            {icon}
          </span>
          <span className="text-caption text-muted-foreground">{label}</span>
        </CardHeader>
        <CardContent>
          <p className="text-title-2 text-foreground">{value}</p>
          <p className="mt-1 text-caption text-muted-foreground">{note}</p>
        </CardContent>
      </Card>
    );
  }

  function fileRows(files: FileInfo[]) {
    return files.map((file) => (
      <div
        key={file.id}
        className="flex items-center gap-3 rounded-input px-3 py-2 transition-colors hover:bg-elevated"
      >
        <FileText size={16} className="shrink-0 text-muted" />
        <span className="truncate text-sm text-foreground">{file.filename}</span>
        <span className="ml-auto shrink-0 text-caption text-muted-foreground">
          {formatBytes(file.encrypted_size)}
        </span>
      </div>
    ));
  }

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="max-w-7xl mx-auto">
        <div className="mb-8 flex items-center justify-between">
          <h1 className="text-title-1">Dashboard</h1>
          <Button variant="outline" onClick={() => loadStatus()}>
            Refresh
          </Button>
        </div>

        {error && (
          <div className="mb-4 text-sm text-danger bg-danger/10 p-3 rounded-input">
            {error}
          </div>
        )}
        {message && (
          <div className="mb-4 text-sm text-success bg-success/10 p-3 rounded-input">{message}</div>
        )}

        {loading ? (
          <div className="text-muted">Loading...</div>
        ) : (
          <>
            {/* Stat cards (Phase 2 brief): counts from vault_status, last
                backup from vault_backup_history. No welcome copy — the
                numbers are the headline. */}
            <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
              {statCard(
                <FileText size={18} />,
                'Files',
                `${status?.file_count ?? 0} files encrypted`,
                status?.locked ? 'Vault locked' : 'Vault unlocked',
              )}
              {statCard(
                <HardDrive size={18} />,
                'Storage',
                `Storage: ${formatBytes(status?.storage_used ?? 0)}`,
                'Encrypted at rest',
              )}
              {statCard(
                <History size={18} />,
                'Last backup',
                lastBackup,
                'Manual backups only',
              )}
            </div>

            <Card className="mt-6">
              <CardHeader>
                <CardTitle>Quick Import</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex flex-col gap-4 md:flex-row">
                  <div className="flex flex-1 flex-col items-center justify-center gap-2 rounded-input border border-dashed border-border py-10 text-muted-foreground">
                    <Upload size={20} />
                    <span className="text-sm">Drag files here to import</span>
                  </div>
                  <div className="flex flex-col gap-2 md:w-52">
                    <Button onClick={handleImport} disabled={busy}>
                      {busy ? 'Importing...' : 'Import Files'}
                    </Button>
                    <Button variant="outline" onClick={() => navigate('/filemanager')}>
                      Browse Files
                    </Button>
                  </div>
                </div>
              </CardContent>
            </Card>
          </>
        )}

        {!loading && (
          <div className="mt-6 grid grid-cols-1 gap-6 md:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>Recent</CardTitle>
              </CardHeader>
              <CardContent className="space-y-1">
                {recent.length === 0 && (
                  <p className="text-muted-foreground text-sm">Nothing imported yet</p>
                )}
                {fileRows(recent)}
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>Favorites</CardTitle>
              </CardHeader>
              <CardContent className="space-y-1">
                {favorites.length === 0 && (
                  <p className="text-muted-foreground text-sm">No starred files yet</p>
                )}
                {fileRows(favorites)}
              </CardContent>
            </Card>
          </div>
        )}
      </div>
    </div>
  );
}
