'use client';

import { useState, useEffect } from 'react';
import { useTranslations } from 'next-intl';

const dashboardImages = [
  { src: '/dashboard.png', label: 'dashboard.slideLabels.dashboard' },
  { src: '/servertop.png', label: 'dashboard.slideLabels.servers' },
  { src: '/server.png', label: 'dashboard.slideLabels.serverDetail' },
  { src: '/auth.png', label: 'dashboard.slideLabels.auth' },
  { src: '/log.png', label: 'dashboard.slideLabels.logs' },
];

export function DashboardSlideshow() {
  const t = useTranslations('home');
  const [currentSlide, setCurrentSlide] = useState(0);

  // Auto-rotate slideshow
  useEffect(() => {
    const timer = setInterval(() => {
      setCurrentSlide((prev) => (prev + 1) % dashboardImages.length);
    }, 5000);
    return () => clearInterval(timer);
  }, []);

  const total = dashboardImages.length;
  const slideWidth = 80; // 各スライドが占める幅（コンテナ比 %）
  const peek = (100 - slideWidth) / 2; // 左右ののぞき幅 %
  // flexトラックの幅はコンテナと同じ100%基準なので、translateX% はコンテナ比でそのまま指定
  const translatePct = peek - slideWidth * currentSlide;

  return (
    <section className="pb-20">
      <div className="max-w-7xl mx-auto px-4 sm:px-6">
        <div className="relative overflow-hidden">
          {/* ピーク表示のカルーセル */}
          <div
            className="transition-transform duration-300 ease-in-out"
            style={{ display: 'flex', flexWrap: 'nowrap', transform: `translateX(${translatePct}%)` }}
          >
            {dashboardImages.map((image, idx) => {
              const active = idx === currentSlide;
              return (
                <div key={idx} className="px-2 sm:px-3" style={{ flex: '0 0 80%', maxWidth: '80%' }}>
                  <button
                    type="button"
                    onClick={() => setCurrentSlide(idx)}
                    className="block w-full overflow-hidden rounded-2xl border border-gray-200 bg-white transition-all duration-500"
                  >
                    <div className="flex items-center gap-1.5 border-b border-gray-100 bg-gray-50/80 px-4 py-3">
                      <span className="h-3 w-3 rounded-full bg-red-400" />
                      <span className="h-3 w-3 rounded-full bg-yellow-400" />
                      <span className="h-3 w-3 rounded-full bg-green-400" />
                      <span className="ml-2 truncate text-xs text-gray-400">{t(image.label)}</span>
                    </div>
                    <img
                      src={image.src}
                      alt={`Dashboard screenshot ${idx + 1}`}
                      className="w-full"
                    />
                  </button>
                </div>
              );
            })}
          </div>

          {/* 両端のフェード（白背景へ徐々に溶ける） */}
          <div className="pointer-events-none absolute inset-y-0 left-0 z-20 w-[14%] bg-gradient-to-r from-white via-white/70 to-transparent" />
          <div className="pointer-events-none absolute inset-y-0 right-0 z-20 w-[14%] bg-gradient-to-l from-white via-white/70 to-transparent" />
        </div>

        {/* Slide indicators */}
        <div className="mt-6 flex items-center justify-center gap-2">
          {dashboardImages.map((_, idx) => (
            <button
              key={idx}
              onClick={() => setCurrentSlide(idx)}
              className={`transition-all duration-300 ${
                currentSlide === idx
                  ? 'h-2 w-6 rounded-full bg-violet-600'
                  : 'h-2 w-2 rounded-full bg-gray-300 hover:bg-gray-400'
              }`}
              aria-label={`Go to slide ${idx + 1}`}
            />
          ))}
        </div>
      </div>
    </section>
  );
}
