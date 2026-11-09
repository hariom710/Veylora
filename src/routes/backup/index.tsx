import { Button } from '@/components/ui/button';
import { Card, CardContent, CardTitle } from '@/components/ui/card';

export default function Backup() {
  return (
    <div className="min-h-screen bg-slate-950 p-8">
      <div className="max-w-md mx-auto">
        <Card>
          <CardHeader>
            <CardTitle>Backup & Restore</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-slate-400 text-sm mb-6">
              Backup functionality will be available in Phase 7.
            </p>
            <Button disabled className="w-full">
              Create Backup
            </Button>
            <Button disabled className="w-full mt-2" variant="outline">
              Restore from Backup
            </Button>
            <Button disabled className="w-full mt-2" variant="outline">
              View History
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
