'use client';

import { useMemo, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { useRouter } from 'next/navigation';
import { ChevronLeft, KeyRound, Home, Plus, Trash2, AlertCircle } from 'lucide-react';
import { api } from '@/lib/api';
import { AccessToken, Workspace, McpServerMinimal } from '@/types';
import { Button } from '@/components/ui/button';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { useSetPageHeader } from '../../page-header';

// Constants
const MS_PER_MINUTE = 60 * 1000;
const MS_PER_HOUR = 60 * MS_PER_MINUTE;
const MS_PER_DAY = 24 * MS_PER_HOUR;

export default function AccessTokensPage() {
  const t = useTranslations('accessTokens');
  const tCommon = useTranslations('common');
  const router = useRouter();
  useSetPageHeader(t('title'), <KeyRound className="w-4 h-4" />);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | null>(null);

  const { data: workspaces, isLoading: isLoadingWorkspaces, isError: isErrorWorkspaces } = useQuery<Workspace[]>({
    queryKey: ['workspaces'],
    queryFn: () => api.get('/workspaces'),
  });

  const workspaceId = selectedWorkspaceId || workspaces?.[0]?.id;

  const { data: accessTokens, isLoading: isLoadingKeys, isError: isErrorKeys } = useQuery<AccessToken[]>({
    queryKey: ['workspaces', workspaceId, 'access-tokens'],
    queryFn: () => api.get(`/workspaces/${workspaceId}/access-tokens`),
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

  const isLoading = isLoadingWorkspaces || isLoadingKeys;
  const isError = isErrorWorkspaces || isErrorKeys;

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
          onClick={() => router.push('/dashboard/auth/access-tokens/new')}
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

      {/* API Keys List */}
      <div className="max-w-4xl">
        <h2 className="text-sm font-medium text-gray-500 uppercase tracking-wider mb-4">{t('yourKeys')}</h2>

        {isLoading ? (
          <div className="space-y-3">
            {[...Array(3)].map((_, i) => (
              <div key={i} className="h-20 bg-gray-100 animate-pulse rounded-xl" />
            ))}
          </div>
        ) : isError ? (
          <div className="py-16 text-center">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-red-100 flex items-center justify-center">
              <AlertCircle className="w-8 h-8 text-red-400" />
            </div>
            <p className="text-gray-500 mb-4">{t('loadError')}</p>
            <button
              onClick={() => window.location.reload()}
              className="text-sm text-violet-600 hover:text-violet-700"
            >
              {tCommon('retry')}
            </button>
          </div>
        ) : accessTokens?.length === 0 ? (
          <div className="py-16 text-center">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-100 flex items-center justify-center">
              <KeyRound className="w-8 h-8 text-gray-400" />
            </div>
            <p className="text-gray-500">{t('empty')}</p>
          </div>
        ) : (
          <div>
            {accessTokens?.map((token, index) => (
              <AccessTokenRow
                key={token.id}
                token={token}
                workspaceId={workspaceId!}
                serverName={token.server_id ? serverMap.get(token.server_id) : undefined}
                t={t}
                isFirst={index === 0}
                isLast={index === accessTokens.length - 1}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function AccessTokenRow({
  token,
  workspaceId,
  serverName,
  t,
  isFirst,
  isLast
}: {
  token: AccessToken;
  workspaceId: string;
  serverName?: string;
  t: (key: string, values?: Record<string, string | number>) => string;
  isFirst: boolean;
  isLast: boolean;
}) {
  const queryClient = useQueryClient();
  const tCommon = useTranslations('common');
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [removing, setRemoving] = useState(false);

  const queryKey = useMemo(
    () => ['workspaces', workspaceId, 'access-tokens'] as const,
    [workspaceId]
  );

  const deleteMutation = useMutation({
    mutationFn: () => api.delete(`/workspaces/${workspaceId}/access-tokens/${token.id}`),
    // Optimistic removal: drop the token from the cache immediately so the row is
    // gone regardless of the server response; roll back if the request fails.
    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey });
      const previous = queryClient.getQueryData<AccessToken[]>(queryKey);
      queryClient.setQueryData<AccessToken[]>(queryKey, (old) =>
        old?.filter((x) => x.id !== token.id) ?? []
      );
      return { previous };
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(queryKey, context.previous);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey });
    },
  });

  // "はい" pressed: start the collapse animation. The actual delete fires once the
  // collapse transition finishes (onExitDone), so the row visually leaves first.
  const handleConfirm = () => {
    setConfirmOpen(false);
    setRemoving(true);
  };

  const onExitDone = (e: React.TransitionEvent<HTMLDivElement>) => {
    if (removing && e.propertyName === 'grid-template-rows') {
      deleteMutation.mutate();
    }
  };

  const formatLastUsed = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / MS_PER_MINUTE);
    const diffHours = Math.floor(diffMs / MS_PER_HOUR);
    const diffDays = Math.floor(diffMs / MS_PER_DAY);

    if (diffMins < 1) return '今';
    if (diffMins < 60) return `${diffMins}分前`;
    if (diffHours < 24) return `${diffHours}時間前`;
    if (diffDays < 7) return `${diffDays}日前`;
    return date.toLocaleDateString();
  };

  return (
    <div
      onTransitionEnd={onExitDone}
      className={`grid transition-all duration-300 ease-in-out ${
        removing ? 'grid-rows-[0fr] opacity-0 -translate-x-4' : 'grid-rows-[1fr]'
      }`}
    >
      <div className="overflow-hidden">
        <div
          className={`group flex items-center gap-4 px-4 py-2 bg-white border-x border-b border-gray-200 hover:bg-gray-50 transition-colors ${
            isFirst ? 'border-t rounded-t-lg' : ''
          } ${isLast ? 'rounded-b-lg' : ''}`}
        >
      {/* Key Icon */}
      <div className="w-8 h-8 flex items-center justify-center flex-shrink-0">
        <KeyRound className="w-4 h-4 text-gray-500" />
      </div>

      {/* Main Content */}
      <div className="flex-1 min-w-0">
        {/* Top Row: Name + Last Used */}
        <div className="flex items-center justify-between">
          <span className="font-medium text-gray-900">{token.name}</span>
          <div className="flex items-center gap-3 text-xs text-gray-400">
            <span>{new Date(token.created_at).toLocaleDateString()}に作成</span>
            {token.last_used_at && (
              <>
                <span className="text-gray-300">•</span>
                <span>{formatLastUsed(token.last_used_at)}に使用</span>
              </>
            )}
          </div>
        </div>
        {/* Bottom Row: Key Prefix + Server + Scopes */}
        <div className="flex items-center gap-2 mt-0.5">
          <code className="text-xs text-gray-400 font-mono">{token.key_prefix}...</code>
          <span className="text-gray-300">•</span>
          <span className="px-2 py-0.5 text-xs bg-blue-50 text-blue-600 border border-blue-200 rounded">
            {serverName || t('create.allServers')}
          </span>
          <span className="text-gray-300">•</span>
          <div className="flex items-center gap-1">
            {token.scopes?.includes('*') ? (
              <span className="px-2 py-0.5 text-xs font-medium bg-violet-100 text-violet-700 border border-violet-300 rounded">
                {t('scopes.fullAccess')}
              </span>
            ) : (
              <>
                {token.scopes?.slice(0, 2).map((scope) => (
                  <span key={scope} className="px-2 py-0.5 text-xs font-mono bg-gray-100 text-gray-600 rounded">
                    {scope}
                  </span>
                ))}
                {token.scopes && token.scopes.length > 2 && (
                  <span className="px-1.5 py-0.5 text-xs text-gray-400">
                    +{token.scopes.length - 2}
                  </span>
                )}
              </>
            )}
          </div>
        </div>
      </div>

      {/* Delete Button */}
      <button
        onClick={() => setConfirmOpen(true)}
        disabled={deleteMutation.isPending || removing}
        className="p-2 text-gray-300 hover:text-red-600 hover:bg-red-50 rounded-md transition-colors"
        title={t('revoke')}
      >
        <Trash2 className="w-4 h-4" />
      </button>

      {/* Confirm revoke modal */}
      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent className="max-w-[calc(100%-2rem)] sm:max-w-md mx-4 sm:mx-auto">
          <AlertDialogHeader>
            <AlertDialogTitle>{t('revoke')}</AlertDialogTitle>
            <AlertDialogDescription>{t('revokeConfirm')}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{tCommon('cancel')}</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleConfirm}
              className="bg-red-600 hover:bg-red-700"
            >
              {tCommon('delete')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
        </div>
      </div>
    </div>
  );
}
