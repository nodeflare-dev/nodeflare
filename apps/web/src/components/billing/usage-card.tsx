'use client';

import { useQuery } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { api } from '@/lib/api';
import { Card } from '@/components/ui/card';

interface ServerUsage {
  server_id: string;
  name: string;
  gb_hours: number;
}

interface UsageResponse {
  plan: string;
  metered: boolean;
  total_gb_hours: number;
  rate_jpy_per_gb_hour: number;
  estimated_cost_jpy: number;
  servers: ServerUsage[];
}

/** Current-month memory-time usage (GB-hours) per server, with an estimated cost. */
export function UsageCard({ workspaceId }: { workspaceId?: string }) {
  const t = useTranslations('billing');

  const { data } = useQuery<UsageResponse>({
    queryKey: ['usage', workspaceId],
    queryFn: () => api.get(`/workspaces/${workspaceId}/billing/usage`),
    enabled: !!workspaceId,
  });

  if (!data) return null;

  const fmt = (n: number) => n.toFixed(2);
  const sorted = [...data.servers].sort((a, b) => b.gb_hours - a.gb_hours);

  return (
    <Card className="p-6 space-y-4">
      <div>
        <h3 className="text-lg font-semibold">{t('usageTitle')}</h3>
        <p className="text-sm text-muted-foreground">{t('usageDesc')}</p>
      </div>

      <div className="flex items-baseline justify-between border-b pb-3">
        <span className="text-sm text-muted-foreground">{t('usageTotal')}</span>
        <span className="text-xl font-semibold">{t('usageGbHours', { value: fmt(data.total_gb_hours) })}</span>
      </div>

      {sorted.length === 0 ? (
        <p className="text-sm text-muted-foreground">{t('usageNoData')}</p>
      ) : (
        <ul className="space-y-1.5">
          {sorted.map((s) => (
            <li key={s.server_id} className="flex items-center justify-between text-sm">
              <span className="truncate">{s.name}</span>
              <span className="font-mono text-muted-foreground">{t('usageGbHours', { value: fmt(s.gb_hours) })}</span>
            </li>
          ))}
        </ul>
      )}

      {data.metered ? (
        data.rate_jpy_per_gb_hour > 0 && (
          <div className="flex items-baseline justify-between border-t pt-3">
            <span className="text-sm text-muted-foreground">{t('usageEstimated')}</span>
            <span className="font-semibold">¥{Math.round(data.estimated_cost_jpy).toLocaleString()}</span>
          </div>
        )
      ) : (
        <p className="text-xs text-muted-foreground border-t pt-3">{t('usageFreeNote')}</p>
      )}
    </Card>
  );
}
