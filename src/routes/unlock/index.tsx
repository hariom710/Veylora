import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  vaultUnlockPin,
  vaultUnlockPassword,
  vaultUnlockRecovery,
  vaultGetSessionToken,
} from '@/lib/ipc';

type Method = 'pin' | 'password' | 'recovery';

export default function Unlock({ onUnlock }: { onUnlock: (method: string, value: string) => void }) {
  const [method, setMethod] = useState<Method>('pin');
  const [value, setValue] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleUnlock(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);

    try {
      let result;
      if (method === 'pin') result = await vaultUnlockPin(value);
      else if (method === 'password') result = await vaultUnlockPassword(value);
      else result = await vaultUnlockRecovery(value);

      if (result.ok) {
        const { data: token } = await vaultGetSessionToken();
        onUnlock(method, token || '');
      } else {
        setError((result.error as any)?.message || 'Invalid credential');
      }
    } catch (e) {
      setError((e as Error)?.message || 'Invalid credential');
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="min-h-screen flex flex-col items-center justify-center bg-background px-6">
      <div className="text-center mb-8">
        <div className="text-title-1 text-foreground">Veylora</div>
      </div>
      <div className="w-full max-w-md p-8 bg-surface border border-border rounded-card">
        <h1 className="text-title-2 text-foreground text-center mb-6">Unlock Vault</h1>

        <div className="flex gap-6 border-b border-border mb-6">
          {(
            [
              ['pin', 'PIN'],
              ['password', 'Password'],
              ['recovery', 'Recovery'],
            ] as const
          ).map(([m, label]) => (
            <button
              key={m}
              type="button"
              onClick={() => setMethod(m)}
              className={`pb-3 -mb-px text-body-sm font-medium transition-colors ${
                method === m
                  ? 'text-foreground border-b-2 border-accent'
                  : 'text-muted hover:text-foreground border-b-2 border-transparent'
              }`}
            >
              {label}
            </button>
          ))}
        </div>

        <form onSubmit={handleUnlock} className="space-y-4">
              {method === 'pin' && (
                <div className="space-y-2">
                  <Label htmlFor="pin">PIN</Label>
                  <Input
                    id="pin"
                    type="password"
                    value={value}
                    onChange={(e) => setValue(e.target.value)}
                    maxLength={6}
                    placeholder="Enter PIN"
                    required
                  />
                </div>
              )}
              {method === 'password' && (
                <div className="space-y-2">
                  <Label htmlFor="password">Password</Label>
                  <Input
                    id="password"
                    type="password"
                    value={value}
                    onChange={(e) => setValue(e.target.value)}
                    placeholder="Enter password"
                    required
                  />
                </div>
              )}
              {method === 'recovery' && (
                <div className="space-y-2">
                  <Label htmlFor="recovery">Recovery Key</Label>
                  <Input
                    id="recovery"
                    value={value}
                    onChange={(e) => setValue(e.target.value)}
                    placeholder="Enter recovery key"
                    required
                  />
                </div>
              )}
              {error && (
                <div className="text-sm text-danger bg-danger/10 p-3 rounded-input">
                  {error}
                </div>
              )}
              <Button type="submit" disabled={loading} className="w-full mt-2">
                {loading ? 'Unlocking...' : 'Unlock'}
              </Button>
          </form>
      </div>
    </div>
  );
}
