'use client';

import { useTranslations } from 'next-intl';
import { Button } from '@/components/ui/button';
import { XCircle } from 'lucide-react';
import Link from 'next/link';

export default function BillingCancelPage() {
  const t = useTranslations('billing.cancel');

  return (
    <div className="flex items-center justify-center min-h-[60vh] px-4">
      <div className="max-w-sm w-full text-center">
        <XCircle className="w-10 h-10 mx-auto text-muted-foreground/60 mb-6" strokeWidth={1.5} />

        <h1 className="text-xl font-bold mb-2 text-[#333333]">{t('title')}</h1>
        <p className="text-sm text-muted-foreground leading-relaxed">
          {t('description')}
        </p>

        <div className="my-8 h-px bg-border" />

        <p className="text-xs text-muted-foreground/80 mb-6">
          {t('help')}
        </p>

        <div className="flex gap-3 justify-center">
          <Button variant="ghost" size="sm" asChild>
            <Link href="/dashboard/billing">{t('backToBilling')}</Link>
          </Button>
          <Button size="sm" asChild>
            <Link href="/pricing">{t('viewPlans')}</Link>
          </Button>
        </div>
      </div>
    </div>
  );
}
