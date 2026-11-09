import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import PDFViewer from './index';

describe('PDF Viewer', () => {
  it('renders with token', () => {
    render(<PDFViewer token="abc123" onLock={vi.fn()} />);
    expect(screen.getByText(/PDF Viewer/i)).toBeInTheDocument();
  });

  it('renders without token', () => {
    render(<PDFViewer token={null} onLock={vi.fn()} />);
    expect(screen.getByText(/No session token/i)).toBeInTheDocument();
  });
});
