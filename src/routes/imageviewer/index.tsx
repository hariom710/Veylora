import { useParams } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import { vaultUrl } from '@/lib/vaultUrl';

interface ImageViewerProps {
  token: string | null;
  onLock: () => void;
}

export default function ImageViewer({ token, onLock }: ImageViewerProps) {
  // Hex blob id from /viewer/image/:fileId.
  const { fileId = '' } = useParams();
  const src = token && fileId ? vaultUrl(fileId, token) : undefined;
  return (
    <div className="min-h-screen bg-background p-8">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-title-1">Image Viewer</h1>
        <Button onClick={onLock} variant="destructive">Lock Vault</Button>
      </div>

      <Card>
        <CardContent className="p-0">
          <div className="h-[calc(100vh-200px)] w-full bg-black flex items-center justify-center">
            {src ? (
              <img className="max-w-full max-h-full object-contain" src={src} alt="Image" />
            ) : (
              <div className="text-muted">No file selected</div>
            )}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
