import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { vaultCreate, vaultOpen } from '@/lib/ipc';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

const VAULT_PATH_KEY = 'veylora.vaultPath';

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

  const [openPath, setOpenPath] = useState('');
  const [openError, setOpenError] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);

  async function handleBrowse() {
    const dir = await openDialog({ directory: true });
    if (dir) setPath(dir as string);
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
      window.location.href = '/';
    } else {
      setOpenError((result.error as any)?.message || 'Failed to open vault');
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-slate-950">
      <div className="w-full max-w-md p-8 bg-slate-900 border border-slate-800 rounded-lg shadow-xl">
        <h1 className="text-2xl font-bold text-white mb-6">Create Vault</h1>
        {adminDefault && (
          <div className="mb-4 p-3 rounded border border-yellow-500/40 bg-yellow-500/10 text-yellow-300 text-sm">
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
          <div className="text-xs text-yellow-400/80 bg-yellow-500/5 border border-yellow-500/20 rounded p-3">
            If you lose your PIN, password, AND recovery key, your files cannot
            be recovered. There is no way to reset.
          </div>
          {error && (
            <div className="text-sm text-destructive-foreground bg-destructive/10 p-3 rounded">
              {error}
            </div>
          )}
          <Button type="submit" disabled={loading} className="w-full bg-primary text-primary-foreground">
            {loading ? 'Creating...' : 'Create Vault'}
          </Button>
        </form>

        <div className="mt-8 pt-6 border-t border-slate-800">
          <h2 className="text-sm font-semibold text-slate-300 mb-3">Open Existing Vault</h2>
          <form onSubmit={handleOpen} className="space-y-3">
            <div className="space-y-2">
              <Label htmlFor="open-path">Vault Path</Label>
              <Input
                id="open-path"
                value={openPath}
                onChange={(e) => setOpenPath(e.target.value)}
                placeholder="C:\Users\Name\Documents\MyVault"
                required
              />
            </div>
            {openError && (
              <div className="text-sm text-destructive-foreground bg-destructive/10 p-3 rounded">
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
