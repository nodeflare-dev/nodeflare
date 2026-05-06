'use client';

import { useEffect } from 'react';
import { useSearchParams } from 'next/navigation';
import { useAuth } from '@/hooks/use-auth';

export function OAuthHandler() {
  const { user, isLoading } = useAuth();
  const searchParams = useSearchParams();
  const returnTo = searchParams.get('return_to');

  useEffect(() => {
    if (!isLoading && user && returnTo) {
      try {
        const url = new URL(returnTo, window.location.origin);
        // Check if this is an OAuth authorize request
        if (url.pathname === '/oauth/authorize') {
          // Handle OAuth authorization by calling API endpoint
          const handleOAuth = async () => {
            try {
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
                // Redirect to client with authorization code
                const redirectUri = url.searchParams.get('redirect_uri');
                const state = url.searchParams.get('state') || '';
                if (redirectUri) {
                  const separator = redirectUri.includes('?') ? '&' : '?';
                  window.location.href = `${redirectUri}${separator}code=${data.code}&state=${state}`;
                  return;
                }
              }
            } catch {
              // OAuth flow failed, stay on landing page
            }
          };
          handleOAuth();
        }
      } catch {
        // Invalid URL, ignore
      }
    }
  }, [user, isLoading, returnTo]);

  // This component doesn't render anything visible
  return null;
}
