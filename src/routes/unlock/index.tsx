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
        const token = await vaultGetSessionToken();
        onUnlock(method, token.ok ? token.data : '');
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
                  {/* Circular keypad (Phase 2 brief): an alternative entry
                      method. The input above stays editable for keyboards,
                      paste, tests and automation. */}
                  <div className="grid grid-cols-3 gap-3 justify-items-center mt-4">
                    {['1', '2', '3', '4', '5', '6', '7', '8', '9'].map((d) => (
                      <button
                        key={d}
                        type="button"
                        onClick={() => setValue((v) => (v.length >= 6 ? v : v + d))}
                        className="w-14 h-14 rounded-full border border-border bg-elevated text-title-3 text-foreground hover:border-accent hover:text-accent transition-colors"
                      >
                        {d}
                      </button>
                    ))}
                    <button
                      type="button"
                      aria-label="Clear"
                      onClick={() => setValue('')}
                      className="w-14 h-14 rounded-full border border-border bg-elevated text-body-sm text-muted hover:border-accent hover:text-accent transition-colors"
                    >
                      C
                    </button>
                    <button
                      type="button"
                      onClick={() => setValue((v) => (v.length >= 6 ? v : v + '0'))}
                      className="w-14 h-14 rounded-full border border-border bg-elevated text-title-3 text-foreground hover:border-accent hover:text-accent transition-colors"
                    >
                      0
                    </button>
                    <button
                      type="button"
                      aria-label="Backspace"
                      onClick={() => setValue((v) => v.slice(0, -1))}
                      className="w-14 h-14 rounded-full border border-border bg-elevated text-body text-muted hover:border-accent hover:text-accent transition-colors"
                    >
                      ⌫
                    </button>
                  </div>
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
