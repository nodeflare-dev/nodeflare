import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useState, useEffect, useSyncExternalStore } from 'react';
import { api } from '@/lib/api';
import { User } from '@/types';

// リフレッシュ状態を監視するためのストア（イベントベース）
function subscribeToRefreshState(callback: () => void) {
  // ポーリングではなくイベントベースで状態変更を購読
  return api.subscribeToRefreshState(callback);
}

function getRefreshingSnapshot() {
  return api.isRefreshing;
}

function getRefreshingServerSnapshot() {
  return false;
}

const AUTH_CACHE_KEY = 'auth_user_cache';
// Cache expiration time (14 days) - matches refresh token validity period
// This allows instant UI display while API validates in background
// Only display data (id, name, avatar) is cached, not auth tokens
const CACHE_MAX_AGE_MS = 14 * 24 * 60 * 60 * 1000;

// Note: HTTP-Only cookies cannot be read from JavaScript
// We always attempt the API call and let the server validate the cookie

// SECURITY NOTE: User data in localStorage is accessible to JavaScript and
// could be exposed via XSS. We only cache non-sensitive display data (id, name, avatar)
// and enforce expiration. Actual authentication relies on HttpOnly cookies.

// Minimal user data for display cache (excludes potentially sensitive fields)
interface CachedUserDisplay {
  id: string;
  name: string | null;
  avatar_url: string | null;
}

// Get cached user from localStorage for instant display (client-side only)
// Returns user data even if stale - API will revalidate in background
function getCachedUser(): User | null {
  if (typeof window === 'undefined') return null;
  try {
    const cached = localStorage.getItem(AUTH_CACHE_KEY);
    if (cached) {
      const { user, timestamp } = JSON.parse(cached) as {
        user: CachedUserDisplay;
        timestamp: number;
      };

      // SECURITY: Expire cache after max age to limit exposure window
      if (Date.now() - timestamp > CACHE_MAX_AGE_MS) {
        localStorage.removeItem(AUTH_CACHE_KEY);
        return null;
      }

      // Return minimal cached data with empty email (will be filled by API)
      // The actual auth state will be verified by /auth/me API call
      return {
        id: user.id,
        name: user.name,
        avatar_url: user.avatar_url,
        email: '', // Don't cache email - will be fetched from API
        created_at: '', // Don't cache - will be fetched from API
      } as User;
    }
  } catch {
    // Ignore parse errors
  }
  return null;
}

// Save user to localStorage cache
// Only caches minimal display data, not sensitive information
function setCachedUser(user: User | null) {
  if (typeof window === 'undefined') return;
  try {
    if (user) {
      // SECURITY: Only cache minimal display data
      const minimalUser: CachedUserDisplay = {
        id: user.id,
        name: user.name,
        avatar_url: user.avatar_url,
      };
      localStorage.setItem(
        AUTH_CACHE_KEY,
        JSON.stringify({ user: minimalUser, timestamp: Date.now() })
      );
    } else {
      localStorage.removeItem(AUTH_CACHE_KEY);
    }
  } catch {
    // Ignore storage errors
  }
}

export function useAuth() {
  const queryClient = useQueryClient();

  // Use state for cached user to avoid hydration mismatch
  const [cachedUser, setCachedUserState] = useState<User | null>(null);
  const [isHydrated, setIsHydrated] = useState(false);

  // リフレッシュ中かどうかを監視
  const isRefreshing = useSyncExternalStore(
    subscribeToRefreshState,
    getRefreshingSnapshot,
    getRefreshingServerSnapshot
  );

  // Load cached user only on client side after hydration
  useEffect(() => {
    setCachedUserState(getCachedUser());
    setIsHydrated(true);
  }, []);

  const {
    data: user,
    isLoading: isQueryLoading,
    error,
    refetch,
  } = useQuery<User | null>({
    queryKey: ['auth', 'me'],
    queryFn: async () => {
      try {
        const userData = await api.get<User>('/auth/me');
        setCachedUser(userData);
        return userData;
      } catch {
        setCachedUser(null);
        return null;
      }
    },
    retry: false,
    staleTime: 5 * 60 * 1000,
    enabled: isHydrated, // Only fetch after hydration
    placeholderData: cachedUser, // Show cached data immediately while fetching
  });

  // Show loading until hydrated and query completes (unless we have cached data)
  // リフレッシュ中もローディング状態として扱う（不要なログアウト遷移を防ぐ）
  const isLoading = !isHydrated || (!cachedUser && isQueryLoading) || isRefreshing;

  // リフレッシュ中でuserがnullの場合は、cachedUserをフォールバックとして使用
  const effectiveUser = user ?? (isRefreshing ? cachedUser : null);

  const logoutMutation = useMutation({
    mutationFn: async () => {
      // Call server-side logout to invalidate tokens and clear cookies
      await api.post('/auth/logout');
    },
    onSuccess: () => {
      setCachedUser(null); // Clear localStorage cache
      queryClient.setQueryData(['auth', 'me'], null);
      queryClient.invalidateQueries();
      window.location.href = '/';
    },
    onError: () => {
      // Even if server logout fails, redirect to home
      setCachedUser(null); // Clear localStorage cache
      queryClient.setQueryData(['auth', 'me'], null);
      queryClient.invalidateQueries();
      window.location.href = '/';
    },
  });

  return {
    user: effectiveUser,
    isLoading,
    error,
    isAuthenticated: !!effectiveUser,
    logout: logoutMutation.mutate,
    refreshUser: refetch,
  };
}
