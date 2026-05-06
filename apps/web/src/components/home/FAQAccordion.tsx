'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { Button } from '@/components/ui/button';
import Link from 'next/link';

const faqItems = [
  { qKey: 'faq.q1.question', aKey: 'faq.q1.answer' },
  { qKey: 'faq.q2.question', aKey: 'faq.q2.answer' },
  { qKey: 'faq.q3.question', aKey: 'faq.q3.answer' },
];

export function FAQAccordion() {
  const t = useTranslations('home');
  const [openFaq, setOpenFaq] = useState<number | null>(null);

  return (
    <section className="py-20 bg-gray-50">
      <div className="max-w-3xl mx-auto px-4 sm:px-6">
        <div className="text-center mb-10">
          <span className="inline-block text-violet-600 text-sm font-medium mb-4">
            {t('faq.badge')}
          </span>
          <h2 className="text-2xl sm:text-3xl font-extrabold mb-3" style={{ color: '#333333' }}>{t('faq.title')}</h2>
          <p className="text-lg text-gray-600">{t('faq.subtitle')}</p>
        </div>

        <div className="space-y-4">
          {faqItems.map((item, idx) => (
            <div
              key={idx}
              className={`bg-white rounded-2xl border transition-all duration-300 ${openFaq === idx ? 'border-violet-400 shadow-lg shadow-violet-500/5' : 'border-gray-300'}`}
            >
              <button
                onClick={() => setOpenFaq(openFaq === idx ? null : idx)}
                className="w-full flex items-center justify-between p-6 text-left"
              >
                <span className="font-semibold text-gray-900 pr-8">{t(item.qKey)}</span>
                <div className={`w-10 h-10 rounded-full flex items-center justify-center flex-shrink-0 transition-all ${openFaq === idx ? 'bg-violet-100 rotate-180' : 'bg-gray-100'}`}>
                  <svg className={`w-5 h-5 transition-colors ${openFaq === idx ? 'text-violet-600' : 'text-gray-400'}`} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M6 9l6 6 6-6" />
                  </svg>
                </div>
              </button>
              <div className={`overflow-hidden transition-all duration-300 ${openFaq === idx ? 'max-h-96' : 'max-h-0'}`}>
                <div className="px-6 pb-6">
                  <p className="text-gray-600 leading-relaxed">{t(item.aKey)}</p>
                </div>
              </div>
            </div>
          ))}
        </div>

        <div className="mt-8 text-center">
          <Link href="/faq">
            <Button variant="ghost" className="hover:bg-gray-100 gap-2">
              {t('faq.viewAll')}
              <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M5 12h14M12 5l7 7-7 7" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </Button>
          </Link>
        </div>
      </div>
    </section>
  );
}
