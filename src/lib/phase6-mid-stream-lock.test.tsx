import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import PDFViewer from '@/routes/pdfviewer';

vi.mock('@/lib/ipc', () => ({
  vaultLock: vi.fn(),
}));

describe('phase6_mid_stream_403_triggers_unlock_modal', () => {
  const onLock = vi.fn();

  beforeEach(() => {
    vi.resetAllMocks();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('403 mid-stream shows unlock modal and no partial plaintext', async () => {
    // Mock fetch to return 403 mid-stream for vault:// URL
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(null, { status: 403 })),
    );

    render(<PDFViewer token="token_1" onLock={onLock} />);

    // Wait for async fetch to complete and modal to appear
    await screen.findByText(/Vault locked/i);

    // Modal is present
    expect(
      screen.getByText(/Vault locked — unlock to continue/i),
    ).toBeInTheDocument();

    // onLock callback fired (navigation to unlock)
    expect(onLock).toHaveBeenCalled();

    // No partial plaintext rendered (no PDF Viewer heading)
    expect(screen.queryByText(/PDF Viewer/i)).not.toBeInTheDocument();
  });
});
