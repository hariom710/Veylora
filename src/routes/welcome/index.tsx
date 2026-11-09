import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { vaultCreate } from '@/lib/ipc';

export default function Welcome() {
  const [name, setName] = useState('');
  const [path, setPath] = useState('');
  const [pin, setPin] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);
    const result = await vaultCreate(name, path, pin, password);
    setLoading(false);
    if (result.ok) {
      window.location.href = '/';
    } else {
      setError((result.error as any)?.message || 'Failed to create vault');
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-slate-950">
      <div className="w-full max-w-md p-8 bg-slate-900 border border-slate-800 rounded-lg shadow-xl">
        <h1 className="text-2xl font-bold text-white mb-6">Create Vault</h1>
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
            <Label htmlFor="path">Vault Path</Label>
            <Input
              id="path"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="/path/to/vault"
              required
            />
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
              required
            />
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
      </div>
    </div>
  );
}
