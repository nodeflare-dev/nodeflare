'use client';

import { Suspense, useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import Link from 'next/link';
import Image from 'next/image';
import { useTranslations } from 'next-intl';

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

/** Indeterminate gradient progress bar: a violet segment sweeps left -> right, looping. */
function IndeterminateBar() {
  return (
    <div className="h-1.5 w-full overflow-hidden rounded-full bg-violet-100">
      <div className="h-full w-2/5 rounded-full bg-gradient-to-r from-violet-400 via-violet-600 to-violet-400 animate-indeterminate" />
    </div>
  );
}

/** NodeFlare wordmark. */
function BrandMark() {
  return (
    <Image
      src="/logo2.png"
      alt="Nodeflare"
      width={153}
      height={32}
      priority
      className="h-9 w-auto"
    />
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
        <div className="flex w-full max-w-[260px] flex-col items-center px-6 text-center">
          <BrandMark />
          <h1 className="mt-6 text-xl font-bold text-red-600">{t('error')}</h1>
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
      <div className="flex w-full max-w-[260px] flex-col items-center px-6">
        <BrandMark />
        <p className="mt-6 text-gray-700 font-medium">{t('signingIn')}</p>
        <div className="mt-5 w-full">
          <IndeterminateBar />
        </div>
      </div>
    </div>
  );
}

// Fallback component without i18n (for SSR)
function LoadingFallback() {
  return (
    <div className="min-h-screen flex items-center justify-center bg-white">
      <div className="flex w-full max-w-[260px] flex-col items-center px-6">
        <BrandMark />
        <p className="mt-6 text-gray-700 font-medium">Signing in…</p>
        <div className="mt-5 w-full">
          <IndeterminateBar />
        </div>
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
