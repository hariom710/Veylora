import { useState } from 'react';
import { Button } from '@/components/ui/button';

// Shown exactly once, right after vault creation. The key itself is only in
// this component's props — App clears it as soon as Continue is pressed.
export default function RecoveryKey({
  recoveryKey,
  onContinue,
}: {
  recoveryKey: string;
  onContinue: () => void;
}) {
  const [saved, setSaved] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(recoveryKey);
      setMessage('Copied to clipboard');
    } catch {
      setMessage('Copy failed — select the key and copy manually');
    }
  }

  function handleDownload() {
    try {
      const blob = new Blob([recoveryKey + '\n'], { type: 'text/plain' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'veylora-recovery-key.txt';
      a.click();
      URL.revokeObjectURL(url);
      setMessage('Downloaded veylora-recovery-key.txt');
    } catch {
      setMessage('Download failed — use Copy instead')
    }
  }

  function handlePrint() {
    window.print();
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-background">
      <div className="w-full max-w-xl p-8 bg-surface border border-border rounded-card">
        <h1 className="text-title-1 mb-2">Your Recovery Key</h1>
        <p className="text-sm text-muted mb-6">
          This is shown exactly once. Write it down. Store it somewhere safe.
        </p>

        <div className="mb-4 p-4 rounded-input border border-danger/40 bg-danger/10 text-danger text-sm">
          If you lose this key AND your PIN AND your password, your files are
          gone forever. There is no way to reset.
        </div>

        <div
          className="mb-6 p-4 bg-background border border-border rounded-input select-all break-all text-mono text-foreground"
          data-testid="recovery-key"
        >
          {recoveryKey}
        </div>

        <div className="flex flex-wrap gap-3 mb-6">
          <Button variant="outline" onClick={handleCopy}>
            Copy to clipboard
          </Button>
          <Button variant="outline" onClick={handleDownload}>
            Download as .txt
          </Button>
          <Button variant="outline" onClick={handlePrint}>
            Print
          </Button>
        </div>

        {message && <p className="text-sm text-muted mb-4">{message}</p>}

        <label className="flex items-center gap-3 mb-6 text-sm text-foreground">
          <input
            type="checkbox"
            checked={saved}
            onChange={(e) => setSaved(e.target.checked)}
          />
          I have saved my recovery key
        </label>

        <Button onClick={onContinue} disabled={!saved} className="w-full">
          Continue
        </Button>
      </div>
    </div>
  );
}
