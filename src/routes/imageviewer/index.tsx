import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle } from '@/components/ui/card';
import { vaultUrl } from '@/lib/vaultUrl';

interface ImageViewerProps {
  token: string | null;
  onLock: () => void;
}

export default function ImageViewer({ token, onLock }: ImageViewerProps) {
  return (
    <div className="min-h-screen bg-slate-950 p-8">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">Image Viewer</h1>
        <Button onClick={onLock} variant="destructive">Lock Vault</Button>
      </div>

      <Card>
        <CardContent className="p-0">
          <div className="h-[calc(100vh-200px)] w-full bg-black flex items-center justify-center">
            <img
              className="max-w-full max-h-full object-contain"
              src={vaultUrl('unknown', token || '')}
              alt="Image"
            />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
