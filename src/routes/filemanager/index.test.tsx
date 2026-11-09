import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import FileManager from './index';

vi.mock('@/lib/vaultUrl', () => ({
  vaultUrl: vi.fn((fileId: string, token: string) => `vault://file/${fileId}?token=${token}`),
}));

describe('FileManager', () => {
  it('renders file list', () => {
    render(<FileManager token="abc123" onLock={vi.fn()} />);
    expect(screen.getByText(/File Manager/i)).toBeInTheDocument();
    expect(screen.getByText(/document\.pdf/i)).toBeInTheDocument();
    expect(screen.getByText(/video\.mp4/i)).toBeInTheDocument();
    expect(screen.getByText(/photo\.jpg/i)).toBeInTheDocument();
  });

  it('filters files by name', async () => {
    const user = userEvent.setup();
    render(<FileManager token="abc123" onLock={vi.fn()} />);
    await user.type(screen.getByPlaceholderText(/Search files/i), 'video');
    expect(screen.getByText(/video\.mp4/i)).toBeInTheDocument();
    expect(screen.queryByText(/document\.pdf/i)).not.toBeInTheDocument();
  });

  it('calls onLock when Lock Vault button clicked', async () => {
    const user = userEvent.setup();
    const onLock = vi.fn();
    render(<FileManager token="abc123" onLock={onLock} />);
    await user.click(screen.getByRole('button', { name: /Lock Vault/i }));
    expect(onLock).toHaveBeenCalled();
  });
});
