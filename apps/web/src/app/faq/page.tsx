'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { Button } from '@/components/ui/button';
import { Header, Footer } from '@/components/layout';
import Link from 'next/link';

type CategoryKey = 'basic' | 'pricing' | 'technical' | 'security';

function useFaqs() {
  const t = useTranslations('faqPage');

  return [
    {
      category: t('categories.basic'),
      categoryKey: 'basic' as CategoryKey,
      questions: [
        { q: t('basic.q1'), a: t('basic.a1') },
        { q: t('basic.q2'), a: t('basic.a2') },
        { q: t('basic.q3'), a: t('basic.a3') },
      ],
    },
    {
      category: t('categories.pricing'),
      categoryKey: 'pricing' as CategoryKey,
      questions: [
        { q: t('pricing.q1'), a: t('pricing.a1') },
        { q: t('pricing.q2'), a: t('pricing.a2') },
        { q: t('pricing.q3'), a: t('pricing.a3') },
      ],
    },
    {
      category: t('categories.technical'),
      categoryKey: 'technical' as CategoryKey,
      questions: [
        { q: t('technical.q1'), a: t('technical.a1') },
        { q: t('technical.q2'), a: t('technical.a2') },
        { q: t('technical.q3'), a: t('technical.a3') },
        { q: t('technical.q4'), a: t('technical.a4') },
      ],
    },
    {
      category: t('categories.security'),
      categoryKey: 'security' as CategoryKey,
      questions: [
        { q: t('security.q1'), a: t('security.a1') },
        { q: t('security.q2'), a: t('security.a2') },
        { q: t('security.q3'), a: t('security.a3') },
      ],
    },
  ];
}

export default function FAQPage() {
  const t = useTranslations('faqPage');
  const faqs = useFaqs();
  const [openIndex, setOpenIndex] = useState<string | null>(null);

  const toggleQuestion = (key: string) => {
    setOpenIndex(openIndex === key ? null : key);
  };

  return (
    <div className="min-h-screen bg-gradient-to-b from-gray-50 to-white">
      <Header />

      <main className="max-w-4xl mx-auto px-4 sm:px-6 py-16 sm:py-24">
        <div className="mb-12 text-center">
          <h1 className="text-xl sm:text-2xl font-semibold" style={{ color: '#323232' }}>{t('title')}</h1>
        </div>

        <div className="space-y-12">
          {faqs.map((section) => (
            <div key={section.category}>
              <h2 className="text-lg font-semibold text-gray-900 mb-4 pb-2 border-b border-gray-100">
                {section.category}
              </h2>
              <div className="space-y-3">
                {section.questions.map((item, idx) => {
                  const key = `${section.category}-${idx}`;
                  const isOpen = openIndex === key;
                  return (
                    <div key={key} className="border border-gray-100 rounded-lg overflow-hidden">
                      <button
                        onClick={() => toggleQuestion(key)}
                        className="w-full flex items-center justify-between p-4 text-left hover:bg-gray-50 transition-colors"
                      >
                        <span className="font-medium text-gray-900">{item.q}</span>
                        <svg
                          className={`w-5 h-5 text-gray-400 transition-transform ${isOpen ? 'rotate-180' : ''}`}
                          viewBox="0 0 24 24"
                          fill="none"
                          stroke="currentColor"
                          strokeWidth="2"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        >
                          <path d="M6 9l6 6 6-6" />
                        </svg>
                      </button>
                      {isOpen && (
                        <div className="px-4 pb-4">
                          <p className="text-gray-600 leading-relaxed">{item.a}</p>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          ))}
        </div>

        <div className="mt-16 text-center p-8 bg-gray-50 rounded-xl">
          <p className="text-gray-600">
            {t('notFound')}
            <Link href="/contact" className="text-violet-600 hover:text-violet-700 underline">
              {t('contactLink')}
            </Link>
            {t('contactSuffix')}
          </p>
        </div>
      </main>

      <Footer />
    </div>
  );
}
