'use client';

import { useRouter } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { Lock, KeyRound, Globe, ArrowRight, Info } from 'lucide-react';

export default function AuthSettingsPage() {
  const t = useTranslations('auth.settings');
  const router = useRouter();

  return (
    <div className="max-w-4xl">
      {/* Header */}
      <div className="mb-8">
        <h1 className="text-2xl font-medium flex items-center gap-2 text-gray-400">
          <Lock className="w-6 h-6" />
          {t('title')}
        </h1>
      </div>

      {/* Auth Method Cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* Access Tokens Card */}
        <button
          onClick={() => router.push('/dashboard/auth/access-tokens')}
          className="group p-5 bg-white rounded-xl border border-gray-300 text-left transition-all duration-300 hover:scale-[1.02] hover:shadow-lg hover:border-violet-300 flex flex-col h-full min-h-[160px]"
        >
          {/* Header: Icon + Title */}
          <div className="flex items-center gap-2 mb-3">
            <KeyRound className="w-5 h-5 text-[#333333]" />
            <h2 className="text-base font-semibold text-[#333333]">
              {t('accessTokens.title')}
            </h2>
            <span className="ml-auto px-2 py-0.5 text-[10px] font-medium text-violet-700">
              {t('accessTokens.badge')}
            </span>
          </div>
          {/* Content */}
          <div className="flex-1">
            <p className="text-sm text-gray-500 leading-relaxed">
              {t('accessTokens.description')}
            </p>
          </div>
          {/* Manage Link - Fixed at bottom */}
          <div className="flex items-center gap-1 mt-3 text-violet-700 text-sm font-medium">
            <span>{t('manage')}</span>
            <ArrowRight className="w-4 h-4 group-hover:translate-x-1 transition-transform" />
          </div>
        </button>

        {/* OAuth Apps Card */}
        <button
          onClick={() => router.push('/dashboard/auth/oauth')}
          className="group p-5 bg-white rounded-xl border border-gray-300 text-left transition-all duration-300 hover:scale-[1.02] hover:shadow-lg hover:border-blue-300 flex flex-col h-full min-h-[160px]"
        >
          {/* Header: Icon + Title */}
          <div className="flex items-center gap-2 mb-3">
            <Globe className="w-5 h-5 text-[#333333]" />
            <h2 className="text-base font-semibold text-[#333333]">
              {t('oauth.title')}
            </h2>
            <span className="ml-auto px-2 py-0.5 text-[10px] font-medium text-violet-700">
              {t('oauth.badge')}
            </span>
          </div>
          {/* Content */}
          <div className="flex-1">
            <p className="text-sm text-gray-500 leading-relaxed">
              {t('oauth.description')}
            </p>
          </div>
          {/* Manage Link - Fixed at bottom */}
          <div className="flex items-center gap-1 mt-3 text-violet-700 text-sm font-medium">
            <span>{t('manage')}</span>
            <ArrowRight className="w-4 h-4 group-hover:translate-x-1 transition-transform" />
          </div>
        </button>
      </div>

      {/* Info Section */}
      <div className="mt-6 flex items-start gap-2">
        <Info className="w-4 h-4 text-gray-400 mt-0.5 flex-shrink-0" />
        <p className="text-sm text-gray-500">
          <span className="font-medium text-gray-600">{t('info.title')}</span>
          {' '}{t('info.description')}
        </p>
      </div>
    </div>
  );
}
