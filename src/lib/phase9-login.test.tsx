import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import App from '@/App';

vi.mock('@/lib/ipc', () => ({
  appStatus: vi.fn(),
  appLogin: vi.fn(),
  appSignup: vi.fn(),
  appLogout: vi.fn(),
  appChangePassword: vi.fn(),
  appListUsers: vi.fn(),
  appDeleteUser: vi.fn(),
  appCreateUser: vi.fn(),
  vaultStatus: vi.fn(),
  vaultOpen: vi.fn(),
  vaultGetMeta: vi.fn(),
  vaultGetSessionToken: vi.fn(),
  vaultUnlockPin: vi.fn(),
  vaultUnlockPassword: vi.fn(),
  vaultUnlockRecovery: vi.fn(),
  vaultLock: vi.fn(),
  vaultActivity: vi.fn(),
  autoLockSet: vi.fn(),
  fileRecent: vi.fn(),
  fileFavorites: vi.fn(),
  fileFavorite: vi.fn(),
}));

import {
  appStatus,
  appLogin,
  appSignup,
  appLogout,
  appListUsers,
  vaultStatus,
  vaultGetSessionToken,
  vaultLock,
  fileRecent,
  fileFavorites,
  vaultActivity,
  autoLockSet,
} from '@/lib/ipc';

const LOGGED_OUT = {
  logged_in: false,
  email: null,
  role: null,
  logged_in_since: null,
  used_default_password: false,
};

const LOGGED_IN = {
  logged_in: true,
  email: 'b1865741@gmail.com',
  role: 'admin',
  logged_in_since: 1762700000,
  used_default_password: false,
};

describe('phase9 app login gate', () => {
  const user = userEvent.setup();

  beforeEach(() => {
    vi.resetAllMocks();
    window.localStorage.clear();
    appListUsers.mockResolvedValue({ ok: true, data: [] });
    vaultGetSessionToken.mockResolvedValue({ ok: true, data: 'token_test' });
    vaultStatus.mockResolvedValue({
      ok: true,
      data: { locked: true, vault_id: null, file_count: 0, storage_used: 0, vault_open: false },
    });
    // Dashboard (main UI) screens call these at mount.
    fileRecent.mockResolvedValue({ ok: true, data: [] });
    fileFavorites.mockResolvedValue({ ok: true, data: [] });
    vaultActivity.mockResolvedValue({ ok: true, data: undefined });
    autoLockSet.mockResolvedValue({ ok: true, data: undefined });
  });

  afterEach(() => {
    window.localStorage.clear();
  });

  it('cold start shows Login, not the vault screens', async () => {
    appStatus.mockResolvedValue({ ok: true, data: LOGGED_OUT });

    render(<App />);

    expect(await screen.findByText('Your Files. Your Privacy. Your Control.')).toBeInTheDocument();
    expect(screen.queryByText(/Create Vault/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Unlock Vault/i)).not.toBeInTheDocument();
  });

  it('successful login reaches the vault gate (Welcome)', async () => {
    appStatus.mockResolvedValue({ ok: true, data: LOGGED_OUT });
    appLogin.mockResolvedValue({ ok: true, data: LOGGED_IN });

    render(<App />);

    await screen.findByText('Your Files. Your Privacy. Your Control.');
    await user.type(screen.getByLabelText('Email'), 'b1865741@gmail.com');
    await user.type(screen.getByLabelText('Password'), 'Harry@12345');
    await user.click(screen.getByRole('button', { name: /Sign in/i }));

    expect(await screen.findByRole('heading', { name: /Create Vault/i })).toBeInTheDocument();
    expect(appLogin).toHaveBeenCalledWith('b1865741@gmail.com', 'Harry@12345');
  });

  it('signup validates length and confirmation client-side', async () => {
    appStatus.mockResolvedValue({ ok: true, data: LOGGED_OUT });

    render(<App />);
    await screen.findByText('Your Files. Your Privacy. Your Control.');
    await user.click(screen.getByRole('button', { name: /Sign up/i }));

    await user.type(screen.getByLabelText('Email'), 'new@example.com');
    await user.type(screen.getByLabelText('Password'), 'short');
    await user.type(screen.getByLabelText('Confirm password'), 'short');
    await user.click(screen.getByRole('button', { name: /Create account/i }));
    expect(await screen.findByText('Password must be at least 8 characters')).toBeInTheDocument();
    expect(appSignup).not.toHaveBeenCalled();

    await user.clear(screen.getByLabelText('Password'));
    await user.type(screen.getByLabelText('Password'), 'longenough');
    await user.type(screen.getByLabelText('Confirm password'), 'different');
    await user.click(screen.getByRole('button', { name: /Create account/i }));
    expect(await screen.findByText('Passwords do not match')).toBeInTheDocument();
    expect(appSignup).not.toHaveBeenCalled();
  });

  it('surfaces the server error for a duplicate email', async () => {
    appStatus.mockResolvedValue({ ok: true, data: LOGGED_OUT });
    appSignup.mockResolvedValue({
      ok: false,
      error: { type: 'Internal', message: 'Email already in use' },
    });

    render(<App />);
    await screen.findByText('Your Files. Your Privacy. Your Control.');
    await user.click(screen.getByRole('button', { name: /Sign up/i }));

    await user.type(screen.getByLabelText('Email'), 'dup@example.com');
    await user.type(screen.getByLabelText('Password'), 'longenough');
    await user.type(screen.getByLabelText('Confirm password'), 'longenough');
    await user.click(screen.getByRole('button', { name: /Create account/i }));

    expect(await screen.findByText('Email already in use')).toBeInTheDocument();
    expect(appSignup).toHaveBeenCalledWith('dup@example.com', 'longenough');
  });

  it('logout returns to Login WITHOUT locking the vault', async () => {
    // Layer 1 passed, vault already unlocked: main app visible.
    appStatus.mockResolvedValue({ ok: true, data: LOGGED_IN });
    vaultStatus.mockResolvedValue({
      ok: true,
      data: { locked: false, vault_id: 1, file_count: 0, storage_used: 0, vault_open: true },
    });
    appLogout.mockResolvedValue({ ok: true, data: undefined });

    render(<App />);

    await screen.findByRole('heading', { name: /Dashboard/i });
    await user.click(screen.getByRole('link', { name: 'Account' }));
    await user.click(await screen.findByRole('button', { name: /Log out/i }));

    expect(await screen.findByText('Your Files. Your Privacy. Your Control.')).toBeInTheDocument();
    expect(appLogout).toHaveBeenCalled();
    // Three-layer model: logout does NOT lock the vault.
    expect(vaultLock).not.toHaveBeenCalled();
  });
});
