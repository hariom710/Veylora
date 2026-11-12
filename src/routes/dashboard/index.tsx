import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle } from '@/components/ui/card';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { vaultStatus, fileImport, fileRecent, fileFavorites, vaultBackupHistory } from '@/lib/ipc';
import { onFilesDropped } from '@/lib/dragDrop';
import { formatBytes } from '@/lib/format';
import type { FileInfo } from '@/lib/types';

interface DashboardProps {
  sessionToken: string | null;
}

export default function Dashboard({ sessionToken }: DashboardProps) {
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
  }, []);

  async function loadStatus() {
    const { data, ok } = await vaultStatus();
    if (ok) setStatus(data);
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

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="max-w-7xl mx-auto">
        <div className="flex items-center justify-between mb-8">
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
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
              <Card>
                <CardContent>
                  <div className="text-caption text-muted-foreground">Files</div>
                  <p className="mt-1 text-title-2 text-foreground">
                    {`${status?.file_count ?? 0} files encrypted`}
                  </p>
                  <p className="text-caption text-muted-foreground mt-1">
                    {status?.locked ? 'Vault locked' : 'Vault unlocked'}
                  </p>
                </CardContent>
              </Card>
              <Card>
                <CardContent>
                  <div className="text-caption text-muted-foreground">Storage</div>
                  <p className="mt-1 text-title-2 text-foreground">
                    {`Storage: ${formatBytes(status?.storage_used ?? 0)}`}
                  </p>
                  <p className="text-caption text-muted-foreground mt-1">Encrypted at rest</p>
                </CardContent>
              </Card>
              <Card>
                <CardContent>
                  <div className="text-caption text-muted-foreground">Last backup</div>
                  <p className="mt-1 text-title-2 text-foreground">{lastBackup}</p>
                  <p className="text-caption text-muted-foreground mt-1">Manual backups only</p>
                </CardContent>
              </Card>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mt-6">
              <Card>
                <CardContent>
                  <CardTitle>Quick Import</CardTitle>
                  <Button className="w-full" onClick={handleImport} disabled={busy}>
                    {busy ? 'Importing...' : 'Import Files'}
                  </Button>
                  <Button
                    className="w-full mt-2"
                    variant="outline"
                    onClick={() => navigate('/filemanager')}
                  >
                    Browse Files
                  </Button>
                  <div className="mt-4 border border-dashed border-border rounded-card p-4 text-center text-caption text-muted-foreground">
                    Drag files here to import
                  </div>
                </CardContent>
              </Card>
              <Card>
                <CardContent>
                  <CardTitle>Session</CardTitle>
                  <p className="text-muted text-sm">
                    Vault: {status?.locked ? 'Locked' : 'Unlocked'}
                  </p>
                  <p className="text-muted text-sm mt-1">
                    Token: {sessionToken ? 'active' : 'none'}
                  </p>
                </CardContent>
              </Card>
            </div>
          </>
        )}

        {!loading && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6 mt-6">
            <Card>
              <CardContent>
                <CardTitle>Recent</CardTitle>
                {recent.length === 0 && (
                  <p className="text-muted-foreground text-sm">Nothing imported yet</p>
                )}
                {recent.map((file) => (
                  <div key={file.id} className="flex justify-between text-sm py-1">
                    <span className="text-foreground truncate mr-2">{file.filename}</span>
                    <span className="text-muted-foreground">{formatBytes(file.encrypted_size)}</span>
                  </div>
                ))}
              </CardContent>
            </Card>
            <Card>
              <CardContent>
                <CardTitle>Favorites</CardTitle>
                {favorites.length === 0 && (
                  <p className="text-muted-foreground text-sm">No starred files yet</p>
                )}
                {favorites.map((file) => (
                  <div key={file.id} className="flex justify-between text-sm py-1">
                    <span className="text-foreground truncate mr-2">{file.filename}</span>
                    <span className="text-muted-foreground">{formatBytes(file.encrypted_size)}</span>
                  </div>
                ))}
              </CardContent>
            </Card>
          </div>
        )}
      </div>
    </div>
  );
}
