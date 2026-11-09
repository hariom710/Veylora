import { useState, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle } from '@/components/ui/card';
import { vaultUrl } from '@/lib/vaultUrl';

interface VideoPlayerProps {
  token: string | null;
  onLock: () => void;
}

export default function VideoPlayer({ token, onLock }: VideoPlayerProps) {
  return (
    <div className="min-h-screen bg-slate-950 p-8">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold text-white">Video Player</h1>
        <Button onClick={onLock} variant="destructive">Lock Vault</Button>
      </div>

      <Card>
        <CardContent className="p-0">
          <div className="h-[calc(100vh-200px)] w-full bg-black">
            <video
              className="w-full h-full"
              controls
              src={vaultUrl('unknown', token || '')}
            />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
