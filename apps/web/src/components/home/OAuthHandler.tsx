'use client';

import { useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import Image from 'next/image';
import { useAuth } from '@/hooks/use-auth';
import { SquareLoader } from '@/components/ui/square-loader';

// Guard against bouncing between the landing page and /login forever if a session can't be
// established. Stores a timestamp; only honored briefly so an abandoned flow can retry later.
const LOGIN_TRIED_KEY = 'nf_oauth_login_tried';
const LOGIN_RETRY_WINDOW_MS = 30_000;

// SECURITY: only follow same-site relative paths (block absolute / protocol-relative).
function isSafeRelativePath(url: string): boolean {
  if (!url.startsWith('/') || url.startsWith('//')) return false;
  const lower = url.toLowerCase();
  if (lower.includes('javascript:') || lower.includes('data:') || lower.includes('vbscript:')) {
    return false;
  }
  return !/[\x00-\x1f\x7f]/.test(url);
}

// Collapse an absolute URL the backend may hand us into a same-site relative path so it
// survives the login round-trip's same-origin return_to validation. String-only (no window)
// so it's safe under SSR/Turbopack prerender.
function toRelative(returnTo: string): string {
  const stripped = returnTo.replace(/^https?:\/\/[^/]+/i, '');
  return stripped.startsWith('/') ? stripped : returnTo;
}

/**
 * Full-screen branded "in progress" screen shown while we finish an auth/authorization
 * redirect. The cross-domain OAuth flow has to bounce back to the frontend (the session
 * cookie lives here, not on the API domain), so without this the user would briefly see
 * the raw marketing landing page.
 */
function AuthorizingScreen({ message, showRetry }: { message: string; showRetry?: boolean }) {
  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-white">
      <div className="flex w-full max-w-[260px] flex-col items-center px-6 text-center">
        <Image src="/logo2.png" alt="Nodeflare" width={153} height={32} priority className="h-9 w-auto" />
        <p className="mt-6 text-gray-700 font-medium">{message}</p>
        {showRetry ? (
          <a href="/login" className="mt-4 text-sm text-violet-600 hover:underline">
            Try signing in again
          </a>
        ) : (
          <SquareLoader className="mt-6" />
        )}
      </div>
    </div>
  );
}

export function OAuthHandler() {
  const { refreshUser } = useAuth();
  const searchParams = useSearchParams();
  const returnTo = searchParams.get('return_to');
  const [failed, setFailed] = useState(false);

  // Render-safe (no window): just used to pick the overlay label. The actual routing in the
  // effect parses the URL precisely.
  const isAuthorize = !!returnTo && returnTo.includes('/oauth/authorize');

  useEffect(() => {
    if (!returnTo) return;
    let cancelled = false;

    (async () => {
      // Resolve auth DEFINITIVELY before deciding. useAuth exposes placeholderData, so
      // `user` can read as null for a render tick before /auth/me actually resolves —
      // relying on that bounced freshly-signed-in users back to /login (a redirect loop).
      let authedUser = null;
      try {
        const result = await refreshUser();
        authedUser = result.data ?? null;
      } catch {
        authedUser = null;
      }
      if (cancelled) return;

      if (!authedUser) {
        // Not signed in. Send to login once; if we come back still unauthenticated within
        // the retry window, stop looping and offer a manual retry instead.
        const tried = Number(sessionStorage.getItem(LOGIN_TRIED_KEY) || 0);
        if (tried && Date.now() - tried < LOGIN_RETRY_WINDOW_MS) {
          setFailed(true);
          return;
        }
        sessionStorage.setItem(LOGIN_TRIED_KEY, String(Date.now()));
        window.location.href = `/login?return_to=${encodeURIComponent(toRelative(returnTo))}`;
        return;
      }

      // Signed in — clear the loop guard and go to the destination. We never auto-issue an
      // authorization code here: the /oauth/consent screen performs the actual authorization
      // only after the user explicitly approves the client.
      sessionStorage.removeItem(LOGIN_TRIED_KEY);
      let dest = toRelative(returnTo);
      // A raw /oauth/authorize return_to (legacy/edge) routes to the consent screen.
      if (dest.startsWith('/oauth/authorize')) {
        dest = '/oauth/consent' + dest.slice('/oauth/authorize'.length);
      }
      window.location.href = isSafeRelativePath(dest) ? dest : '/dashboard';
    })();

    return () => {
      cancelled = true;
    };
  }, [returnTo, refreshUser]);

  if (!returnTo) return null;
  if (failed) {
    return <AuthorizingScreen message="We couldn't finish signing you in." showRetry />;
  }
  return <AuthorizingScreen message={isAuthorize ? 'Authorizing…' : 'Signing you in…'} />;
}
