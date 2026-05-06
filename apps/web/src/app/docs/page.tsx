'use client';

import { useState, useEffect, useRef } from 'react';
import { useTranslations } from 'next-intl';
import { Header, Footer } from '@/components/layout';
import Link from 'next/link';
import Image from 'next/image';

type SectionKey =
  | 'dashboard'
  | 'servers'
  | 'serverNew'
  | 'serverDetail'
  | 'logs'
  | 'accessTokens'
  | 'oauth'
  | 'team'
  | 'billing'
  | 'settings';

const sectionIds: { id: string; key: SectionKey }[] = [
  { id: 'dashboard', key: 'dashboard' },
  { id: 'servers', key: 'servers' },
  { id: 'server-new', key: 'serverNew' },
  { id: 'server-detail', key: 'serverDetail' },
  { id: 'logs', key: 'logs' },
  { id: 'access-tokens', key: 'accessTokens' },
  { id: 'oauth', key: 'oauth' },
  { id: 'team', key: 'team' },
  { id: 'billing', key: 'billing' },
  { id: 'settings', key: 'settings' },
];

function useSections() {
  const t = useTranslations('docs');
  return sectionIds.map(({ id, key }) => ({
    id,
    title: t(`sections.${key}.title`),
  }));
}

function ScreenSection({
  id,
  sectionKey,
  screenshot
}: {
  id: string;
  sectionKey: SectionKey;
  screenshot: string;
}) {
  const t = useTranslations('docs');
  const section = `sections.${sectionKey}`;

  // Get features as array
  const featuresRaw = t.raw(`${section}.features`) as string[] | undefined;
  const features = Array.isArray(featuresRaw) ? featuresRaw : [];

  // Get actions as array
  const actionsRaw = t.raw(`${section}.actions`) as string[] | undefined;
  const actions = Array.isArray(actionsRaw) ? actionsRaw : [];

  return (
    <section id={id} className="scroll-mt-24 mb-16 pt-8 border-t first:border-t-0 first:pt-0">
      <h2 className="text-2xl font-bold text-gray-900 mb-4">
        {t(`${section}.title`)}
      </h2>

      {/* Screenshot */}
      <div className="mb-6 rounded-xl border border-gray-200 overflow-hidden bg-gray-50">
        <Image
          src={`/screenshots/${screenshot}`}
          alt={t(`${section}.title`)}
          width={1200}
          height={675}
          className="w-full h-auto"
          priority={id === 'dashboard'}
        />
      </div>

      {/* Description */}
      <p className="text-gray-600 mb-6">
        {t(`${section}.description`)}
      </p>

      {/* Features */}
      {features.length > 0 && (
        <div className="mb-6">
          <h3 className="text-lg font-semibold text-gray-900 mb-3">
            {t('common.features')}
          </h3>
          <ul className="space-y-2">
            {features.map((feature, i) => (
              <li key={i} className="flex items-start gap-2 text-gray-600">
                <span className="text-violet-500 mt-1">•</span>
                <span>{feature}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Actions */}
      {actions.length > 0 && (
        <div>
          <h3 className="text-lg font-semibold text-gray-900 mb-3">
            {t('common.actions')}
          </h3>
          <ul className="space-y-2">
            {actions.map((action, i) => (
              <li key={i} className="flex items-start gap-2 text-gray-600">
                <span className="text-emerald-500 mt-1">→</span>
                <span>{action}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

export default function DocsPage() {
  const t = useTranslations('docs');
  const sections = useSections();
  const [activeSection, setActiveSection] = useState('dashboard');
  const isScrollingRef = useRef(false);

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        if (isScrollingRef.current) return;

        const visibleSections = entries
          .filter((entry) => entry.isIntersecting)
          .map((entry) => entry.target.id);

        if (visibleSections.length > 0) {
          const sectionOrder = sectionIds.map((s) => s.id);
          const topSection = visibleSections.sort(
            (a, b) => sectionOrder.indexOf(a) - sectionOrder.indexOf(b)
          )[0];
          setActiveSection(topSection);
        }
      },
      {
        rootMargin: '-80px 0px -60% 0px',
        threshold: 0,
      }
    );

    sectionIds.forEach((section) => {
      const element = document.getElementById(section.id);
      if (element) observer.observe(element);
    });

    return () => observer.disconnect();
  }, []);

  const scrollToSection = (id: string) => {
    setActiveSection(id);
    isScrollingRef.current = true;
    const element = document.getElementById(id);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'start' });
      setTimeout(() => {
        isScrollingRef.current = false;
      }, 1000);
    }
  };

  const screenshotMap: Record<SectionKey, string> = {
    dashboard: 'dashboard.png',
    servers: 'servers.png',
    serverNew: 'server-new.png',
    serverDetail: 'server-overview.png',
    logs: 'logs.png',
    accessTokens: 'access-tokens.png',
    oauth: 'oauth-apps.png',
    team: 'team.png',
    billing: 'billing.png',
    settings: 'settings.png',
  };

  return (
    <div className="min-h-screen bg-white">
      <Header />

      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
        <div className="flex gap-12">
          {/* Sidebar */}
          <aside className="hidden lg:block w-64 flex-shrink-0">
            <nav className="sticky top-24 space-y-1">
              <p className="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-4">
                {t('sidebar')}
              </p>
              {sections.map((section) => (
                <button
                  key={section.id}
                  onClick={() => scrollToSection(section.id)}
                  className={`block w-full text-left px-3 py-2 text-sm transition-colors ${
                    activeSection === section.id
                      ? 'text-gray-900 font-medium'
                      : 'text-gray-400 hover:text-gray-600'
                  }`}
                >
                  {section.title}
                </button>
              ))}
            </nav>
          </aside>

          {/* Main Content */}
          <main className="flex-1 min-w-0">
            {/* Header */}
            <div className="mb-12">
              <h1 className="text-4xl font-black text-gray-900 mb-4">
                {t('title')}
              </h1>
              <p className="text-base text-gray-600">
                {t('subtitle')}
              </p>
            </div>

            {/* Sections */}
            {sectionIds.map(({ id, key }) => (
              <ScreenSection
                key={id}
                id={id}
                sectionKey={key}
                screenshot={screenshotMap[key]}
              />
            ))}

            {/* Contact */}
            <div className="mt-12 pt-8 border-t">
              <p className="text-gray-600 mb-4">
                {t('contact.prompt')}
              </p>
              <Link href="/contact" className="text-violet-600 font-medium hover:underline">
                {t('contact.link')}
              </Link>
            </div>
          </main>
        </div>
      </div>

      <Footer />
    </div>
  );
}
