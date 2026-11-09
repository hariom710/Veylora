import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import Unlock from './index';

vi.mock('@/lib/ipc', () => ({
  vaultUnlockPin: vi.fn(),
  vaultUnlockPassword: vi.fn(),
  vaultUnlockRecovery: vi.fn(),
  vaultGetSessionToken: vi.fn(),
}));

import {
  vaultUnlockPin,
  vaultUnlockPassword,
  vaultUnlockRecovery,
  vaultGetSessionToken,
} from '@/lib/ipc';

describe('Unlock screen', () => {
  const mockOnUnlock = vi.fn();

  beforeEach(() => {
    vi.resetAllMocks();
    vaultGetSessionToken.mockResolvedValue({ ok: true, data: 'token123' });
  });

  it('renders PIN input by default', () => {
    render(<Unlock onUnlock={mockOnUnlock} />);
    expect(screen.getByLabelText(/PIN/i)).toBeInTheDocument();
  });

  it('switches to password tab', async () => {
    const user = userEvent.setup();
    render(<Unlock onUnlock={mockOnUnlock} />);
    await user.click(screen.getByText('Password'));
    expect(screen.getByLabelText(/Password/i)).toBeInTheDocument();
  });

  it('switches to recovery tab', async () => {
    const user = userEvent.setup();
    render(<Unlock onUnlock={mockOnUnlock} />);
    await user.click(screen.getByText('Recovery'));
    expect(screen.getByLabelText(/Recovery Key/i)).toBeInTheDocument();
  });

  it('unlocks with valid PIN', async () => {
    const user = userEvent.setup();
    vaultUnlockPin.mockResolvedValue({ ok: true, data: { vault_id: 1, unlock_method: 'pin' } });
    render(<Unlock onUnlock={mockOnUnlock} />);

    await user.type(screen.getByLabelText(/PIN/i), '1234');
    await user.click(screen.getByRole('button', { name: /Unlock/i }));

    expect(mockOnUnlock).toHaveBeenCalledWith('pin', 'token123');
  });

  it('shows error for invalid PIN', async () => {
    const user = userEvent.setup();
    vaultUnlockPin.mockRejectedValue(new Error('invalid credential'));
    render(<Unlock onUnlock={mockOnUnlock} />);

    await user.type(screen.getByLabelText(/PIN/i), 'wrong');
    await user.click(screen.getByRole('button', { name: /Unlock/i }));

    expect(screen.getByText(/invalid credential/i)).toBeInTheDocument();
  });
});
