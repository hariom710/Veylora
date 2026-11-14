import { useState } from 'react';
import { useParams } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { LockedModal } from '@/components/ui/locked-modal';
import { vaultUrl } from '@/lib/vaultUrl';
import { vaultFetch } from '@/lib/vaultFetch';

interface ImageViewerProps {
  token: string | null;
  onLock: () => void;
}

export default function ImageViewer({ token, onLock }: ImageViewerProps) {
  // Hex blob id from /viewer/image/:fileId.
  const { fileId = '' } = useParams();
  const src = token && fileId ? vaultUrl(fileId, token) : undefined;
  const [isLocked, setIsLocked] = useState(false);
  const [failed, setFailed] = useState(false);

  // <img> cannot read status codes: on error, probe the URL — a 403 means
  // the vault locked mid-view (same reaction as the PDF viewer), anything
  // else is a genuinely broken image.
  function handleLoadError() {
    if (!src || isLocked || failed) return;
    vaultFetch(src, () => {
      setIsLocked(true);
      onLock();
    })
      .then((r) => {
        if (r.status !== 403) setFailed(true);
      })
      .catch(() => setFailed(true));
  }

  if (isLocked) return <LockedModal onUnlock={onLock} />;

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-title-1">Image Viewer</h1>
        <Button onClick={onLock} variant="destructive">Lock Vault</Button>
      </div>

      <Card>
        <CardContent className="p-0">
          <div className="h-[calc(100vh-200px)] w-full bg-black flex items-center justify-center">
            {src && !failed ? (
              <img
                className="max-w-full max-h-full object-contain"
                src={src}
                alt="Image"
                onError={handleLoadError}
              />
            ) : failed ? (
              <div className="text-muted">Image failed to load</div>
            ) : (
              <div className="text-muted">No file selected</div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
