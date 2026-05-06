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

  return (
    <section className="pb-20">
      <div className="max-w-4xl mx-auto px-4 sm:px-6">
        <div className="relative">
          {/* 背景の装飾 */}
          <div className="absolute -inset-4 bg-gray-100/80 rounded-3xl transform rotate-1" />
          <div className="absolute -inset-4 bg-white rounded-3xl transform -rotate-1 shadow-xl" />

          {/* メインカード */}
          <div className="relative rounded-2xl border border-gray-200 shadow-2xl overflow-hidden bg-white">
            <div className="border-b border-gray-100 bg-gray-50/80 px-4 py-3 flex items-center gap-3">
              <div className="flex gap-1.5">
                <div className="w-3 h-3 rounded-full bg-red-400" />
                <div className="w-3 h-3 rounded-full bg-yellow-400" />
                <div className="w-3 h-3 rounded-full bg-green-400" />
              </div>
              <div className="flex-1 flex justify-center">
                <div className="px-4 py-1 rounded-md bg-white border border-gray-200 text-xs text-gray-500 transition-all duration-300">
                  {t(dashboardImages[currentSlide].label)}
                </div>
              </div>
            </div>

            {/* Dashboard Screenshots Slideshow */}
            <div className="relative overflow-hidden">
              <div
                className="flex transition-transform duration-500 ease-in-out"
                style={{ transform: `translateX(-${currentSlide * 100}%)` }}
              >
                {dashboardImages.map((image, idx) => (
                  <img
                    key={idx}
                    src={image.src}
                    alt={`Dashboard screenshot ${idx + 1}`}
                    className="w-full flex-shrink-0"
                  />
                ))}
              </div>

              {/* Navigation arrows */}
              <button
                onClick={() => setCurrentSlide((prev) => (prev - 1 + dashboardImages.length) % dashboardImages.length)}
                className="absolute left-3 top-1/2 -translate-y-1/2 w-10 h-10 rounded-full bg-black/40 hover:bg-black/60 backdrop-blur-sm flex items-center justify-center text-white transition-all"
                aria-label="Previous slide"
              >
                <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M15 18l-6-6 6-6" />
                </svg>
              </button>
              <button
                onClick={() => setCurrentSlide((prev) => (prev + 1) % dashboardImages.length)}
                className="absolute right-3 top-1/2 -translate-y-1/2 w-10 h-10 rounded-full bg-black/40 hover:bg-black/60 backdrop-blur-sm flex items-center justify-center text-white transition-all"
                aria-label="Next slide"
              >
                <svg className="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M9 18l6-6-6-6" />
                </svg>
              </button>
            </div>

            {/* Slide indicators */}
            <div className="absolute bottom-4 left-1/2 transform -translate-x-1/2 flex items-center gap-2 bg-black/40 backdrop-blur-sm rounded-full px-3 py-2">
              {dashboardImages.map((_, idx) => (
                <button
                  key={idx}
                  onClick={() => setCurrentSlide(idx)}
                  className={`transition-all duration-300 ${
                    currentSlide === idx
                      ? 'w-6 h-2 bg-white rounded-full'
                      : 'w-2 h-2 bg-white/50 hover:bg-white/70 rounded-full'
                  }`}
                  aria-label={`Go to slide ${idx + 1}`}
                />
              ))}
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
