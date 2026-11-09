import { useState, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle, CardHeader } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { vaultUrl } from '@/lib/vaultUrl';

interface FileManagerProps {
  token: string | null;
  onLock: () => void;
}

const ALL_FILES = [
  { id: 'doc1', name: 'document.pdf', size: 1024 * 500, type: 'pdf' },
  { id: 'vid1', name: 'video.mp4', size: 1024 * 1024 * 100, type: 'video' },
  { id: 'img1', name: 'photo.jpg', size: 1024 * 2000, type: 'image' },
];

export default function FileManager({ token, onLock }: FileManagerProps) {
  const [files, setFiles] = useState<typeof ALL_FILES>([]);
  const [search, setSearch] = useState('');

  useEffect(() => {
    setFiles(ALL_FILES);
  }, [token]);

  const filteredFiles = files.filter((f) =>
    f.name.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="min-h-screen bg-slate-950 p-8">
      <div className="max-w-7xl mx-auto">
        <div className="flex items-center justify-between mb-8">
          <h1 className="text-2xl font-bold text-white">File Manager</h1>
          <div className="flex gap-4">
            <Input
              placeholder="Search files..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-64 bg-slate-800"
            />
            <Button onClick={onLock} variant="destructive">
              Lock Vault
            </Button>
          </div>
        </div>

        <Card>
          <CardHeader>
            <CardTitle>Files ({filteredFiles.length})</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-4 gap-4">
              {filteredFiles.map((file) => (
                <div key={file.id} className="bg-slate-900 border border-slate-800 rounded-lg p-4">
                  <div className="text-lg font-bold">{file.name}</div>
                  <div className="text-sm text-slate-400">{file.size} bytes</div>
                  <div className="text-xs text-slate-500 mt-2">{file.type}</div>
                  <div className="mt-4 flex gap-2">
                    {file.type === 'pdf' && token && (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          const url = vaultUrl(file.id, token);
                          window.open(url, '_blank');
                        }}
                      >
                        View PDF
                      </Button>
                    )}
                    {file.type === 'video' && token && (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          const url = vaultUrl(file.id, token);
                          window.open(url, '_blank');
                        }}
                      >
                        Play Video
                      </Button>
                    )}
                    {file.type === 'image' && token && (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => {
                          const url = vaultUrl(file.id, token);
                          window.open(url, '_blank');
                        }}
                      >
                        View Image
                      </Button>
                    )}
                  </div>
                </div>
              ))}
              {filteredFiles.length === 0 && (
                <div className="text-slate-400 text-center py-8">No files found</div>
              )}
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
