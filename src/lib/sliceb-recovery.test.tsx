import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import App from '@/App';

vi.mock('@/lib/ipc', () => ({
  appStatus: vi.fn(),
  appLogin: vi.fn(),
  appSignup: vi.fn(),
  appLogout: vi.fn(),
  appListUsers: vi.fn(),
  vaultStatus: vi.fn(),
  vaultOpen: vi.fn(),
  vaultGetMeta: vi.fn(),
  vaultCreate: vi.fn(),
  vaultGetSessionToken: vi.fn(),
  vaultUnlockPin: vi.fn(),
  vaultUnlockPassword: vi.fn(),
  vaultUnlockRecovery: vi.fn(),
  vaultLock: vi.fn(),
  vaultActivity: vi.fn(),
  autoLockSet: vi.fn(),
  fileImport: vi.fn(),
  fileList: vi.fn(),
  fileExport: vi.fn(),
  fileDelete: vi.fn(),
  fileRename: vi.fn(),
  fileFavorite: vi.fn(),
  fileFavorites: vi.fn(),
  fileRecent: vi.fn(),
  folderCreate: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

import {
  appStatus,
  appLogin,
  vaultStatus,
  vaultCreate,
  vaultGetSessionToken,
  fileRecent,
  fileFavorites,
  vaultActivity,
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

const RECOVERY_KEY = 'ABCDEFGH234567ABCDEFGH234567ABCDEFGH234567ABCDEFGH234567ABCD';

async function fillCreateForm(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText('Vault Name'), 'Personal');
  await user.type(screen.getByLabelText('Storage Location'), 'C:\\Vaults\\Personal');
  await user.type(screen.getByLabelText(/PIN/i), '1234');
  await user.type(screen.getByLabelText('Password', { exact: true }), 'password123');
  await user.type(screen.getByLabelText('Confirm Password'), 'password123');
}

describe('Slice B: create vault → recovery key → dashboard', () => {
  const user = userEvent.setup();

  beforeEach(() => {
    vi.resetAllMocks();
    window.localStorage.clear();
    appStatus.mockResolvedValue({ ok: true, data: LOGGED_OUT });
    appLogin.mockResolvedValue({ ok: true, data: LOGGED_IN });
    // Fresh install: no vault open yet.
    vaultStatus.mockResolvedValue({
      ok: true,
      data: { locked: true, vault_id: null, file_count: 0, storage_used: 0, vault_open: false },
    });
    vaultGetSessionToken.mockResolvedValue({ ok: true, data: 'token_1' });
    fileRecent.mockResolvedValue({ ok: true, data: [] });
    fileFavorites.mockResolvedValue({ ok: true, data: [] });
    vaultActivity.mockResolvedValue({ ok: true, data: undefined });
  });

  afterEach(() => {
    window.localStorage.clear();
  });

  it('shows the recovery key exactly once after creation, gated by the checkbox', async () => {
    vaultCreate.mockResolvedValue({
      ok: true,
      data: { name: 'Personal', path: 'C:\\Vaults\\Personal', vault_id: 1, recovery_key: RECOVERY_KEY },
    });

    render(<App />);

    // Layer 1: log in first.
    await screen.findByText('Your Files. Your Privacy. Your Control.');
    await user.type(screen.getByLabelText('Email'), 'b1865741@gmail.com');
    await user.type(screen.getByLabelText('Password'), 'password123');
    await user.click(screen.getByRole('button', { name: /Sign in/i }));

    // Layer 2: Welcome → Create Vault form.
    await screen.findByRole('heading', { name: 'Create Vault' });
    await fillCreateForm(user);
    await user.click(screen.getByRole('button', { name: /Create Vault/i }));

    // Recovery Key screen shows the key.
    expect(await screen.findByRole('heading', { name: /Your Recovery Key/i })).toBeInTheDocument();
    expect(screen.getByTestId('recovery-key')).toHaveTextContent(RECOVERY_KEY);
    expect(vaultCreate).toHaveBeenCalledWith(
      'Personal',
      'C:\\Vaults\\Personal',
      '1234',
      'password123',
    );

    // Continue is disabled until the checkbox is checked.
    const continueButton = screen.getByRole('button', { name: 'Continue' });
    expect(continueButton).toBeDisabled();
    await user.click(screen.getByRole('checkbox'));
    expect(continueButton).toBeEnabled();

    // Continue → Dashboard (vault already unlocked by vault_create).
    await user.click(continueButton);
    expect(await screen.findByRole('heading', { name: /Dashboard/i })).toBeInTheDocument();
    expect(vaultGetSessionToken).toHaveBeenCalled();
    // The key is gone from the UI after Continue.
    expect(screen.queryByTestId('recovery-key')).not.toBeInTheDocument();
  }, 10_000);

  it('rejects mismatched passwords client-side without calling the backend', async () => {
    render(<App />);
    await screen.findByText('Your Files. Your Privacy. Your Control.');
    await user.type(screen.getByLabelText('Email'), 'b1865741@gmail.com');
    await user.type(screen.getByLabelText('Password'), 'password123');
    await user.click(screen.getByRole('button', { name: /Sign in/i }));

    await screen.findByRole('heading', { name: 'Create Vault' });
    await user.type(screen.getByLabelText('Vault Name'), 'Personal');
    await user.type(screen.getByLabelText('Storage Location'), 'C:\\Vaults\\Personal');
    await user.type(screen.getByLabelText(/PIN/i), '1234');
    await user.type(screen.getByLabelText('Password', { exact: true }), 'password123');
    await user.type(screen.getByLabelText('Confirm Password'), 'different12');
    await user.click(screen.getByRole('button', { name: /Create Vault/i }));

    expect(await screen.findByText('Passwords do not match')).toBeInTheDocument();
    expect(vaultCreate).not.toHaveBeenCalled();
  }, 10_000);

  it('rejects a PIN that is not 4-6 digits client-side', async () => {
    render(<App />);
    await screen.findByText('Your Files. Your Privacy. Your Control.');
    await user.type(screen.getByLabelText('Email'), 'b1865741@gmail.com');
    await user.type(screen.getByLabelText('Password'), 'password123');
    await user.click(screen.getByRole('button', { name: /Sign in/i }));

    await screen.findByRole('heading', { name: 'Create Vault' });
    await user.type(screen.getByLabelText('Vault Name'), 'Personal');
    await user.type(screen.getByLabelText('Storage Location'), 'C:\\Vaults\\Personal');
    await user.type(screen.getByLabelText(/PIN/i), '12');
    await user.type(screen.getByLabelText('Password', { exact: true }), 'password123');
    await user.type(screen.getByLabelText('Confirm Password'), 'password123');
    await user.click(screen.getByRole('button', { name: /Create Vault/i }));

    expect(await screen.findByText('PIN must be 4-6 digits')).toBeInTheDocument();
    expect(vaultCreate).not.toHaveBeenCalled();
  }, 10_000);
});
