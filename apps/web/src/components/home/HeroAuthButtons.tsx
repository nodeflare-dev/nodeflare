'use client';

import { useTranslations } from 'next-intl';
import { Button } from '@/components/ui/button';
import Link from 'next/link';

export function HeroAuthButtons() {
  const t = useTranslations('home');
  const tNav = useTranslations('nav');

  return (
    <div className="mt-8 flex flex-wrap justify-center gap-3">
      <Link href="/signup">
        <Button className="h-10 px-5 text-sm rounded-lg bg-violet-600 hover:bg-violet-700 text-white">
          {t('getStarted')}
        </Button>
      </Link>
      <Link href="/docs">
        <Button variant="outline" className="h-10 px-5 text-sm rounded-lg border-gray-400 text-gray-700 hover:bg-gray-50">
          {tNav('docs')}
        </Button>
      </Link>
    </div>
  );
}
