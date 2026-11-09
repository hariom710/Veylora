import { useState, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle } from '@/components/ui/card';
import { vaultStatus } from '@/lib/ipc';

interface DashboardProps {
  sessionToken: string | null;
}

export default function Dashboard({ sessionToken }: DashboardProps) {
  const [status, setStatus] = useState<any>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadStatus();
  }, []);

  async function loadStatus() {
    const { data, ok } = await vaultStatus();
    if (ok) setStatus(data);
    setLoading(false);
  }

  return (
    <div className="min-h-screen bg-slate-950 p-8">
      <div className="max-w-7xl mx-auto">
        <div className="flex items-center justify-between mb-8">
          <h1 className="text-2xl font-bold text-white">Dashboard</h1>
          <Button variant="outline" onClick={() => window.location.reload()}>
            Refresh
          </Button>
        </div>

        {loading ? (
          <div className="text-slate-400">Loading...</div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            <Card>
              <CardContent>
                <CardTitle>Vault Status</CardTitle>
                <p className="text-slate-400 text-sm">{status?.locked ? 'Locked' : 'Unlocked'}</p>
                <p className="text-slate-400 text-sm">Vault ID: {status?.vault_id ?? 'N/A'}</p>
                <p className="text-slate-400 text-sm">Files: {status?.file_count ?? 0}</p>
                <p className="text-slate-400 text-sm">Storage: {status?.storage_used ?? 0} bytes</p>
              </CardContent>
            </Card>
            <Card>
              <CardContent>
                <CardTitle>Quick Actions</CardTitle>
                <Button className="w-full">Import File</Button>
                <Button className="w-full mt-2" variant="outline">
                  Browse Files
                </Button>
              </CardContent>
            </Card>
          </div>
        )}
      </div>
    </div>
  );
}
