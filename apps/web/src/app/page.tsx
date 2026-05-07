import { getTranslations } from 'next-intl/server';
import { Suspense } from 'react';
import Link from 'next/link';

// Use ISR for better SEO - revalidate every 60 seconds
export const revalidate = 60;
import { Header, Footer } from '@/components/layout';
import {
  OAuthHandler,
  HeroAuthButtons,
  DashboardSlideshow,
  FAQAccordion,
  ContactForm,
  PricingButtons,
  BlogSection,
} from '@/components/home';

export default async function HomePage() {
  const t = await getTranslations('home');

  const features = [
    { titleKey: 'features.zeroConfig.title', descKey: 'features.zeroConfig.desc', icon: <><path d="M12 2a10 10 0 1 0 10 10H12V2z" /><path d="M21.18 8.02A10 10 0 0 0 12 2v10h10a10 10 0 0 0-0.82-3.98z" /></>, align: 'left' },
    { titleKey: 'features.acl.title', descKey: 'features.acl.desc', icon: <><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /><path d="M9 12l2 2 4-4" /></>, align: 'right' },
    { titleKey: 'features.secrets.title', descKey: 'features.secrets.desc', icon: <><rect x="3" y="11" width="18" height="11" rx="2" /><path d="M7 11V7a5 5 0 0 1 10 0v4" /></>, align: 'left' },
    { titleKey: 'features.protocol.title', descKey: 'features.protocol.desc', icon: <><path d="M22 12h-4l-3 9L9 3l-3 9H2" /></>, align: 'right' },
    { titleKey: 'features.alwaysOn.title', descKey: 'features.alwaysOn.desc', icon: <><circle cx="12" cy="12" r="10" /><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" /></>, align: 'left' },
  ];

  const devFeatures = [
    { icon: '✓', textKey: 'devExperience.tokenAuth', color: 'text-violet-600' },
    { icon: '✓', textKey: 'devExperience.toolPermission', color: 'text-violet-600' },
    { icon: '✓', textKey: 'devExperience.accessLog', color: 'text-violet-600' },
    { icon: '✓', textKey: 'devExperience.noCode', color: 'text-violet-600' },
  ];

  const freeFeatures = ['pricing.free.feature1', 'pricing.free.feature2', 'pricing.free.feature3', 'pricing.free.feature4'];
  const proFeatures = ['pricing.pro.feature1', 'pricing.pro.feature2', 'pricing.pro.feature3', 'pricing.pro.feature4', 'pricing.pro.feature5'];

  return (
    <div className="min-h-screen bg-white">
      {/* OAuth Handler - invisible component for OAuth redirect processing */}
      <Suspense fallback={null}>
        <OAuthHandler />
      </Suspense>

      <Header />

      <main>
        {/* Hero - ドットパターン背景 */}
        <section className="relative pt-16 pb-20 sm:pt-20 sm:pb-24 overflow-hidden">
          {/* 右側の背景画像 */}
          <div
            className="absolute -right-20 w-[60%] h-[120%] bg-no-repeat bg-top bg-contain pointer-events-none hidden lg:block"
            style={{ backgroundImage: 'url(/top.png)', top: '-5%' }}
          />
          <div className="relative max-w-6xl mx-auto px-6 sm:px-10 lg:px-16">
            <div>
              <div className="relative inline-block mb-6 ml-1">
                <div className="relative px-4 py-2 bg-gray-900 text-white text-sm font-medium rounded-lg">
                  {t('title')}
                  <div className="absolute -bottom-1.5 left-6 w-2.5 h-2.5 bg-gray-900 rotate-45" />
                </div>
              </div>

              <h1 className="text-6xl sm:text-7xl lg:text-8xl font-black text-gray-900 tracking-tight leading-[1.05] text-left">
                {t('heroTitle1')}<br />
                <span className="text-violet-600">{t('heroTitle2')}</span>
              </h1>

              <div className="text-center">
                <p className="mt-6 text-2xl text-gray-400 leading-relaxed max-w-2xl text-left font-bold">
                  {t('heroDescription1')}<br />
                  {t('heroDescription2')}
                </p>

                <Suspense fallback={
                  <div className="mt-8 flex flex-wrap justify-center gap-3">
                    <div className="h-10 w-40 bg-violet-100 rounded-lg animate-pulse" />
                    <div className="h-10 w-24 bg-gray-100 rounded-lg animate-pulse" />
                  </div>
                }>
                  <HeroAuthButtons />
                </Suspense>
              </div>
            </div>
          </div>
        </section>

        {/* Dashboard Preview - カード重ねデザイン */}
        <DashboardSlideshow />

        {/* Features - 吹き出しブロック */}
        <section className="py-24">
          <div className="max-w-4xl mx-auto px-4 sm:px-6">
            <div className="text-center mb-16">
              <span className="inline-block text-violet-600 text-sm font-medium mb-4">
                {t('features.badge')}
              </span>
              <h2 className="text-3xl sm:text-4xl font-extrabold" style={{ color: '#333333' }}>{t('features.title')}</h2>
            </div>

            {/* 吹き出しブロック */}
            <div className="space-y-4">
              {features.map((item, idx) => (
                <div key={idx} className={`flex flex-col ${item.align === 'right' ? 'items-end' : 'items-start'}`}>
                  {/* タイトル行（アイコン＋タイトル）- カード外 */}
                  <div className="flex items-center gap-2 mb-1.5">
                    <svg className="w-4 h-4 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      {item.icon}
                    </svg>
                    <p className="text-sm font-bold text-gray-400">{t(item.titleKey)}</p>
                  </div>
                  {/* カード */}
                  <div className="relative inline-block max-w-sm">
                    {/* 紫の影（ずらした吹き出し） */}
                    <div className="absolute top-1 left-1 w-full h-full">
                      <div className="px-4 py-3 bg-violet-500" style={{ visibility: 'hidden' }}>
                        <p className="text-base">{t(item.descKey)}</p>
                      </div>
                      <div className="absolute inset-0 bg-violet-500" />
                      {/* 矢印（横向き） */}
                      <div className={`absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 rotate-45 bg-violet-500 ${item.align === 'right' ? '-right-[5px]' : '-left-[5px]'}`} />
                    </div>
                    {/* 吹き出し本体 */}
                    <div className="relative px-4 py-3" style={{ backgroundColor: '#323232' }}>
                      <p className="text-base text-gray-100">{t(item.descKey)}</p>
                    </div>
                    {/* 吹き出しの矢印（横向き） */}
                    <div className={`absolute top-1/2 -translate-y-1/2 w-2.5 h-2.5 rotate-45 ${item.align === 'right' ? '-right-[5px]' : '-left-[5px]'}`} style={{ backgroundColor: '#323232' }} />
                  </div>
                </div>
              ))}
            </div>
          </div>
        </section>

        {/* Code Example - サイドバイサイド + シンタックスハイライト */}
        <section className="py-20">
          <div className="max-w-4xl mx-auto px-4 sm:px-6">
            <div className="grid lg:grid-cols-2 gap-12 items-center">
              <div>
                <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full text-violet-600 text-sm font-medium mb-6">
                  {t('devExperience.badge')}
                </div>
                <h2 className="text-3xl font-extrabold mb-5" style={{ color: '#333333' }}>
                  {t('devExperience.title')}
                </h2>
                <p className="text-lg text-gray-600 mb-8 leading-relaxed">
                  {t('devExperience.description')}
                </p>

                <div className="space-y-4">
                  {devFeatures.map((item, idx) => (
                    <div key={idx} className="flex items-center gap-3">
                      <span className={`${item.color} font-bold`}>{item.icon}</span>
                      <span className="text-gray-700">{t(item.textKey)}</span>
                    </div>
                  ))}
                </div>
              </div>

              <div className="relative">
                <img
                  src="/access.png"
                  alt="Access Control Settings"
                  className="w-full rounded-2xl"
                />
              </div>
            </div>
          </div>
        </section>

        {/* Pricing - 非対称カード */}
        <section className="py-20 bg-gradient-to-b from-gray-50 to-white">
          <div className="max-w-4xl mx-auto px-4 sm:px-6">
            <div className="text-center mb-10">
              <span className="inline-block text-violet-600 text-sm font-medium mb-4">
                {t('pricing.badge')}
              </span>
              <h2 className="text-2xl sm:text-3xl font-extrabold mb-3" style={{ color: '#333333' }}>{t('pricing.title')}</h2>
              <p className="text-lg text-gray-600">{t('pricing.subtitle')}</p>
            </div>

            <div className="grid md:grid-cols-2 gap-6">
              {/* Free */}
              <div className="relative group">
                <div className="relative bg-white rounded-2xl p-8 border border-gray-200 hover:border-gray-300 hover:shadow-lg transition-all h-full">
                  <div className="text-sm font-medium text-gray-500 mb-2">{t('pricing.free.name')}</div>
                  <div className="flex items-baseline gap-1 mb-6">
                    <span className="text-5xl font-bold text-gray-900">{t('pricing.free.price')}</span>
                    <span className="text-gray-500">{t('pricing.perMonth')}</span>
                  </div>
                  <p className="text-gray-600 mb-8">{t('pricing.free.description')}</p>
                  <ul className="space-y-4 mb-8">
                    {freeFeatures.map((featureKey) => (
                      <li key={featureKey} className="flex items-center gap-3 text-gray-700">
                        <svg className="w-5 h-5 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                          <path d="M20 6L9 17l-5-5" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                        {t(featureKey)}
                      </li>
                    ))}
                  </ul>
                  <Suspense fallback={<div className="h-12 bg-gray-100 rounded-lg animate-pulse" />}>
                    <PricingButtons variant="free" />
                  </Suspense>
                </div>
              </div>

              {/* Pro */}
              <div className="relative group">
                <div className="absolute -inset-[1px] bg-violet-500 rounded-2xl" />
                <div className="relative bg-gray-900 rounded-2xl p-8 text-white h-full">
                  <div className="flex items-center gap-2 mb-2">
                    <span className="text-sm font-medium text-violet-300">{t('pricing.pro.name')}</span>
                    <span className="px-2 py-0.5 rounded-full bg-violet-500/20 text-violet-300 text-xs font-medium">{t('pricing.pro.badge')}</span>
                  </div>
                  <div className="flex items-baseline gap-1 mb-6">
                    <span className="text-5xl font-bold">{t('pricing.pro.price')}</span>
                    <span className="text-gray-400">{t('pricing.perMonth')}</span>
                  </div>
                  <p className="text-gray-400 mb-8">{t('pricing.pro.description')}</p>
                  <ul className="space-y-4 mb-8">
                    {proFeatures.map((featureKey) => (
                      <li key={featureKey} className="flex items-center gap-3">
                        <svg className="w-5 h-5 text-violet-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                          <path d="M20 6L9 17l-5-5" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                        {t(featureKey)}
                      </li>
                    ))}
                  </ul>
                  <Suspense fallback={<div className="h-12 bg-violet-500/50 rounded-lg animate-pulse" />}>
                    <PricingButtons variant="pro" />
                  </Suspense>
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* Blog - マガジンレイアウト */}
        <BlogSection />

        {/* FAQ - インタラクティブアコーディオン */}
        <FAQAccordion />

        {/* Contact - シンプルフォーム */}
        <ContactForm />

      </main>

      <Footer />
    </div>
  );
}
