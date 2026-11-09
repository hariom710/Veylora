import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Card, CardContent, CardTitle, CardHeader } from '@/components/ui/card';
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
    <div className="min-h-screen flex items-center justify-center bg-slate-950">
      <div className="w-full max-w-md p-8 bg-slate-900 border border-slate-800 rounded-lg shadow-xl">
        <Card>
          <CardHeader>
            <CardTitle>Unlock Vault</CardTitle>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="flex gap-2">
              <Button
                variant={method === 'pin' ? 'default' : 'outline'}
                onClick={() => setMethod('pin')}
                size="sm"
              >
                PIN
              </Button>
              <Button
                variant={method === 'password' ? 'default' : 'outline'}
                onClick={() => setMethod('password')}
                size="sm"
              >
                Password
              </Button>
              <Button
                variant={method === 'recovery' ? 'default' : 'outline'}
                onClick={() => setMethod('recovery')}
                size="sm"
              >
                Recovery
              </Button>
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
                <div className="text-sm text-destructive-foreground bg-destructive/10 p-3 rounded">
                  {error}
                </div>
              )}
              <Button type="submit" disabled={loading} className="w-full bg-primary text-primary-foreground">
                {loading ? 'Unlocking...' : 'Unlock'}
              </Button>
            </form>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
