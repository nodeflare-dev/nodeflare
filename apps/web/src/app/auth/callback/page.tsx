'use client';

import { Suspense, useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import Link from 'next/link';
import { useTranslations } from 'next-intl';
import { SiGithub } from 'react-icons/si';

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

function GitHubLoader() {
  return (
    <div className="relative w-16 h-16">
      <div className="absolute inset-0 animate-spin rounded-full border-4 border-violet-200 border-t-violet-600"></div>
      <div className="absolute inset-0 flex items-center justify-center pt-1">
        <SiGithub className="w-7 h-7 text-violet-600" />
      </div>
    </div>
  );
}

function AuthCallbackContent() {
  const t = useTranslations('auth.callback');
  const searchParams = useSearchParams();
  const [error, setError] = useState<string | null>(null);

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
        <GitHubLoader />
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
          <div className="absolute inset-0 flex items-center justify-center pt-1">
            <SiGithub className="w-7 h-7 text-violet-600" />
          </div>
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
