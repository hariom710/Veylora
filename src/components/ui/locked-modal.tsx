import { Button } from './button';

export function LockedModal({ onUnlock }: { onUnlock: () => void }) {
  return (
    <div className="fixed inset-0 bg-black/70 z-50 flex items-center justify-center">
      <div className="bg-elevated border border-border p-6 rounded-dialog shadow-modal">
        <h2 className="text-foreground text-title-3">
          Vault locked — unlock to continue
        </h2>
        <Button onClick={onUnlock} className="mt-4">
          Unlock
        </Button>
      </div>
    </div>
  );
}
