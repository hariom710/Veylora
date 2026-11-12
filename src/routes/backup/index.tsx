import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle, CardHeader } from '@/components/ui/card';
import {
  vaultBackupCreate,
  vaultBackupVerify,
  vaultBackupRestore,
  vaultBackupHistory,
} from '@/lib/ipc';
import { commandErrorMessage, type BackupInfo, type RestoreInfo, type BackupHistoryEntry } from '@/lib/types';
import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog';

export default function Backup() {
  const [backupInfo, setBackupInfo] = useState<BackupInfo | null>(null);
  const [restoreInfo, setRestoreInfo] = useState<RestoreInfo | null>(null);
  const [history, setHistory] = useState<BackupHistoryEntry[]>([]);
  const [backupDest, setBackupDest] = useState('');
  const [restoreSource, setRestoreSource] = useState('');
  const [message, setMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function loadHistory() {
    const result = await vaultBackupHistory();
    if (result.ok) {
      setHistory(result.data);
    } else {
      setMessage(`Failed to load history: ${commandErrorMessage(result.error)}`);
    }
  }

  async function pickBackupDest() {
    const picked = await saveDialog({
      title: 'Backup destination',
      filters: [{ name: 'Veylora backup', extensions: ['vey'] }],
    });
    if (picked) setBackupDest(picked);
  }

  async function pickRestoreSource() {
    const picked = await openDialog({
      title: 'Backup file to restore',
      multiple: false,
      filters: [{ name: 'Veylora backup', extensions: ['vey'] }],
    });
    if (picked) setRestoreSource(picked);
  }

  async function handleCreateBackup() {
    if (!backupDest) {
      setMessage('Please enter a backup destination path');
      return;
    }
    setLoading(true);
    setMessage(null);
    try {
      const result = await vaultBackupCreate(backupDest);
      if (result.ok) {
        setBackupInfo(result.data);
        setMessage(`Backup created: ${result.data.file_count} files`);
        loadHistory();
      } else {
        setMessage(`Backup failed: ${commandErrorMessage(result.error)}`);
      }
    } finally {
      setLoading(false);
    }
  }

  async function handleVerifyBackup() {
    if (!backupDest) {
      setMessage('Please enter a backup path to verify');
      return;
    }
    setLoading(true);
    setMessage(null);
    try {
      const result = await vaultBackupVerify(backupDest);
      if (result.ok) {
        setMessage(
          `Backup verified: ${result.data.file_count} files, verified: ${result.data.verified_fields.join(', ')}`,
        );
      } else {
        setMessage(`Verification failed: ${commandErrorMessage(result.error)}`);
      }
    } finally {
      setLoading(false);
    }
  }

  async function handleRestoreBackup() {
    if (!restoreSource) {
      setMessage('Please enter a backup file to restore');
      return;
    }
    setLoading(true);
    setMessage(null);
    try {
      const result = await vaultBackupRestore(restoreSource);
      if (result.ok) {
        setRestoreInfo(result.data);
        setMessage(
          `Restored: ${result.data.files_restored} files, ${result.data.bytes_written} bytes`,
        );
      } else {
        setMessage(`Restore failed: ${commandErrorMessage(result.error)}`);
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="max-w-md mx-auto">
        <Card>
          <CardHeader>
            <CardTitle>Backup & Restore</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm text-muted">Backup Destination</label>
                <input
                  type="text"
                  placeholder="Backup destination path"
                  value={backupDest}
                  onChange={(e) => setBackupDest(e.target.value)}
                  className="w-full bg-surface border border-border rounded-input px-3 py-2 text-body text-foreground"
                />
                <p className="text-caption text-muted-foreground mt-1">
                  Enter a full filesystem path (e.g., C:\Users\Name\Backups\myvault.vey)
                </p>
                <Button variant="outline" size="sm" onClick={pickBackupDest}>
                  Browse…
                </Button>
              </div>
              <div className="flex gap-2">
                <Button
                  onClick={handleCreateBackup}
                  disabled={loading || !backupDest}
                >
                  {loading ? 'Creating...' : 'Create Backup'}
                </Button>
                <Button
                  variant="outline"
                  onClick={handleVerifyBackup}
                  disabled={loading || !backupDest}
                >
                  Verify
                </Button>
              </div>

              <div className="space-y-2">
                <label className="text-sm text-muted">Restore Source</label>
                <input
                  type="text"
                  placeholder="Backup file to restore"
                  value={restoreSource}
                  onChange={(e) => setRestoreSource(e.target.value)}
                  className="w-full bg-surface border border-border rounded-input px-3 py-2 text-body text-foreground"
                />
                <Button variant="outline" size="sm" onClick={pickRestoreSource}>
                  Browse…
                </Button>
              </div>
              <Button
                onClick={handleRestoreBackup}
                disabled={loading || !restoreSource}
              >
                {loading ? 'Restoring...' : 'Restore from Backup'}
              </Button>

              {message && (
                <div className="p-3 rounded-input bg-elevated/50 border border-border text-sm">
                  {message}
                </div>
              )}

              {backupInfo && (
                <div className="mt-4 p-3 bg-success/10 border border-success/30 rounded-input">
                  <div className="text-sm text-success">Backup Info</div>
                  <div className="text-caption text-muted">
                    Files: {backupInfo.file_count} | Size: {backupInfo.total_size} bytes | Verified: {backupInfo.verified_fields.join(', ')}
                  </div>
                </div>
              )}

              {restoreInfo && (
                <div className="mt-4 p-3 bg-success/10 border border-success/30 rounded-input">
                  <div className="text-sm text-success">Restore Info</div>
                  <div className="text-caption text-muted">
                    Files: {restoreInfo.files_restored} | Bytes: {restoreInfo.bytes_written}
                  </div>
                </div>
              )}

              <div className="mt-6">
                <h3 className="text-sm text-muted mb-2">Backup History</h3>
                <Button onClick={loadHistory} variant="outline" size="sm">
                  Refresh History
                </Button>
                <div className="mt-2">
                  {history.length === 0 && (
                    <div className="text-caption text-muted-foreground">No backup history</div>
                  )}
                  {history.map((entry, i) => (
                    <div key={i} className="text-caption text-muted truncate">
                      {entry.path} — {entry.status}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
