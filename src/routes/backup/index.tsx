import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle, CardHeader } from '@/components/ui/card';
import {
  vaultBackupCreate,
  vaultBackupVerify,
  vaultBackupRestore,
  vaultBackupHistory,
} from '@/lib/ipc';
import type { BackupInfo, RestoreInfo, BackupHistoryEntry } from '@/lib/types';
import { saveDialog, openDialog } from '@tauri-apps/plugin-dialog';

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
      setMessage(`Failed to load history: ${result.error.message}`);
    }
  }

  async function handlePickBackupDest() {
    const selected = await saveDialog({
      defaultPath: 'veylora-backup.vey',
      filters: [{ name: 'Veylora Backup', extensions: ['vey'] }],
    });
    if (selected) {
      setBackupDest(selected);
    }
  }

  async function handlePickRestoreSource() {
    const selected = await openDialog({
      filters: [{ name: 'Veylora Backup', extensions: ['vey'] }],
      multiple: false,
    });
    if (selected && selected.length > 0) {
      setRestoreSource(selected[0]);
    }
  }

  async function handleCreateBackup() {
    if (!backupDest) {
      setMessage('Please select a backup destination');
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
        setMessage(`Backup failed: ${result.error.message}`);
      }
    } finally {
      setLoading(false);
    }
  }

  async function handleVerifyBackup() {
    if (!backupDest) {
      setMessage('Please select a backup to verify');
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
        setMessage(`Verification failed: ${result.error.message}`);
      }
    } finally {
      setLoading(false);
    }
  }

  async function handleRestoreBackup() {
    if (!restoreSource) {
      setMessage('Please select a backup to restore');
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
        setMessage(`Restore failed: ${result.error.message}`);
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="min-h-screen bg-slate-950 p-8">
      <div className="max-w-md mx-auto">
        <Card>
          <CardHeader>
            <CardTitle>Backup & Restore</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <div className="space-y-2">
                <label className="text-sm text-slate-400">Backup Destination</label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    placeholder="Select backup destination..."
                    value={backupDest}
                    onChange={(e) => setBackupDest(e.target.value)}
                    className="flex-1 bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200"
                  />
                  <Button onClick={handlePickBackupDest} variant="outline" size="sm">
                    Browse
                  </Button>
                </div>
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
                <label className="text-sm text-slate-400">Restore Source</label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    placeholder="Select backup file..."
                    value={restoreSource}
                    onChange={(e) => setRestoreSource(e.target.value)}
                    className="flex-1 bg-slate-800 border border-slate-700 rounded px-3 py-2 text-sm text-slate-200"
                  />
                  <Button onClick={handlePickRestoreSource} variant="outline" size="sm">
                    Browse
                  </Button>
                </div>
              </div>
              <Button
                onClick={handleRestoreBackup}
                disabled={loading || !restoreSource}
              >
                {loading ? 'Restoring...' : 'Restore from Backup'}
              </Button>

              {message && (
                <div className="p-3 rounded bg-slate-800/50 border border-slate-700 text-sm">
                  {message}
                </div>
              )}

              {backupInfo && (
                <div className="mt-4 p-3 bg-green-900/20 border border-green-500/30 rounded">
                  <div className="text-sm text-green-400">Backup Info</div>
                  <div className="text-xs text-slate-400">
                    Files: {backupInfo.file_count} | Size: {backupInfo.total_size} bytes | Verified: {backupInfo.verified_fields.join(', ')}
                  </div>
                </div>
              )}

              {restoreInfo && (
                <div className="mt-4 p-3 bg-green-900/20 border border-green-500/30 rounded">
                  <div className="text-sm text-green-400">Restore Info</div>
                  <div className="text-xs text-slate-400">
                    Files: {restoreInfo.files_restored} | Bytes: {restoreInfo.bytes_written}
                  </div>
                </div>
              )}

              <div className="mt-6">
                <h3 className="text-sm text-slate-400 mb-2">Backup History</h3>
                <Button onClick={loadHistory} variant="outline" size="sm">
                  Refresh History
                </Button>
                <div className="mt-2">
                  {history.length === 0 && (
                    <div className="text-xs text-slate-500">No backup history</div>
                  )}
                  {history.map((entry, i) => (
                    <div key={i} className="text-xs text-slate-400 truncate">
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
