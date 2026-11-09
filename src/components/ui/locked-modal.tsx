import { Button } from './button';

export function LockedModal({ onUnlock }: { onUnlock: () => void }) {
  return (
    <div className="fixed inset-0 bg-slate-900/90 z-50 flex items-center justify-center">
      <div className="bg-slate-900 border border-slate-700 p-6 rounded-lg">
        <h2 className="text-white text-lg font-medium">
          Vault locked — unlock to continue
        </h2>
        <Button onClick={onUnlock} className="mt-4">
          Unlock
        </Button>
      </div>
    </div>
  );
}
