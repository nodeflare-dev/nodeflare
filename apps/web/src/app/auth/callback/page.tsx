'use client';

import { Suspense, useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import Link from 'next/link';
import { useTranslations } from 'next-intl';
import { SiGithub } from 'react-icons/si';
import { HiOutlineMail } from 'react-icons/hi';

const GoogleIcon = ({ className = "w-7 h-7" }: { className?: string }) => (
  <svg className={className} viewBox="0 0 24 24">
    <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z" />
    <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" />
    <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" />
    <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" />
  </svg>
);

type AuthProvider = 'github' | 'google' | 'email';

// SECURITY: Validate return_to URL to prevent open redirect attacks
function isValidReturnTo(url: string): boolean {
  if (!url || typeof url !== 'string') return false;

  const trimmed = url.trim();
  if (trimmed.length === 0) return false;

  // Must start with single / (relative path)
  // Block protocol-relative URLs (//) and absolute URLs
  if (!trimmed.startsWith('/') || trimmed.startsWith('//')) {
    return false;
  }

  // Block dangerous schemes that could be embedded
  const lower = trimmed.toLowerCase();
  if (
    lower.includes('javascript:') ||
    lower.includes('data:') ||
    lower.includes('vbscript:')
  ) {
    return false;
  }

  // Block control characters
  if (/[\x00-\x1f\x7f]/.test(trimmed)) {
    return false;
  }

  return true;
}

function AuthLoader({ provider }: { provider: AuthProvider }) {
  const renderIcon = () => {
    switch (provider) {
      case 'google':
        return <GoogleIcon className="w-7 h-7" />;
      case 'email':
        return <HiOutlineMail className="w-7 h-7 text-violet-600" />;
      case 'github':
      default:
        return <SiGithub className="w-7 h-7 text-violet-600" />;
    }
  };

  return (
    <div className="relative w-16 h-16">
      <div className="absolute inset-0 animate-spin rounded-full border-4 border-violet-200 border-t-violet-600"></div>
      <div className="absolute inset-0 flex items-center justify-center pt-1">
        {renderIcon()}
      </div>
    </div>
  );
}

function AuthCallbackContent() {
  const t = useTranslations('auth.callback');
  const searchParams = useSearchParams();
  const [error, setError] = useState<string | null>(null);

  // Get provider from URL params (set by backend during OAuth redirect)
  const providerParam = searchParams.get('provider');
  const provider: AuthProvider =
    providerParam === 'google' ? 'google' :
    providerParam === 'email' ? 'email' :
    'github';

  useEffect(() => {
    const errorParam = searchParams.get('error');

    if (errorParam) {
      setError(errorParam);
      return;
    }

    // Check if there's a return_to parameter (for OAuth flow)
    const returnTo = searchParams.get('return_to');
    if (returnTo && isValidReturnTo(returnTo)) {
      // SECURITY: Only redirect if return_to is a valid relative path
      window.location.href = `/?return_to=${encodeURIComponent(returnTo)}`;
      return;
    }

    // Tokens are now set as HTTP-only cookies by the server.
    // Use window.location for more reliable redirect
    window.location.href = '/dashboard';
  }, [searchParams]);

  if (error) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-white">
        <div className="text-center">
          <h1 className="text-2xl font-bold text-red-600">{t('error')}</h1>
          <p className="mt-2 text-gray-600">{error}</p>
          <Link href="/" className="mt-4 inline-block text-violet-600 hover:underline">
            {t('returnHome')}
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-white">
      <div className="flex flex-col items-center">
        <AuthLoader provider={provider} />
        <p className="mt-6 text-gray-700 font-medium">{t('signingIn')}</p>
      </div>
    </div>
  );
}

// Fallback component without i18n (for SSR)
function LoadingFallback() {
  return (
    <div className="min-h-screen flex items-center justify-center bg-white">
      <div className="flex flex-col items-center">
        <div className="relative w-16 h-16">
          <div className="absolute inset-0 animate-spin rounded-full border-4 border-violet-200 border-t-violet-600"></div>
        </div>
        <p className="mt-6 text-gray-700 font-medium">Loading...</p>
      </div>
    </div>
  );
}

export default function AuthCallbackPage() {
  return (
    <Suspense fallback={<LoadingFallback />}>
      <AuthCallbackContent />
    </Suspense>
  );
}
