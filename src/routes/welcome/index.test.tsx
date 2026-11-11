import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Welcome from './index';

vi.mock('@/lib/ipc', () => ({
  vaultCreate: vi.fn(),
  vaultOpen: vi.fn(),
  vaultGetMeta: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

import { vaultOpen, vaultGetMeta } from '@/lib/ipc';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

const RECENT_KEY = 'veylora.recentVaults';
const VAULT_PATH_KEY = 'veylora.vaultPath';

const metaOk = (p: string) => ({
  ok: true,
  data: { name: p, path: p, version: 1, kdf_params_public: 'null' },
});

describe('Welcome — recent vaults and open flow', () => {
  const user = userEvent.setup();
  const onCreated = vi.fn();

  beforeEach(() => {
    vi.resetAllMocks();
    window.localStorage.clear();
    vaultGetMeta.mockImplementation((p: string) => Promise.resolve(metaOk(p)));
    vaultOpen.mockResolvedValue({ ok: true, data: { name: 'v', path: 'p', vault_id: 1 } });
    (openDialog as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  afterEach(() => {
    window.localStorage.clear();
  });

  it('shows no recent section when nothing is remembered', async () => {
    render(<Welcome onCreated={onCreated} />);

    expect(await screen.findByRole('heading', { name: /Create Vault/i })).toBeInTheDocument();
    expect(screen.queryByText('Open a vault')).not.toBeInTheDocument();
    expect(vaultOpen).not.toHaveBeenCalled();
    expect(vaultGetMeta).not.toHaveBeenCalled();
  });

  it('renders remembered vaults and opens the one that is clicked', async () => {
    window.localStorage.setItem(RECENT_KEY, JSON.stringify(['/v/alpha', '/v/beta']));

    render(<Welcome onCreated={onCreated} />);

    expect(await screen.findByText('Open a vault')).toBeInTheDocument();
    expect(screen.getByText('alpha')).toBeInTheDocument();
    expect(screen.getByText('beta')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /alpha/ }));
    expect(vaultOpen).toHaveBeenCalledWith('/v/alpha');
  });

  it('drops a stale vault from the list on mount and persists the pruned list', async () => {
    window.localStorage.setItem(RECENT_KEY, JSON.stringify(['/gone', '/kept']));
    vaultGetMeta.mockImplementation((p: string) =>
      Promise.resolve(
        p === '/kept' ? metaOk(p) : { ok: false, error: { message: 'vault not found' } },
      ),
    );

    render(<Welcome onCreated={onCreated} />);

    expect(await screen.findByText('kept')).toBeInTheDocument();
    expect(screen.queryByText('gone')).not.toBeInTheDocument();
    expect(JSON.parse(window.localStorage.getItem(RECENT_KEY) as string)).toEqual(['/kept']);
  });

  it('offers Browse on the Open form so a path is never typed', async () => {
    (openDialog as ReturnType<typeof vi.fn>).mockResolvedValue('C:\\Vaults\\Picked');

    render(<Welcome onCreated={onCreated} />);

    const field = (await screen.findByLabelText('Vault Path')) as HTMLInputElement;
    const browse = screen.getAllByRole('button', { name: /Browse/i });
    expect(browse).toHaveLength(2); // Create form + Open form

    await user.click(browse[1]);
    expect(openDialog).toHaveBeenCalledWith({ directory: true });
    expect(field.value).toBe('C:\\Vaults\\Picked');
  });

  it('prefills the Open field from the last used vault', async () => {
    window.localStorage.setItem(VAULT_PATH_KEY, 'C:\\Vaults\\LastOne');

    render(<Welcome onCreated={onCreated} />);

    const field = (await screen.findByLabelText('Vault Path')) as HTMLInputElement;
    expect(field.value).toBe('C:\\Vaults\\LastOne');
  });

  it('seeds the list from the remembered vault so the first run after update is not empty', async () => {
    window.localStorage.setItem(VAULT_PATH_KEY, 'C:\\Vaults\\Legacy');

    render(<Welcome onCreated={onCreated} />);

    expect(await screen.findByText('Legacy')).toBeInTheDocument();
    expect(JSON.parse(window.localStorage.getItem(RECENT_KEY) as string)).toEqual([
      'C:\\Vaults\\Legacy',
    ]);
  });

  it('remembers a manually opened vault, deduped and capped at five', async () => {
    window.localStorage.setItem(RECENT_KEY, JSON.stringify(['/a', '/b', '/c', '/d', '/e']));
    (openDialog as ReturnType<typeof vi.fn>).mockResolvedValue('/new');

    render(<Welcome onCreated={onCreated} />);

    const browse = screen.getAllByRole('button', { name: /Browse/i });
    await user.click(browse[1]);
    await user.click(screen.getByRole('button', { name: /^Open Vault$/ }));

    expect(vaultOpen).toHaveBeenCalledWith('/new');
    const list = JSON.parse(window.localStorage.getItem(RECENT_KEY) as string);
    expect(list[0]).toBe('/new');
    expect(list).toHaveLength(5);
  });

  it('removes a row that fails to open instead of leaving a dead button', async () => {
    window.localStorage.setItem(RECENT_KEY, JSON.stringify(['/broken']));
    vaultOpen.mockResolvedValue({ ok: false, error: { message: 'vault not found' } });

    render(<Welcome onCreated={onCreated} />);

    await user.click(await screen.findByRole('button', { name: /broken/ }));

    expect(screen.queryByText('Open a vault')).not.toBeInTheDocument();
    expect(JSON.parse(window.localStorage.getItem(RECENT_KEY) as string)).toEqual([]);
  });
});
