import { useState, useEffect } from 'react';
import { useParams } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle } from '@/components/ui/card';
import { vaultUrl } from '@/lib/vaultUrl';
import { vaultFetch } from '@/lib/vaultFetch';
import { LockedModal } from '@/components/ui/locked-modal';

interface PDFViewerProps {
  token: string | null;
  onLock: () => void;
}

export default function PDFViewer({ token, onLock }: PDFViewerProps) {
  const [error, setError] = useState<string | null>(null);
  // Hex blob id from /viewer/pdf/:fileId (File Manager navigation).
  const { fileId = '' } = useParams();
  const [isLocked, setIsLocked] = useState(false);

  useEffect(() => {
    if (!token) return;
    const url = vaultUrl(fileId || 'unknown', token);
    vaultFetch(url, () => {
      setIsLocked(true);
      onLock();
    }).catch(() => {
      // Ignore network errors (e.g., unsupported vault:// scheme in tests)
    });
  }, [token, fileId, onLock]);

  if (isLocked) return <LockedModal onUnlock={onLock} />;
  if (!token) return <div className="text-muted">No session token</div>;

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-title-1">PDF Viewer</h1>
        <Button onClick={onLock} variant="destructive">
          Lock Vault
        </Button>
      </div>

      {error && (
        <div className="bg-danger/10 border border-danger/50 p-4 rounded-card text-danger">
          {error}
        </div>
      )}

      <Card>
        <CardContent className="p-0">
          <div className="h-[calc(100vh-200px)] w-full bg-surface">
            <iframe
              className="w-full h-full"
              src={vaultUrl(fileId || 'unknown', token)}
              title="PDF Viewer"
            />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
