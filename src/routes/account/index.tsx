import { useState, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle, CardHeader } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  appChangePassword,
  appListUsers,
  appDeleteUser,
  appCreateUser,
  appLogout,
} from '@/lib/ipc';
import { commandErrorMessage, type AppStatus, type AppUser } from '@/lib/types';

interface AccountProps {
  app: AppStatus;
  // Clears the Layer-1 session in the UI. Deliberately does NOT lock the
  // vault: logout and vault lock are independent (three-layer model).
  onLogout: () => void;
}

function formatUnix(ts: number): string {
  return new Date(ts * 1000).toLocaleString();
}

export default function Account({ app, onLogout }: AccountProps) {
  const isAdmin = app.role === 'admin';
  const [users, setUsers] = useState<AppUser[]>([]);

  const [oldPassword, setOldPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [pwMessage, setPwMessage] = useState<{ ok: boolean; text: string } | null>(null);

  const [showAddUser, setShowAddUser] = useState(false);
  const [newEmail, setNewEmail] = useState('');
  const [newUserPw, setNewUserPw] = useState('');
  const [newUserRole, setNewUserRole] = useState('user');
  const [addMessage, setAddMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  async function loadUsers() {
    if (!isAdmin) return;
    const result = await appListUsers();
    if (result.ok) setUsers(result.data);
  }

  useEffect(() => {
    loadUsers();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function handleChangePassword(e: React.FormEvent) {
    e.preventDefault();
    setPwMessage(null);
    if (newPassword.length < 8) {
      setPwMessage({ ok: false, text: 'Password must be at least 8 characters' });
      return;
    }
    if (newPassword !== confirmPassword) {
      setPwMessage({ ok: false, text: 'Passwords do not match' });
      return;
    }
    const result = await appChangePassword(oldPassword, newPassword);
    if (result.ok) {
      setPwMessage({ ok: true, text: 'Password updated' });
      setOldPassword('');
      setNewPassword('');
      setConfirmPassword('');
    } else {
      setPwMessage({
        ok: false,
        text: commandErrorMessage(result.error) || 'Failed to update password',
      });
    }
  }

  async function handleAddUser(e: React.FormEvent) {
    e.preventDefault();
    setAddMessage(null);
    const result = await appCreateUser(newEmail, newUserPw, newUserRole);
    if (result.ok) {
      setAddMessage({ ok: true, text: `User ${newEmail} created` });
      setNewEmail('');
      setNewUserPw('');
      setShowAddUser(false);
      await loadUsers();
    } else {
      setAddMessage({
        ok: false,
        text: commandErrorMessage(result.error) || 'Failed to create user',
      });
    }
  }

  async function handleDelete(user: AppUser) {
    setActionError(null);
    const result = await appDeleteUser(user.id);
    if (!result.ok) {
      setActionError(commandErrorMessage(result.error) || 'Failed to delete user');
    } else {
      await loadUsers();
    }
  }

  async function handleLogout() {
    await appLogout();
    onLogout();
  }

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="max-w-3xl mx-auto space-y-6">
        <div className="flex items-center gap-4">
          {/* Avatar (Phase 2 brief): initial, local, no gravatar/network. */}
          <div
            className="w-14 h-14 rounded-full bg-elevated border border-border flex items-center justify-center text-title-3 text-accent select-none shrink-0"
            aria-hidden="true"
          >
            {(app.email || '?').charAt(0).toUpperCase()}
          </div>
          <div className="min-w-0">
            <div className="text-body font-semibold text-foreground truncate">{app.email}</div>
            <div className="text-caption text-muted capitalize">{app.role}</div>
          </div>
        </div>
        <h1 className="text-title-1">Account</h1>

        <Card>
          <CardHeader>
            <CardTitle>Your account</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2 text-sm">
            <div className="flex justify-between">
              <span className="text-muted">Email</span>
              <span className="text-foreground">{app.email}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-muted">Role</span>
              <span className="text-foreground capitalize">{app.role}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-muted">Logged in since</span>
              <span className="text-foreground">
                {app.logged_in_since ? formatUnix(app.logged_in_since) : '—'}
              </span>
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Change password</CardTitle>
          </CardHeader>
          <CardContent>
            <form onSubmit={handleChangePassword} className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="old-password">Old password</Label>
                <Input
                  id="old-password"
                  type="password"
                  value={oldPassword}
                  onChange={(e) => setOldPassword(e.target.value)}
                  required
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="new-password">New password</Label>
                <Input
                  id="new-password"
                  type="password"
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  placeholder="At least 8 characters"
                  required
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="confirm-new-password">Confirm new password</Label>
                <Input
                  id="confirm-new-password"
                  type="password"
                  value={confirmPassword}
                  onChange={(e) => setConfirmPassword(e.target.value)}
                  required
                />
              </div>
              {pwMessage && (
                <div
                  className={`text-sm p-3 rounded-input ${
                    pwMessage.ok
                      ? 'text-success bg-success/10'
                      : 'text-danger bg-danger/10'
                  }`}
                >
                  {pwMessage.text}
                </div>
              )}
              <Button type="submit" className="w-full">
                Change password
              </Button>
            </form>
          </CardContent>
        </Card>

        {isAdmin && (
          <Card>
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle>Users</CardTitle>
                <Button size="sm" variant="outline" onClick={() => setShowAddUser((v) => !v)}>
                  Add user
                </Button>
              </div>
            </CardHeader>
            <CardContent>
              {actionError && (
                <div className="mb-3 text-sm text-danger bg-danger/10 p-3 rounded-input">
                  {actionError}
                </div>
              )}
              {showAddUser && (
                <form onSubmit={handleAddUser} className="mb-4 space-y-3 border border-border rounded-input p-4">
                  {addMessage && (
                    <div
                      className={`text-sm p-2 rounded-input ${
                        addMessage.ok
                          ? 'text-success bg-success/10'
                          : 'text-danger bg-danger/10'
                      }`}
                    >
                      {addMessage.text}
                    </div>
                  )}
                  <div className="grid grid-cols-3 gap-3">
                    <Input
                      placeholder="Email"
                      type="email"
                      value={newEmail}
                      onChange={(e) => setNewEmail(e.target.value)}
                      required
                    />
                    <Input
                      placeholder="Password (min 8)"
                      type="password"
                      value={newUserPw}
                      onChange={(e) => setNewUserPw(e.target.value)}
                      required
                    />
                    <select
                      value={newUserRole}
                      onChange={(e) => setNewUserRole(e.target.value)}
                      className="bg-elevated border border-border rounded-input px-3 py-2 text-sm text-foreground"
                    >
                      <option value="user">user</option>
                      <option value="admin">admin</option>
                    </select>
                  </div>
                  <Button type="submit" size="sm">
                    Create user
                  </Button>
                </form>
              )}
              <table className="w-full text-sm">
                <thead>
                  <tr className="text-left text-muted border-b border-border">
                    <th className="py-2">Email</th>
                    <th>Role</th>
                    <th>Created</th>
                    <th>Last login</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  {users.map((u) => (
                    <tr key={u.id} className="border-b border-border/60">
                      <td className="py-2 text-foreground">{u.email}</td>
                      <td className="text-foreground capitalize">{u.role}</td>
                      <td className="text-muted">{formatUnix(u.created_at)}</td>
                      <td className="text-muted">
                        {u.last_login ? formatUnix(u.last_login) : 'never'}
                      </td>
                      <td className="text-right">
                        <Button
                          size="sm"
                          variant="destructive"
                          onClick={() => handleDelete(u)}
                        >
                          Delete
                        </Button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </CardContent>
          </Card>
        )}

        <div className="flex justify-end">
          <Button variant="destructive" onClick={handleLogout}>
            Sign out
          </Button>
        </div>
      </div>
    </div>
  );
}
