'use client';

import { useEffect } from 'react';
import { useSearchParams } from 'next/navigation';
import Image from 'next/image';
import { useAuth } from '@/hooks/use-auth';

// SECURITY: only follow same-site relative paths (block absolute / protocol-relative).
function isSafeRelativePath(url: string): boolean {
  if (!url.startsWith('/') || url.startsWith('//')) return false;
  const lower = url.toLowerCase();
  if (lower.includes('javascript:') || lower.includes('data:') || lower.includes('vbscript:')) {
    return false;
  }
  return !/[\x00-\x1f\x7f]/.test(url);
}

/**
 * Full-screen branded "in progress" screen shown while we finish an auth/authorization
 * redirect. The cross-domain OAuth flow has to bounce back to the frontend (the session
 * cookie lives here, not on the API domain), so without this the user would briefly see
 * the raw marketing landing page.
 */
function AuthorizingScreen({ message }: { message: string }) {
  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center bg-white">
      <div className="flex w-full max-w-[260px] flex-col items-center px-6">
        <Image src="/logo2.png" alt="Nodeflare" width={153} height={32} priority className="h-9 w-auto" />
        <p className="mt-6 text-gray-700 font-medium">{message}</p>
        <div className="mt-5 h-1.5 w-full overflow-hidden rounded-full bg-violet-100">
          <div className="h-full w-2/5 rounded-full bg-gradient-to-r from-violet-400 via-violet-600 to-violet-400 animate-indeterminate" />
        </div>
      </div>
    </div>
  );
}

export function OAuthHandler() {
  const { user, isLoading } = useAuth();
  const searchParams = useSearchParams();
  const returnTo = searchParams.get('return_to');

  const isAuthorize = (() => {
    if (!returnTo) return false;
    try {
      return new URL(returnTo, window.location.origin).pathname === '/oauth/authorize';
    } catch {
      return false;
    }
  })();

  useEffect(() => {
    if (!returnTo || isLoading) return;

    // Not authenticated on the frontend either: send to login, preserving return_to so the
    // user lands back here and the flow continues after signing in.
    if (!user) {
      window.location.href = `/login?return_to=${encodeURIComponent(returnTo)}`;
      return;
    }

    let cancelled = false;
    (async () => {
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
          if (!cancelled) window.location.href = '/dashboard';
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
  }, [user, isLoading, returnTo]);

  if (!returnTo) return null;
  return <AuthorizingScreen message={isAuthorize ? 'Authorizing…' : 'Signing you in…'} />;
}
