import { useState, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle, CardHeader } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  vaultStatus,
  vaultChangePin,
  vaultChangePassword,
  vaultBackupHistoryClear,
  vaultDelete,
  autoLockSet,
} from '@/lib/ipc';
import { formatBytes } from '@/lib/format';

const AUTO_LOCK_KEY = 'veylora.autoLockSecs';

type Tab = 'security' | 'appearance' | 'storage' | 'backup' | 'about' | 'danger';

const TABS: { id: Tab; label: string }[] = [
  { id: 'security', label: 'Security' },
  { id: 'appearance', label: 'Appearance' },
  { id: 'storage', label: 'Storage' },
  { id: 'backup', label: 'Backup' },
  { id: 'about', label: 'About' },
  { id: 'danger', label: 'Danger Zone' },
];

const AUTO_LOCK_CHOICES: { label: string; secs: number }[] = [
  { label: '1 minute', secs: 60 },
  { label: '5 minutes (default)', secs: 300 },
  { label: '15 minutes', secs: 900 },
  { label: '30 minutes', secs: 1800 },
  { label: 'Never', secs: 0 },
];

function savedAutoLockSecs(): number {
  try {
    const raw = window.localStorage.getItem(AUTO_LOCK_KEY);
    if (raw != null) return Number(raw);
  } catch {
    /* ignore */
  }
  return 300;
}

export default function Settings() {
  const [tab, setTab] = useState<Tab>('security');
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Auto-lock (persisted by the frontend, applied to the backend watcher).
  const [autoLock, setAutoLock] = useState<number>(savedAutoLockSecs);

  // Change PIN / password forms.
  const [currentPin, setCurrentPin] = useState('');
  const [newPin, setNewPin] = useState('');
  const [currentPw, setCurrentPw] = useState('');
  const [newPw, setNewPw] = useState('');
  const [confirmPw, setConfirmPw] = useState('');

  // Danger zone.
  const [deletePin, setDeletePin] = useState('');
  const [deleteConfirmText, setDeleteConfirmText] = useState('');

  const [status, setStatus] = useState<any>(null);

  useEffect(() => {
    vaultStatus().then(({ ok, data }) => {
      if (ok) setStatus(data);
    });
  }, []);

  // Re-apply the saved timeout to the running backend watcher.
  useEffect(() => {
    autoLockSet(autoLock).catch(() => {});
  }, [autoLock]);

  function ok(msg: string) {
    setError(null);
    setMessage(msg);
  }

  async function handleAutoLock(secs: number) {
    setAutoLock(secs);
    try {
      window.localStorage.setItem(AUTO_LOCK_KEY, String(secs));
    } catch {
      /* ignore */
    }
    const result = await autoLockSet(secs);
    if (result.ok) ok(secs === 0 ? 'Auto-lock disabled' : 'Auto-lock timeout updated');
    else setError((result.error as any)?.message || 'Failed to update auto-lock');
  }

  async function handleChangePin(e: React.FormEvent) {
    e.preventDefault();
    setMessage(null);
    setError(null);
    if (!/^\d{4,6}$/.test(newPin)) {
      setError('New PIN must be 4-6 digits');
      return;
    }
    const result = await vaultChangePin(currentPin, newPin);
    if (result.ok) {
      ok('PIN changed');
      setCurrentPin('');
      setNewPin('');
    } else {
      setError((result.error as any)?.message || 'Failed to change PIN');
    }
  }

  async function handleChangePassword(e: React.FormEvent) {
    e.preventDefault();
    setMessage(null);
    setError(null);
    if (newPw.length < 8) {
      setError('New password must be at least 8 characters');
      return;
    }
    if (newPw !== confirmPw) {
      setError('New passwords do not match');
      return;
    }
    const result = await vaultChangePassword(currentPw, newPw);
    if (result.ok) {
      ok('Password changed');
      setCurrentPw('');
      setNewPw('');
      setConfirmPw('');
    } else {
      setError((result.error as any)?.message || 'Failed to change password');
    }
  }

  async function handleClearHistory() {
    const confirmed = window.confirm(
      'Clear the backup history list? Backup files on disk are NOT deleted.',
    );
    if (!confirmed) return;
    const result = await vaultBackupHistoryClear();
    if (result.ok) ok('Backup history cleared');
    else setError((result.error as any)?.message || 'Failed to clear history');
  }

  async function handleDeleteVault(e: React.FormEvent) {
    e.preventDefault();
    setMessage(null);
    setError(null);
    if (deleteConfirmText !== 'DELETE MY VAULT') {
      setError('Type DELETE MY VAULT to confirm');
      return;
    }
    const confirmed = window.confirm(
      'This permanently destroys the vault and every encrypted file inside it. This cannot be undone. Continue?',
    );
    if (!confirmed) return;
    const result = await vaultDelete(deletePin);
    if (result.ok) {
      try {
        window.localStorage.removeItem('veylora.vaultPath');
      } catch {
        /* ignore */
      }
      // Back to a fresh install state (Create Vault screen).
      window.location.reload();
    } else {
      setError((result.error as any)?.message || 'Failed to delete vault');
    }
  }

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="max-w-3xl mx-auto">
        <h1 className="text-title-1 mb-6">Settings</h1>

        <div className="flex flex-wrap gap-6 mb-6 border-b border-border">
          {TABS.map((t) => (
            <button
              key={t.id}
              onClick={() => setTab(t.id)}
              className={`pb-3 -mb-px text-body-sm font-medium transition-colors ${
                tab === t.id
                  ? 'text-foreground border-b-2 border-accent'
                  : 'text-muted hover:text-foreground border-b-2 border-transparent'
              }`}
            >
              {t.label}
            </button>
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

        {tab === 'security' && (
          <div className="space-y-6">
            <Card>
              <CardHeader>
                <CardTitle>Auto-lock</CardTitle>
              </CardHeader>
              <CardContent>
                <Label htmlFor="auto-lock">Lock the vault after inactivity</Label>
                <select
                  id="auto-lock"
                  value={autoLock}
                  onChange={(e) => handleAutoLock(Number(e.target.value))}
                  className="mt-2 w-full h-10 rounded-button border border-border bg-surface px-3 text-sm"
                >
                  {AUTO_LOCK_CHOICES.map((c) => (
                    <option key={c.secs} value={c.secs}>
                      {c.label}
                    </option>
                  ))}
                </select>
                <p className="text-caption text-muted-foreground mt-2">
                  Inactivity means no mouse or keyboard input. Relocking is checked every 30
                  seconds.
                </p>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Change PIN</CardTitle>
              </CardHeader>
              <CardContent>
                <form onSubmit={handleChangePin} className="space-y-4">
                  <div className="space-y-2">
                    <Label htmlFor="current-pin">Current PIN</Label>
                    <Input
                      id="current-pin"
                      type="password"
                      inputMode="numeric"
                      value={currentPin}
                      onChange={(e) => setCurrentPin(e.target.value)}
                      required
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="new-pin">New PIN</Label>
                    <Input
                      id="new-pin"
                      type="password"
                      inputMode="numeric"
                      value={newPin}
                      onChange={(e) => setNewPin(e.target.value)}
                      placeholder="4-6 digits"
                      required
                    />
                  </div>
                  <Button type="submit">Change PIN</Button>
                </form>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Change Password</CardTitle>
              </CardHeader>
              <CardContent>
                <form onSubmit={handleChangePassword} className="space-y-4">
                  <div className="space-y-2">
                    <Label htmlFor="current-pw">Current Password</Label>
                    <Input
                      id="current-pw"
                      type="password"
                      value={currentPw}
                      onChange={(e) => setCurrentPw(e.target.value)}
                      required
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="new-pw">New Password</Label>
                    <Input
                      id="new-pw"
                      type="password"
                      value={newPw}
                      onChange={(e) => setNewPw(e.target.value)}
                      placeholder="At least 8 characters"
                      required
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="confirm-new-pw">Confirm New Password</Label>
                    <Input
                      id="confirm-new-pw"
                      type="password"
                      value={confirmPw}
                      onChange={(e) => setConfirmPw(e.target.value)}
                      required
                    />
                  </div>
                  <Button type="submit">Change Password</Button>
                </form>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Recovery Key</CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-sm text-muted">
                  Your recovery key was shown exactly once, when this vault was created. It wraps
                  your vault key one-way — Veylora cannot re-display it, by design. If you lost it,
                  your PIN and password still unlock the vault; the recovery key is only needed if
                  you lose both.
                </p>
              </CardContent>
            </Card>
          </div>
        )}

        {tab === 'appearance' && (
          <Card>
            <CardHeader>
              <CardTitle>Appearance</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="flex items-center justify-between">
                <span className="text-muted text-sm">Theme</span>
                <span className="text-sm text-foreground">Dark (only theme in v1.0)</span>
              </div>
              <p className="text-caption text-muted-foreground mt-3">
                A light theme is planned for v1.1 — v1.0 ships dark only.
              </p>
            </CardContent>
          </Card>
        )}

        {tab === 'storage' && (
          <Card>
            <CardHeader>
              <CardTitle>Storage</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-2 text-sm text-muted">
                <div className="flex justify-between">
                  <span>Files encrypted</span>
                  <span className="text-foreground">{status?.file_count ?? 0}</span>
                </div>
                <div className="flex justify-between">
                  <span>Vault size on disk</span>
                  <span className="text-foreground">
                    {formatBytes(status?.storage_used ?? 0)}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span>Vault path</span>
                  <span className="text-foreground break-all">
                    {status?.path ?? status?.name ?? '—'}
                  </span>
                </div>
              </div>
              <p className="text-caption text-muted-foreground mt-3">
                Per-folder storage breakdown is planned for v1.1.
              </p>
            </CardContent>
          </Card>
        )}

        {tab === 'backup' && (
          <Card>
            <CardHeader>
              <CardTitle>Backup</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted mb-4">
                Create, verify and restore backups from the Backup screen in the main navigation.
                Veylora never runs backups automatically — every backup is something you start.
              </p>
              <Button variant="outline" onClick={handleClearHistory}>
                Clear backup history
              </Button>
              <p className="text-caption text-muted-foreground mt-2">
                Clears the history list only. Backup files on disk are not touched.
              </p>
            </CardContent>
          </Card>
        )}

        {tab === 'about' && (
          <Card>
            <CardHeader>
              <CardTitle>About Veylora</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="space-y-2 text-sm text-muted">
                <div className="flex justify-between">
                  <span>Version</span>
                  <span className="text-foreground">1.0</span>
                </div>
                <div className="flex justify-between">
                  <span>Vault format</span>
                  <span className="text-foreground">v1 (frozen)</span>
                </div>
                <div className="flex justify-between">
                  <span>Encryption</span>
                  <span className="text-foreground">XChaCha20-Poly1305</span>
                </div>
                <div className="flex justify-between">
                  <span>Key derivation</span>
                  <span className="text-foreground">Argon2id + HKDF-SHA256</span>
                </div>
                <div className="flex justify-between">
                  <span>Storage</span>
                  <span className="text-foreground">Local only — no cloud, ever</span>
                </div>
                <div className="flex justify-between">
                  <span>License</span>
                  <span className="text-foreground">MIT OR Apache-2.0</span>
                </div>
              </div>
            </CardContent>
          </Card>
        )}

        {tab === 'danger' && (
          <Card className="border-danger/50">
            <CardHeader>
              <CardTitle className="text-danger">Delete Vault</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-sm text-muted mb-4">
                This permanently deletes the vault directory and every encrypted file inside it.
                Backup files you created elsewhere are not affected. This cannot be undone.
              </p>
              <form onSubmit={handleDeleteVault} className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="delete-pin">Confirm PIN</Label>
                  <Input
                    id="delete-pin"
                    type="password"
                    inputMode="numeric"
                    value={deletePin}
                    onChange={(e) => setDeletePin(e.target.value)}
                    required
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="delete-confirm">
                    Type <span className="font-mono">DELETE MY VAULT</span> to confirm
                  </Label>
                  <Input
                    id="delete-confirm"
                    value={deleteConfirmText}
                    onChange={(e) => setDeleteConfirmText(e.target.value)}
                    required
                  />
                </div>
                <Button type="submit" variant="destructive">
                  Delete Vault Permanently
                </Button>
              </form>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}
