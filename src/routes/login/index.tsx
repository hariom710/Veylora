import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { appLogin, appSignup } from '@/lib/ipc';
import type { AppStatus } from '@/lib/types';

// Layer 1: app login. Does not touch the vault — the vault gate (Welcome /
// Unlock) is only reached after this succeeds.
export default function Login({ onLogin }: { onLogin: (status: AppStatus) => void }) {
  const [mode, setMode] = useState<'login' | 'signup'>('login');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleLogin(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setLoading(true);
    const result = await appLogin(email, password);
    setLoading(false);
    if (result.ok) {
      onLogin(result.data);
    } else {
      setError((result.error as any)?.message || 'Invalid email or password');
    }
  }

  async function handleSignup(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (!email.includes('@')) {
      setError('Enter a valid email address');
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
    const result = await appSignup(email, password);
    setLoading(false);
    if (result.ok) {
      // Account created: back to sign-in with the email kept filled in.
      setMode('login');
      setPassword('');
      setConfirm('');
      setError(null);
    } else {
      setError((result.error as any)?.message || 'Failed to create account');
    }
  }

  return (
    <div className="min-h-screen flex flex-col items-center justify-center bg-background px-6">
      <div className="text-center mb-12">
        <h1 className="text-display text-foreground">Veylora</h1>
        <p className="text-body text-muted mt-3">
          Your Files. Your Privacy. Your Control.
        </p>
      </div>

      <div className="w-full max-w-[400px] p-8 bg-surface border border-border rounded-card">
        {error && (
          <div className="mb-4 text-sm text-danger bg-danger/10 p-3 rounded-input">
            {error}
          </div>
        )}

        {mode === 'login' ? (
          <form onSubmit={handleLogin} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="email">Email</Label>
              <Input
                id="email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
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
                placeholder="Password"
                required
              />
            </div>
            <Button
              type="submit"
              disabled={loading}
              className="w-full mt-2"
            >
              {loading ? 'Signing in...' : 'Sign in'}
            </Button>
          </form>
        ) : (
          <form onSubmit={handleSignup} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="signup-email">Email</Label>
              <Input
                id="signup-email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="you@example.com"
                required
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="signup-password">Password</Label>
              <Input
                id="signup-password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="At least 8 characters"
                required
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="signup-confirm">Confirm password</Label>
              <Input
                id="signup-confirm"
                type="password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                placeholder="Repeat password"
                required
              />
            </div>
            <Button
              type="submit"
              disabled={loading}
              className="w-full mt-2"
            >
              {loading ? 'Creating account...' : 'Create account'}
            </Button>
          </form>
        )}
      </div>

      <p className="text-body-sm text-muted text-center mt-6">
        {mode === 'login' ? "Don't have an account?" : 'Already have an account?'}{' '}
        <button
          type="button"
          onClick={() => {
            setMode(mode === 'login' ? 'signup' : 'login');
            setError(null);
          }}
          className="text-foreground hover:underline"
        >
          {mode === 'login' ? 'Sign up' : 'Sign in'}
        </button>
      </p>
    </div>
  );
}
