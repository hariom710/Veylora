import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { useState, useEffect } from 'react';
import Welcome from './routes/welcome';
import Unlock from './routes/unlock';
import Dashboard from './routes/dashboard';
import FileManager from './routes/filemanager';
import PDFViewer from './routes/pdfviewer';
import VideoPlayer from './routes/videoplayer';
import ImageViewer from './routes/imageviewer';
import Backup from './routes/backup';
import Settings from './routes/settings';
import {
  vaultStatus,
  vaultGetSessionToken,
  vaultUnlockPin,
  vaultUnlockPassword,
  vaultUnlockRecovery,
  vaultLock,
} from '@/lib/ipc';
import { CommandError } from '@/lib/types';

function AppContent() {
  const [sessionToken, setSessionToken] = useState<string | null>(null);
  const [isLocked, setIsLocked] = useState(true);

  useEffect(() => {
    checkStatus();
  }, []);

  async function checkStatus() {
    const { data, ok } = await vaultStatus();
    if (ok) {
      setIsLocked(data.locked);
      if (!data.locked && data.vault_id != null) {
        const { data: token } = await vaultGetSessionToken();
        if (token) setSessionToken(token);
      }
    }
  }

  async function handleUnlock(method: 'pin' | 'password' | 'recovery', value: string) {
    let result;
    if (method === 'pin') result = await vaultUnlockPin(value);
    else if (method === 'password') result = await vaultUnlockPassword(value);
    else result = await vaultUnlockRecovery(value);

    if (result.ok) {
      const { data: token } = await vaultGetSessionToken();
      setSessionToken(token);
      setIsLocked(false);
    }
  }

  async function handleLock() {
    await vaultLock();
    setIsLocked(true);
    setSessionToken(null);
  }

  return (
    <BrowserRouter>
      <AppContentInner />
    </BrowserRouter>
  );
}

function AppContentInner() {
  const [sessionToken, setSessionToken] = useState<string | null>(null);
  const [isLocked, setIsLocked] = useState(true);

  useEffect(() => {
    checkStatus();
  }, []);

  async function checkStatus() {
    const { data, ok } = await vaultStatus();
    if (ok) {
      setIsLocked(data.locked);
      if (!data.locked && data.vault_id != null) {
        const { data: token } = await vaultGetSessionToken();
        if (token) setSessionToken(token);
      }
    }
  }

  async function handleUnlock(method: 'pin' | 'password' | 'recovery', value: string) {
    let result;
    if (method === 'pin') result = await vaultUnlockPin(value);
    else if (method === 'password') result = await vaultUnlockPassword(value);
    else result = await vaultUnlockRecovery(value);

    if (result.ok) {
      const { data: token } = await vaultGetSessionToken();
      setSessionToken(token);
      setIsLocked(false);
    }
  }

  async function handleLock() {
    await vaultLock();
    setIsLocked(true);
    setSessionToken(null);
  }

  if (isLocked) {
    return <Unlock onUnlock={handleUnlock} />;
  }

  return (
    <div className="min-h-screen bg-slate-950 text-slate-50">
      <nav className="border-b border-slate-800 bg-slate-900/80 px-6 py-4 flex items-center justify-between">
        <h1 className="text-xl font-bold text-primary">Veylora</h1>
        <div className="flex items-center gap-4">
          <button
            onClick={handleLock}
            className="text-sm text-slate-400 hover:text-white transition-colors"
          >
            Lock Vault
          </button>
        </div>
      </nav>

      <Routes>
        <Route path="/" element={<Dashboard sessionToken={sessionToken} />} />
        <Route path="/welcome" element={<Welcome />} />
        <Route
          path="/filemanager"
          element={<FileManager token={sessionToken} onLock={handleLock} />}
        />
        <Route
          path="/viewer/pdf/:fileId"
          element={<PDFViewer token={sessionToken} onLock={handleLock} />}
        />
        <Route
          path="/viewer/video/:fileId"
          element={<VideoPlayer token={sessionToken} onLock={handleLock} />}
        />
        <Route
          path="/viewer/image/:fileId"
          element={<ImageViewer token={sessionToken} onLock={handleLock} />}
        />
        <Route path="/backup" element={<Backup />} />
        <Route path="/settings" element={<Settings />} />
      </Routes>
    </div>
  );
}

export default function App() {
  return <AppContent />;
}
