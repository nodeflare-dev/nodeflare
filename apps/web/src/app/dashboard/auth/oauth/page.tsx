'use client';

import { useMemo, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { useRouter } from 'next/navigation';
import { ChevronLeft, Aperture, Home, Plus, Trash2 } from 'lucide-react';
import { api } from '@/lib/api';
import { Workspace, McpServerMinimal } from '@/types';
import { Button } from '@/components/ui/button';
import { format } from 'date-fns';
import { useSetPageHeader } from '../../page-header';

interface OAuthApp {
  id: string;
  client_id: string;
  client_secret?: string;
  client_name: string;
  redirect_uris: string[];
  server_id?: string;
  scopes: string[];
  created_at: string;
}

export default function OAuthAppsPage() {
  const t = useTranslations('oauth');
  const router = useRouter();
  const queryClient = useQueryClient();
  useSetPageHeader(t('title'), <Aperture className="w-4 h-4" />);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | null>(null);

  const { data: workspaces, isLoading: isLoadingWorkspaces } = useQuery<Workspace[]>({
    queryKey: ['workspaces'],
    queryFn: () => api.get('/workspaces'),
  });

  const workspaceId = selectedWorkspaceId || workspaces?.[0]?.id;

  const { data: oauthApps, isLoading: isLoadingApps } = useQuery<OAuthApp[]>({
    queryKey: ['workspaces', workspaceId, 'oauth-apps'],
    queryFn: () => api.get(`/workspaces/${workspaceId}/oauth-apps`),
    enabled: !!workspaceId,
  });

  const { data: servers } = useQuery<McpServerMinimal[]>({
    queryKey: ['servers-minimal'],
    queryFn: () => api.get('/servers/minimal'),
  });

  // Create a map for quick server name lookup
  const serverMap = useMemo(
    () => new Map(servers?.map(s => [s.id, s.name]) || []),
    [servers]
  );

  const deleteMutation = useMutation({
    mutationFn: (clientId: string) =>
      api.delete(`/workspaces/${workspaceId}/oauth-apps/${clientId}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', workspaceId, 'oauth-apps'] });
    },
  });

  const isLoading = isLoadingWorkspaces || isLoadingApps;

  return (
    <div>
      {/* Back Button + New */}
      <div className="flex items-center justify-between mb-4">
        <button
          onClick={() => router.push('/dashboard/auth')}
          className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 transition-colors"
        >
          <ChevronLeft className="w-4 h-4" />
          {t('backToAuth')}
        </button>
        <Button
          size="sm"
          onClick={() => router.push('/dashboard/auth/oauth/new')}
          disabled={!workspaceId}
          className="h-7 text-xs px-2.5 bg-violet-600 hover:bg-violet-700 border border-violet-900 text-white"
        >
          <Plus className="w-3.5 h-3.5 mr-1" />
          {t('new')}
        </Button>
      </div>

      {workspaces && workspaces.length > 1 && (
        <div className="mb-8">
          <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-100 border border-gray-200 w-fit">
            <Home className="w-4 h-4 text-gray-500" />
              <select
                className="bg-transparent text-sm font-medium text-gray-700 focus:outline-none cursor-pointer pr-6 appearance-none"
                value={workspaceId || ''}
                onChange={(e) => setSelectedWorkspaceId(e.target.value)}
                style={{ backgroundImage: `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='%236b7280' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M6 9l6 6 6-6'/%3E%3C/svg%3E")`, backgroundRepeat: 'no-repeat', backgroundPosition: 'right 0 center' }}
              >
                {workspaces.map((ws) => (
                  <option key={ws.id} value={ws.id}>{ws.name}</option>
                ))}
              </select>
            </div>
          </div>
        )}

      {/* OAuth Apps List */}
      <div className="max-w-4xl">
        <h2 className="text-sm font-medium text-gray-500 uppercase tracking-wider mb-4">{t('yourApps')}</h2>

        {isLoading ? (
          <div className="space-y-3">
            {[...Array(3)].map((_, i) => (
              <div key={i} className="h-20 bg-gray-100 animate-pulse rounded-xl" />
            ))}
          </div>
        ) : oauthApps && oauthApps.length > 0 ? (
          <div>
            {oauthApps.map((app, index) => (
              <OAuthAppRow
                key={app.id}
                app={app}
                serverName={app.server_id ? serverMap.get(app.server_id) : undefined}
                t={t}
                isFirst={index === 0}
                isLast={index === oauthApps.length - 1}
                onDelete={() => {
                  if (confirm(t('delete.confirm'))) {
                    deleteMutation.mutate(app.id);
                  }
                }}
                isDeleting={deleteMutation.isPending}
              />
            ))}
          </div>
        ) : (
          <div className="py-16 text-center">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-100 flex items-center justify-center">
              <Aperture className="w-8 h-8 text-gray-400" />
            </div>
            <h2 className="text-lg font-medium text-gray-700 mb-2">{t('empty.title')}</h2>
            <p className="text-gray-500 max-w-md mx-auto">{t('empty.description')}</p>
          </div>
        )}
      </div>
    </div>
  );
}

function OAuthAppRow({
  app,
  serverName,
  t,
  isFirst,
  isLast,
  onDelete,
  isDeleting,
}: {
  app: OAuthApp;
  serverName?: string;
  t: (key: string, values?: Record<string, string | number>) => string;
  isFirst: boolean;
  isLast: boolean;
  onDelete: () => void;
  isDeleting: boolean;
}) {
  return (
    <div
      className={`group flex items-center gap-4 px-4 py-2 bg-white border-x border-b border-gray-200 hover:bg-gray-50 transition-colors ${
        isFirst ? 'border-t rounded-t-lg' : ''
      } ${isLast ? 'rounded-b-lg' : ''}`}
    >
      {/* OAuth Icon */}
      <div className="w-8 h-8 flex items-center justify-center flex-shrink-0">
        <Aperture className="w-4 h-4 text-gray-500" />
      </div>

      {/* Main Content */}
      <div className="flex-1 min-w-0">
        {/* Top Row: Name + Created Date */}
        <div className="flex items-center justify-between">
          <span className="font-medium text-gray-900">{app.client_name}</span>
          <div className="flex items-center gap-3 text-xs text-gray-400">
            <span>{format(new Date(app.created_at), 'yyyy/MM/dd')}{t('list.createdSuffix')}</span>
          </div>
        </div>
        {/* Bottom Row: Client ID + Server + Scopes */}
        <div className="flex items-center gap-2 mt-0.5">
          <code className="text-xs text-gray-400 font-mono">{app.client_id.slice(0, 16)}...</code>
          <span className="text-gray-300">•</span>
          <span className="px-2 py-0.5 text-xs bg-violet-50 text-violet-600 border border-violet-200 rounded">
            {serverName || t('create.allServers')}
          </span>
          <span className="text-gray-300">•</span>
          <div className="flex items-center gap-1">
            {app.scopes?.includes('*') ? (
              <span className="px-2 py-0.5 text-xs font-medium bg-violet-100 text-violet-700 border border-violet-300 rounded">
                {t('scopes.fullAccess')}
              </span>
            ) : (
              <>
                {app.scopes?.slice(0, 2).map((scope) => (
                  <span key={scope} className="px-2 py-0.5 text-xs font-mono bg-gray-100 text-gray-600 rounded">
                    {scope}
                  </span>
                ))}
                {app.scopes && app.scopes.length > 2 && (
                  <span className="px-1.5 py-0.5 text-xs text-gray-400">
                    +{app.scopes.length - 2}
                  </span>
                )}
              </>
            )}
          </div>
        </div>
      </div>

      {/* Delete Button */}
      <button
        onClick={onDelete}
        disabled={isDeleting}
        className="p-2 text-gray-300 hover:text-red-600 hover:bg-red-50 rounded-md transition-colors"
        title={t('delete.title')}
      >
        <Trash2 className="w-4 h-4" />
      </button>
    </div>
  );
}
