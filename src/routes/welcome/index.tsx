import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { vaultCreate, vaultGetMeta, vaultOpen } from '@/lib/ipc';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

const VAULT_PATH_KEY = 'veylora.vaultPath';
const RECENT_KEY = 'veylora.recentVaults';
const RECENT_MAX = 5;

function readRecent(): string[] {
  try {
    const raw = window.localStorage.getItem(RECENT_KEY);
    const list = raw ? (JSON.parse(raw) as unknown) : [];
    return Array.isArray(list)
      ? list.filter((p): p is string => typeof p === 'string')
      : [];
  } catch {
    return [];
  }
}

function writeRecent(paths: string[]) {
  try {
    window.localStorage.setItem(RECENT_KEY, JSON.stringify(paths.slice(0, RECENT_MAX)));
  } catch {
    /* ignore */
  }
}

function pushRecent(p: string) {
  writeRecent([p, ...readRecent().filter((x) => x !== p)]);
}

function folderLabel(p: string): string {
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || p;
}

export default function Welcome({
  adminDefault = false,
  onCreated,
}: {
  adminDefault?: boolean;
  onCreated: (recoveryKey: string) => void;
}) {
  const [name, setName] = useState('');
  const [path, setPath] = useState('');
  const [pin, setPin] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const [openPath, setOpenPath] = useState(
    () => window.localStorage.getItem(VAULT_PATH_KEY) || '',
  );
  const [openError, setOpenError] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);

  // Last-used vaults, newest first. Validated on mount so a moved or deleted
  // vault never leaves a dead row in the list.
  const [recent, setRecent] = useState<string[]>([]);

  useEffect(() => {
    let alive = true;
    (async () => {
      let stored = readRecent();
      if (stored.length === 0) {
        // Migrate existing installs: the remembered last vault becomes the
        // first recent entry, so the list is useful on the very first run.
        const legacy = window.localStorage.getItem(VAULT_PATH_KEY);
        if (legacy) {
          stored = [legacy];
          writeRecent(stored);
        }
      }
      if (stored.length === 0) return;
      const checks = await Promise.all(stored.map((p) => vaultGetMeta(p)));
      const valid = stored.filter((_, i) => checks[i].ok);
      if (!alive) return;
      if (valid.length !== stored.length) writeRecent(valid);
      setRecent(valid);
    })();
    return () => {
      alive = false;
    };
  }, []);

  async function handleBrowse() {
    const dir = await openDialog({ directory: true });
    if (dir) setPath(dir as string);
  }

  async function handleBrowseOpen() {
    const dir = await openDialog({ directory: true });
    if (dir) setOpenPath(dir as string);
  }

  async function handleRecent(p: string) {
    setOpenError(null);
    setOpening(true);
    const result = await vaultOpen(p);
    setOpening(false);
    if (result.ok) {
      pushRecent(p);
      window.localStorage.setItem(VAULT_PATH_KEY, p);
      window.location.href = '/';
    } else {
      setOpenError((result.error as any)?.message || 'Failed to open vault');
      // Stale entry (vault moved or deleted): drop it instead of leaving a
      // row that can never work.
      const next = readRecent().filter((x) => x !== p);
      writeRecent(next);
      setRecent(next);
    }
  }

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (!/^\d{4,6}$/.test(pin)) {
      setError('PIN must be 4-6 digits');
      return;
    }
    if (password.length < 8) {
      setError('Password must be at least 8 characters');
      return;
    }
    if (password !== confirm) {
      setError('Passwords do not match');
      return;
    }
    setLoading(true);
    const result = await vaultCreate(name, path, pin, password);
    setLoading(false);
    if (result.ok) {
      window.localStorage.setItem(VAULT_PATH_KEY, path);
      pushRecent(path);
      // No page reload here: reloading would discard the one-time recovery
      // key before the user ever sees it.
      onCreated(result.data.recovery_key);
    } else {
      setError((result.error as any)?.message || 'Failed to create vault');
    }
  }

  async function handleOpen(e: React.FormEvent) {
    e.preventDefault();
    setOpenError(null);
    setOpening(true);
    const result = await vaultOpen(openPath);
    setOpening(false);
    if (result.ok) {
      window.localStorage.setItem(VAULT_PATH_KEY, openPath);
      pushRecent(openPath);
      window.location.href = '/';
    } else {
      setOpenError((result.error as any)?.message || 'Failed to open vault');
    }
  }

  return (
    <div className="min-h-screen flex flex-col items-center justify-center bg-background px-6 py-12">
      <h1 className="text-display text-foreground mb-12">Veylora</h1>
      <div className="w-full max-w-md p-8 bg-surface border border-border rounded-card">
        {recent.length > 0 && (
          <div className="mb-8">
            <h2 className="text-title-3 text-foreground mb-4">Open a vault</h2>
            <div className="space-y-3">
              {recent.map((p, i) => (
                <button
                  key={p}
                  type="button"
                  onClick={() => handleRecent(p)}
                  disabled={opening}
                  className={`w-full text-left rounded-card border px-4 py-4 transition-colors ${
                    i === 0
                      ? 'border-accent bg-elevated hover:bg-elevated'
                      : 'border-border bg-background/60 hover:border-muted-foreground'
                  }`}
                >
                  <span className="block text-body font-medium text-foreground">
                    {folderLabel(p)}
                  </span>
                  <span className="block truncate text-mono text-muted mt-1">{p}</span>
                </button>
              ))}
            </div>
          </div>
        )}

        <h2 className="text-title-2 text-foreground mb-6">Create Vault</h2>
        {adminDefault && (
          <div className="mb-4 p-3 rounded-input border border-warning/40 bg-warning/10 text-warning text-sm">
            Admin account is using the default password. Change it in Settings → Account.
          </div>
        )}
        <form onSubmit={handleCreate} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="name">Vault Name</Label>
            <Input
              id="name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Vault"
              required
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="path">Storage Location</Label>
            <div className="flex gap-2">
              <Input
                id="path"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder="C:\Users\Name\Documents\MyVault"
                required
              />
              <Button type="button" variant="outline" onClick={handleBrowse}>
                Browse...
              </Button>
            </div>
          </div>
          <div className="space-y-2">
            <Label htmlFor="pin">PIN (4-6 digits)</Label>
            <Input
              id="pin"
              type="password"
              value={pin}
              onChange={(e) => setPin(e.target.value)}
              maxLength={6}
              required
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="password">Password</Label>
            <Input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="At least 8 characters"
              required
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="confirm-password">Confirm Password</Label>
            <Input
              id="confirm-password"
              type="password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              placeholder="Repeat password"
              required
            />
          </div>
          <div className="text-caption text-warning/80 bg-warning/5 border border-warning/20 rounded-input p-3">
            If you lose your PIN, password, AND recovery key, your files cannot
            be recovered. There is no way to reset.
          </div>
          {error && (
            <div className="text-sm text-danger bg-danger/10 p-3 rounded-input">
              {error}
            </div>
          )}
          <Button type="submit" disabled={loading} className="w-full">
            {loading ? 'Creating...' : 'Create Vault'}
          </Button>
        </form>

        <div className="mt-8 pt-6 border-t border-border">
          <h2 className="text-title-3 text-foreground mb-4">Open Existing Vault</h2>
          <form onSubmit={handleOpen} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="open-path">Vault Path</Label>
              <div className="flex gap-2">
                <Input
                  id="open-path"
                  value={openPath}
                  onChange={(e) => setOpenPath(e.target.value)}
                  placeholder="C:\Users\Name\Documents\MyVault"
                  required
                />
                <Button type="button" variant="outline" onClick={handleBrowseOpen}>
                  Browse...
                </Button>
              </div>
            </div>
            {openError && (
              <div className="text-sm text-danger bg-danger/10 p-3 rounded-input">
                {openError}
              </div>
            )}
            <Button type="submit" disabled={opening} variant="outline" className="w-full">
              {opening ? 'Opening...' : 'Open Vault'}
            </Button>
          </form>
        </div>
      </div>
    </div>
  );
}
