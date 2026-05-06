'use client';

import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useState } from 'react';
import { Toaster } from 'sonner';

// Query cache configuration
const STALE_TIME_MS = 5 * 60 * 1000; // 5 minutes - reduces unnecessary refetches
const GC_TIME_MS = 30 * 60 * 1000; // 30 minutes - keep unused data in cache longer

export function Providers({ children }: { children: React.ReactNode }) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            staleTime: STALE_TIME_MS,
            gcTime: GC_TIME_MS, // Keep data in cache longer for faster navigation
            refetchOnWindowFocus: false,
            refetchOnMount: false, // Don't refetch if data exists and isn't stale
            retry: 1, // Reduce retries for faster failure detection
            retryDelay: 1000, // Fixed 1s retry delay
          },
        },
      })
  );

  return (
    <QueryClientProvider client={queryClient}>
      {children}
      <Toaster position="bottom-right" richColors />
    </QueryClientProvider>
  );
}
