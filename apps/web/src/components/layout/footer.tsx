'use client';

import Link from 'next/link';
import { useTranslations } from 'next-intl';

export function Footer() {
  const t = useTranslations('footer');

  return (
    <footer className="bg-gray-950 border-t border-gray-800 py-16">
      <div className="max-w-6xl mx-auto px-4 sm:px-6">
        <div className="grid md:grid-cols-4 gap-12 mb-12">
          <div>
            <div className="flex items-center gap-2 mb-4">
              <img src="/logo.png" alt="Nodeflare" className="h-8 w-auto" />
              <span className="font-black text-white">NodeFlare</span>
            </div>
            <p className="text-gray-400 text-sm">
              {t('tagline')}
            </p>
          </div>

          <div>
            <h4 className="font-semibold text-white mb-4">{t('product')}</h4>
            <ul className="space-y-2 text-gray-400 text-sm">
              <li><Link href="/docs" className="hover:text-white transition-colors">{t('docs')}</Link></li>
              <li><Link href="/pricing" className="hover:text-white transition-colors">{t('pricing')}</Link></li>
              <li><Link href="/blog" className="hover:text-white transition-colors">{t('blog')}</Link></li>
            </ul>
          </div>

          <div>
            <h4 className="font-semibold text-white mb-4">{t('support')}</h4>
            <ul className="space-y-2 text-gray-400 text-sm">
              <li><Link href="/faq" className="hover:text-white transition-colors">{t('faq')}</Link></li>
              <li><Link href="/contact" className="hover:text-white transition-colors">{t('contact')}</Link></li>
            </ul>
          </div>

          <div>
            <h4 className="font-semibold text-white mb-4">{t('legal')}</h4>
            <ul className="space-y-2 text-gray-400 text-sm">
              <li><Link href="/legal/terms" className="hover:text-white transition-colors">{t('terms')}</Link></li>
              <li><Link href="/legal/privacy" className="hover:text-white transition-colors">{t('privacy')}</Link></li>
              <li><Link href="/legal/commerce" className="hover:text-white transition-colors">{t('commerce')}</Link></li>
            </ul>
          </div>
        </div>

        <div className="pt-8 border-t border-gray-800 text-center text-gray-500 text-sm">
          {t('copyright', { year: new Date().getFullYear() })}
        </div>
      </div>
    </footer>
  );
}
