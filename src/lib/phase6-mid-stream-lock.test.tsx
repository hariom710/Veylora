import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import PDFViewer from '@/routes/pdfviewer';
import ImageViewer from '@/routes/imageviewer';
import VideoPlayer from '@/routes/videoplayer';

vi.mock('@/lib/ipc', () => ({
  vaultLock: vi.fn(),
}));

// The image/video viewers gate on the :fileId route param, so they need a
// real route to render their media element at all.
function renderAt(routePath: string, initial: string, element: React.ReactElement) {
  return render(
    <MemoryRouter initialEntries={[initial]}>
      <Routes>
        <Route path={routePath} element={element} />
      </Routes>
    </MemoryRouter>,
  );
}

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

  it('image 403 shows the unlock modal instead of a broken image', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(null, { status: 403 })),
    );

    renderAt('/viewer/image/:fileId', '/viewer/image/abc123', (
      <ImageViewer token="token_1" onLock={onLock} />
    ));
    fireEvent.error(screen.getByRole('img'));

    await screen.findByText(/Vault locked/i);
    expect(onLock).toHaveBeenCalled();
    expect(screen.queryByText(/Image Viewer/i)).not.toBeInTheDocument();
  });

  it('image load failure that is not 403 is a broken image, not a lock', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(null, { status: 404 })),
    );

    renderAt('/viewer/image/:fileId', '/viewer/image/abc123', (
      <ImageViewer token="token_1" onLock={onLock} />
    ));
    fireEvent.error(screen.getByRole('img'));

    await screen.findByText(/Image failed to load/i);
    expect(onLock).not.toHaveBeenCalled();
    expect(screen.queryByText(/Vault locked/i)).not.toBeInTheDocument();
  });

  it('video 403 mid-stream shows the unlock modal', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(null, { status: 403 })),
    );

    const { container } = renderAt('/viewer/video/:fileId', '/viewer/video/abc123', (
      <VideoPlayer token="token_1" onLock={onLock} />
    ));
    const video = container.querySelector('video');
    expect(video).not.toBeNull();
    fireEvent.error(video!);

    await screen.findByText(/Vault locked/i);
    expect(onLock).toHaveBeenCalled();
    expect(screen.queryByText(/Video Player/i)).not.toBeInTheDocument();
  });
});
