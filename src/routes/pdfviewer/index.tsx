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
  if (!token) return <div className="text-slate-400">No session token</div>;

  return (
    <div className="min-h-screen bg-slate-950 p-8">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">PDF Viewer</h1>
        <Button onClick={onLock} variant="destructive">
          Lock Vault
        </Button>
      </div>

      {error && (
        <div className="bg-destructive/10 border border-destructive/50 p-4 rounded-lg text-destructive">
          {error}
        </div>
      )}

      <Card>
        <CardContent className="p-0">
          <div className="h-[calc(100vh-200px)] w-full bg-slate-900">
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
