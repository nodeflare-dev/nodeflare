'use client';

import { useEffect, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import Image from 'next/image';
import { useAuth } from '@/hooks/use-auth';

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
// survives the login round-trip's same-origin return_to validation.
function toRelative(returnTo: string): string {
  try {
    const u = new URL(returnTo, window.location.origin);
    return `${u.pathname}${u.search}`;
  } catch {
    return returnTo;
  }
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
          <div className="mt-5 h-1.5 w-full overflow-hidden rounded-full bg-violet-100">
            <div className="h-full w-2/5 rounded-full bg-gradient-to-r from-violet-400 via-violet-600 to-violet-400 animate-indeterminate" />
          </div>
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

      // Signed in — clear the loop guard and finish the flow.
      sessionStorage.removeItem(LOGIN_TRIED_KEY);
      try {
        const url = new URL(returnTo, window.location.origin);

        // nodeflare acting as an OAuth provider (e.g. Claude connecting to an MCP server):
        // exchange the session for an authorization code and bounce to the client.
        if (url.pathname === '/oauth/authorize') {
          const response = await fetch('/api/v1/oauth/authorize-code', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            credentials: 'include',
            body: JSON.stringify({
              response_type: url.searchParams.get('response_type'),
              client_id: url.searchParams.get('client_id'),
              redirect_uri: url.searchParams.get('redirect_uri'),
              code_challenge: url.searchParams.get('code_challenge'),
              code_challenge_method: url.searchParams.get('code_challenge_method') || 'S256',
              state: url.searchParams.get('state') || '',
              scope: url.searchParams.get('scope') || '*',
            }),
          });
          if (cancelled) return;
          if (response.ok) {
            const data = await response.json();
            const redirectUri = url.searchParams.get('redirect_uri');
            const clientState = url.searchParams.get('state') || '';
            if (redirectUri) {
              const separator = redirectUri.includes('?') ? '&' : '?';
              window.location.href = `${redirectUri}${separator}code=${data.code}&state=${clientState}`;
              return;
            }
          }
          // Authorization failed — fall back to the dashboard rather than hang.
          window.location.href = '/dashboard';
          return;
        }

        // Ordinary deep-link return: only follow safe same-site relative paths.
        window.location.href = isSafeRelativePath(returnTo) ? returnTo : '/dashboard';
      } catch {
        if (!cancelled) window.location.href = '/dashboard';
      }
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
