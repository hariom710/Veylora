import { BrowserRouter, Routes, Route, NavLink, useNavigate } from 'react-router-dom';
import { useState, useEffect } from 'react';
import Login from './routes/login';
import RecoveryKey from './routes/recovery';
import Welcome from './routes/welcome';
import Unlock from './routes/unlock';
import Dashboard from './routes/dashboard';
import FileManager from './routes/filemanager';
import Favorites from './routes/favorites';
import Recent from './routes/recent';
import PDFViewer from './routes/pdfviewer';
import VideoPlayer from './routes/videoplayer';
import ImageViewer from './routes/imageviewer';
import Backup from './routes/backup';
import Settings from './routes/settings';
import Account from './routes/account';
import {
  appStatus,
  vaultStatus,
  vaultOpen,
  vaultGetSessionToken,
  vaultLock,
  vaultActivity,
  autoLockSet,
} from '@/lib/ipc';
import { CommandError, AppStatus } from '@/lib/types';

// Last opened vault path. In-memory backend state (current_path) does not
// survive restart or lock, so the UI re-opens it before unlock.
const VAULT_PATH_KEY = 'veylora.vaultPath';

const LOGGED_OUT: AppStatus = {
  logged_in: false,
  email: null,
  role: null,
  logged_in_since: null,
  used_default_password: false,
};

function storedVaultPath(): string | null {
  try {
    return window.localStorage.getItem(VAULT_PATH_KEY);
  } catch {
    return null;
  }
}

function AppContent() {
  return (
    <BrowserRouter>
      <AppContentInner />
    </BrowserRouter>
  );
}

function AppContentInner() {
  const navigate = useNavigate();
  const [sessionToken, setSessionToken] = useState<string | null>(null);
  const [isLocked, setIsLocked] = useState(true);
  // Start true so the first paint is the Unlock screen; the status check
  // corrects it to Welcome (Create Vault) on fresh installs.
  const [hasOpenVault, setHasOpenVault] = useState(true);
  // Layer 1: app session is in-memory only, so every cold start begins
  // logged out (matches the three-layer model).
  const [app, setApp] = useState<AppStatus>(LOGGED_OUT);
  // Recovery key shown exactly once, right after vault creation. Held only
  // in memory; cleared on Continue (never persisted, never re-displayable).
  const [pendingRecovery, setPendingRecovery] = useState<string | null>(null);
  // Ctrl+/ cheat-sheet overlay (main app only).
  const [showHelp, setShowHelp] = useState(false);

  useEffect(() => {
    boot();
  }, []);

  async function boot() {
    const status = await appStatus();
    if (status.ok) setApp(status.data);

    // Re-apply the persisted auto-lock timeout to the backend watcher
    // (the backend default is 300s; Settings may have changed it).
    try {
      const saved = window.localStorage.getItem('veylora.autoLockSecs');
      if (saved != null) await autoLockSet(Number(saved));
    } catch {
      /* ignore */
    }

    // Never auto-open a remembered vault on cold start: the flow is
    // Login → Welcome (Create/Open) → Unlock, so a fresh launch must land on
    // Welcome. The stored path is opened explicitly by Welcome (Open) and by
    // handleLock (after a lock within the same session).
    await checkStatus();
  }

  async function checkStatus() {
    const { data, ok } = await vaultStatus();
    if (ok) {
      setHasOpenVault(data.vault_open);
      setIsLocked(data.locked);
      if (!data.locked && data.vault_id != null) {
        const { data: token } = await vaultGetSessionToken();
        if (token) setSessionToken(token);
      }
    }
  }

  // The Unlock screen already authenticated and fetched the session token;
  // re-unlocking here with the token as a credential would always fail and
  // strand the UI on the Unlock screen while the backend stays unlocked.
  function handleUnlock(_method: string, token: string) {
    if (token) setSessionToken(token);
    setIsLocked(false);
  }

  async function handleLock() {
    await vaultLock();
    // lock() clears the backend's current path; re-open so unlock works.
    const path = storedVaultPath();
    if (path) {
      await vaultOpen(path);
      setHasOpenVault(true);
    } else {
      setHasOpenVault(false);
    }
    setIsLocked(true);
    setSessionToken(null);
  }

  // Logout clears only Layer 1. The vault stays unlocked (three-layer model).
  function handleLogout() {
    setApp(LOGGED_OUT);
  }

  // vault_create already unlocked the session in the backend; we only need
  // the session token for vault:// URLs before entering the main app.
  function handleVaultCreated(recoveryKey: string) {
    setHasOpenVault(true);
    setPendingRecovery(recoveryKey);
  }

  async function handleRecoveryContinue() {
    setPendingRecovery(null);
    const { data: token } = await vaultGetSessionToken();
    if (token) setSessionToken(token);
    setIsLocked(false);
  }

  // Activity heartbeat: the backend auto-lock watcher locks after `timeout`
  // seconds of no activity, so the frontend reports real input (throttled to
  // one IPC call per minute). Only while unlocked.
  useEffect(() => {
    if (isLocked || pendingRecovery || !app.logged_in) return;
    let lastCall = 0;
    const report = () => {
      const now = Date.now();
      if (now - lastCall < 60_000) return;
      lastCall = now;
      vaultActivity().catch(() => {});
    };
    const events = ['mousemove', 'keydown', 'pointerdown'] as const;
    events.forEach((e) => window.addEventListener(e, report, { passive: true }));
    // Report immediately on unlock so the first minute starts active.
    report();
    return () => {
      events.forEach((e) => window.removeEventListener(e, report));
    };
  }, [isLocked, pendingRecovery, app.logged_in]);

  // Keyboard shortcuts (main app only — gate screens have nothing to do).
  useEffect(() => {
    if (isLocked || pendingRecovery || !app.logged_in) return;
    function onKey(e: KeyboardEvent) {
      if (!e.ctrlKey || e.metaKey) return;
      const target = e.target as HTMLElement | null;
      if (target && /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName)) return;
      const key = e.key.toLowerCase();
      if (key === 'l') {
        e.preventDefault();
        handleLock();
      } else if (key === 'o' || key === 'f') {
        // FileManager must be mounted before it can hear the event.
        e.preventDefault();
        navigate('/filemanager');
        setTimeout(() => {
          window.dispatchEvent(
            new CustomEvent(key === 'o' ? 'veylora:import' : 'veylora:focus-search'),
          );
        }, 150);
      } else if (key === ',') {
        e.preventDefault();
        navigate('/settings');
      } else if (key === '/') {
        e.preventDefault();
        setShowHelp((v) => !v);
      } else if (key === 'a' && e.shiftKey) {
        e.preventDefault();
        navigate('/account');
      } else if (key >= '1' && key <= '4') {
        e.preventDefault();
        navigate(['/', '/filemanager', '/backup', '/settings'][Number(key) - 1]);
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isLocked, pendingRecovery, app.logged_in]);

  // Layer 1 gate: app login sits in front of every vault screen.
  if (!app.logged_in) {
    return <Login onLogin={setApp} />;
  }

  // Shown exactly once after creation — before Unlock, because the vault is
  // already unlocked by vault_create.
  if (pendingRecovery) {
    return <RecoveryKey recoveryKey={pendingRecovery} onContinue={handleRecoveryContinue} />;
  }

  if (isLocked && !hasOpenVault) {
    return (
      <Welcome adminDefault={app.used_default_password} onCreated={handleVaultCreated} />
    );
  }

  if (isLocked) {
    return <Unlock onUnlock={handleUnlock} />;
  }

  return (
    <div className="min-h-screen bg-background text-foreground">
      <nav className="border-b border-border bg-surface/80 px-6 py-4 flex items-center justify-between">
        <div className="flex items-center gap-6">
          <h1 className="text-title-3 text-foreground">Veylora</h1>
          <NavLink
            to="/"
            className={({ isActive }) =>
              `text-body-sm font-medium transition-colors ${isActive ? 'text-accent' : 'text-muted hover:text-foreground'}`
            }
          >
            Dashboard
          </NavLink>
          <NavLink
            to="/filemanager"
            className={({ isActive }) =>
              `text-body-sm font-medium transition-colors ${isActive ? 'text-accent' : 'text-muted hover:text-foreground'}`
            }
          >
            Files
          </NavLink>
          <NavLink
            to="/favorites"
            className={({ isActive }) =>
              `text-body-sm font-medium transition-colors ${isActive ? 'text-accent' : 'text-muted hover:text-foreground'}`
            }
          >
            Favorites
          </NavLink>
          <NavLink
            to="/recent"
            className={({ isActive }) =>
              `text-body-sm font-medium transition-colors ${isActive ? 'text-accent' : 'text-muted hover:text-foreground'}`
            }
          >
            Recent
          </NavLink>
          <NavLink
            to="/backup"
            className={({ isActive }) =>
              `text-body-sm font-medium transition-colors ${isActive ? 'text-accent' : 'text-muted hover:text-foreground'}`
            }
          >
            Backup
          </NavLink>
          <NavLink
            to="/settings"
            className={({ isActive }) =>
              `text-body-sm font-medium transition-colors ${isActive ? 'text-accent' : 'text-muted hover:text-foreground'}`
            }
          >
            Settings
          </NavLink>
          <NavLink
            to="/account"
            className={({ isActive }) =>
              `text-body-sm font-medium transition-colors ${isActive ? 'text-accent' : 'text-muted hover:text-foreground'}`
            }
          >
            Account
          </NavLink>
        </div>
        <div className="flex items-center gap-4">
          <button
            onClick={handleLock}
            className="text-body-sm font-medium text-muted hover:text-danger transition-colors"
          >
            Lock Vault
          </button>
        </div>
      </nav>

      <Routes>
        <Route path="/" element={<Dashboard sessionToken={sessionToken} />} />
        <Route path="/welcome" element={<Welcome onCreated={handleVaultCreated} />} />
        <Route
          path="/filemanager"
          element={<FileManager token={sessionToken} onLock={handleLock} />}
        />
        <Route path="/favorites" element={<Favorites token={sessionToken} />} />
        <Route path="/recent" element={<Recent token={sessionToken} />} />
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
        <Route path="/account" element={<Account app={app} onLogout={handleLogout} />} />
      </Routes>

      {/* Shortcut cheat-sheet — Ctrl+/ toggles it. */}
      {showHelp && (
        <div
          className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center"
          onClick={() => setShowHelp(false)}
        >
          <div
            role="dialog"
            aria-label="Keyboard shortcuts"
            className="w-96 max-w-[90vw] bg-surface border border-border rounded-card p-6"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-title-3 text-foreground">Keyboard shortcuts</h2>
              <button
                onClick={() => setShowHelp(false)}
                aria-label="Close shortcuts"
                className="text-muted hover:text-foreground"
              >
                ✕
              </button>
            </div>
            <dl className="space-y-2 text-sm">
              {[
                ['Ctrl + /', 'Show / hide this list'],
                ['Ctrl + L', 'Lock vault'],
                ['Ctrl + O', 'Import files'],
                ['Ctrl + F', 'Search files'],
                ['Ctrl + ,', 'Settings'],
                ['Ctrl + Shift + A', 'Account'],
                ['Ctrl + 1…4', 'Dashboard / Files / Backup / Settings'],
              ].map(([keys, desc]) => (
                <div key={keys} className="flex items-center justify-between gap-4">
                  <dt className="font-mono text-[13px] text-accent">{keys}</dt>
                  <dd className="text-muted text-right">{desc}</dd>
                </div>
              ))}
            </dl>
          </div>
        </div>
      )}
    </div>
  );
}

export default function App() {
  return <AppContent />;
}
