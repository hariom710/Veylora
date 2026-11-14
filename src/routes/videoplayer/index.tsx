import { useState } from 'react';
import { useParams } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { LockedModal } from '@/components/ui/locked-modal';
import { vaultUrl } from '@/lib/vaultUrl';
import { vaultFetch } from '@/lib/vaultFetch';

interface VideoPlayerProps {
  token: string | null;
  onLock: () => void;
}

export default function VideoPlayer({ token, onLock }: VideoPlayerProps) {
  // Hex blob id from /viewer/video/:fileId.
  const { fileId = '' } = useParams();
  const src = token && fileId ? vaultUrl(fileId, token) : undefined;
  const [isLocked, setIsLocked] = useState(false);
  const [failed, setFailed] = useState(false);

  // <video> cannot read status codes: on error, probe the URL — a 403 means
  // the vault locked mid-stream (same reaction as the PDF viewer), anything
  // else is a genuinely broken source.
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
        <h1 className="text-title-1">Video Player</h1>
        <Button onClick={onLock} variant="destructive">Lock Vault</Button>
      </div>

      <Card>
        <CardContent className="p-0">
          <div className="h-[calc(100vh-200px)] w-full bg-black">
            {src && !failed ? (
              <video
                className="w-full h-full"
                controls
                src={src}
                onError={handleLoadError}
              />
            ) : failed ? (
              <div className="w-full h-full flex items-center justify-center text-muted">
                Video failed to load
              </div>
            ) : (
              <div className="w-full h-full flex items-center justify-center text-muted">
                No file selected
              </div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
