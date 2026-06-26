'use client';

import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { useAuth } from '@/hooks/use-auth';
import { Workspace } from '@/types';

const STORAGE_KEY = 'activeWorkspaceId';

interface WorkspaceContextValue {
  workspaces: Workspace[] | undefined;
  activeWorkspace: Workspace | undefined;
  activeWorkspaceId: string | null;
  setActiveWorkspaceId: (id: string) => void;
  isLoading: boolean;
}

const WorkspaceContext = createContext<WorkspaceContextValue | undefined>(undefined);

export function WorkspaceProvider({ children }: { children: React.ReactNode }) {
  const { user } = useAuth();
  const queryClient = useQueryClient();
  const [activeWorkspaceId, setActiveWorkspaceIdState] = useState<string | null>(null);

  // Hydrate the stored active workspace id on the client (avoids SSR mismatch).
  useEffect(() => {
    if (typeof window === 'undefined') return;
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) setActiveWorkspaceIdState(stored);
  }, []);

  const { data: workspaces, isLoading } = useQuery<Workspace[]>({
    queryKey: ['workspaces'],
    queryFn: () => api.get<Workspace[]>('/workspaces'),
    enabled: !!user,
  });

  // When the list loads, make sure the active id points at a real workspace.
  // Falls back to the first workspace if the stored id is missing/stale.
  useEffect(() => {
    if (!workspaces || workspaces.length === 0) return;
    const exists = activeWorkspaceId && workspaces.some((w) => w.id === activeWorkspaceId);
    if (!exists) {
      const fallback = workspaces[0].id;
      setActiveWorkspaceIdState(fallback);
      if (typeof window !== 'undefined') localStorage.setItem(STORAGE_KEY, fallback);
    }
  }, [workspaces, activeWorkspaceId]);

  const setActiveWorkspaceId = useCallback(
    (id: string) => {
      setActiveWorkspaceIdState(id);
      if (typeof window !== 'undefined') localStorage.setItem(STORAGE_KEY, id);
      // Workspace-scoped query keys include the active id, so changing it already
      // refetches scoped data. Invalidate to also refresh any in-flight/cached views.
      queryClient.invalidateQueries();
    },
    [queryClient]
  );

  // Resolve the active workspace, defaulting to the first one until reconciled.
  const activeWorkspace = useMemo(
    () => workspaces?.find((w) => w.id === activeWorkspaceId) ?? workspaces?.[0],
    [workspaces, activeWorkspaceId]
  );

  const value = useMemo<WorkspaceContextValue>(
    () => ({
      workspaces,
      activeWorkspace,
      activeWorkspaceId,
      setActiveWorkspaceId,
      isLoading,
    }),
    [workspaces, activeWorkspace, activeWorkspaceId, setActiveWorkspaceId, isLoading]
  );

  return <WorkspaceContext.Provider value={value}>{children}</WorkspaceContext.Provider>;
}

export function useWorkspace(): WorkspaceContextValue {
  const ctx = useContext(WorkspaceContext);
  if (!ctx) {
    throw new Error('useWorkspace must be used within a WorkspaceProvider');
  }
  return ctx;
}
