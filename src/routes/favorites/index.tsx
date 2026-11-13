import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle, CardHeader } from '@/components/ui/card';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';
import { fileFavorites, fileFavorite, fileExport } from '@/lib/ipc';
import { formatBytes, extOf } from '@/lib/format';
import { commandErrorMessage, type FileInfo } from '@/lib/types';

const VIEWER_BY_EXT: Record<string, 'pdf' | 'video' | 'image'> = {
  pdf: 'pdf',
  mp4: 'video',
  mkv: 'video',
  webm: 'video',
  mov: 'video',
  jpg: 'image',
  jpeg: 'image',
  png: 'image',
  gif: 'image',
  webp: 'image',
  bmp: 'image',
};

export default function Favorites({ token }: { token: string | null }) {
  const navigate = useNavigate();
  const [files, setFiles] = useState<FileInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  async function load() {
    const result = await fileFavorites();
    if (result.ok) setFiles(result.data);
    else setError(commandErrorMessage(result.error) || 'Failed to load favorites');
  }

  useEffect(() => {
    load();
  }, []);

  function openFile(file: FileInfo) {
    const kind = VIEWER_BY_EXT[extOf(file.filename)];
    if (token && kind) navigate(`/viewer/${kind}/${file.file_id}`);
  }

  async function handleUnstar(file: FileInfo) {
    const result = await fileFavorite(file.id, false);
    if (result.ok) await load();
    else setError(commandErrorMessage(result.error) || 'Failed to unstar');
  }

  async function handleExport(file: FileInfo) {
    const confirmed = window.confirm(
      'Exporting will write an unencrypted copy to disk. Anyone with access to that disk can read it. Continue?',
    );
    if (!confirmed) return;
    const dest = await saveDialog({ defaultPath: file.filename });
    if (!dest) return;
    const result = await fileExport(file.id, dest);
    if (result.ok) setMessage(`Exported to ${dest} (unencrypted copy)`);
    else setError(commandErrorMessage(result.error) || 'Export failed');
  }

  return (
    <div className="min-h-screen bg-background p-8">
      <div className="max-w-7xl mx-auto">
        <h1 className="text-title-1 mb-6">Favorites</h1>

        {error && (
          <div className="mb-4 text-sm text-danger bg-danger/10 p-3 rounded-input">
            {error}
          </div>
        )}
        {message && (
          <div className="mb-4 text-sm text-success bg-success/10 p-3 rounded-input">{message}</div>
        )}

        <Card>
          <CardHeader>
            <CardTitle>Starred files ({files.length})</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-4 gap-4">
              {files.map((file) => (
                <div
                  key={file.id}
                  className="bg-surface border border-border rounded-card p-4"
                >
                  <div className="font-bold text-foreground break-all">{file.filename}</div>
                  <div className="text-sm text-muted">{formatBytes(file.encrypted_size)}</div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    {token && VIEWER_BY_EXT[extOf(file.filename)] && (
                      <Button size="sm" variant="outline" onClick={() => openFile(file)}>
                        Open
                      </Button>
                    )}
                    <Button size="sm" variant="outline" onClick={() => handleExport(file)}>
                      Export
                    </Button>
                    <Button size="sm" variant="destructive" onClick={() => handleUnstar(file)}>
                      Unstar
                    </Button>
                  </div>
                </div>
              ))}
              {files.length === 0 && (
                <div className="text-muted text-center py-8 col-span-full">
                  No favorites yet — star files in the File Manager
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
