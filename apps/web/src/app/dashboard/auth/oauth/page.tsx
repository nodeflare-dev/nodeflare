'use client';

import { useMemo, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslations } from 'next-intl';
import { useRouter } from 'next/navigation';
import { ChevronLeft, Aperture, Home, Plus, Check, X, Search, ChevronDown, Trash2 } from 'lucide-react';
import { api } from '@/lib/api';
import { Workspace, McpServerMinimal } from '@/types';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { format } from 'date-fns';
import { useSetPageHeader } from '../../page-header';

// Constants
const COPY_FEEDBACK_DURATION_MS = 2000;

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
  const tCommon = useTranslations('common');
  const tApiErrors = useTranslations('apiErrors');
  const router = useRouter();
  const queryClient = useQueryClient();
  useSetPageHeader(t('title'), <Aperture className="w-4 h-4" />);
  const [showCreate, setShowCreate] = useState(false);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string | null>(null);
  const [newlyCreatedApp, setNewlyCreatedApp] = useState<OAuthApp | null>(null);
  const [copiedId, setCopiedId] = useState(false);
  const [copiedSecret, setCopiedSecret] = useState(false);

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

  const handleAppCreated = (app: OAuthApp) => {
    setNewlyCreatedApp(app);
    setShowCreate(false);
    queryClient.invalidateQueries({ queryKey: ['workspaces', workspaceId, 'oauth-apps'] });
  };

  const handleCopyClientId = () => {
    if (newlyCreatedApp) {
      navigator.clipboard.writeText(newlyCreatedApp.client_id);
      setCopiedId(true);
      setTimeout(() => setCopiedId(false), COPY_FEEDBACK_DURATION_MS);
    }
  };

  const handleCopySecret = () => {
    if (newlyCreatedApp?.client_secret) {
      navigator.clipboard.writeText(newlyCreatedApp.client_secret);
      setCopiedSecret(true);
      setTimeout(() => setCopiedSecret(false), COPY_FEEDBACK_DURATION_MS);
    }
  };

  const isLoading = isLoadingWorkspaces || isLoadingApps;

  return (
    <div className="max-w-4xl">
      {/* Back Button */}
      <button
        onClick={() => router.push('/dashboard/auth')}
        className="flex items-center gap-1 text-sm text-gray-500 hover:text-gray-700 mb-4 transition-colors"
      >
        <ChevronLeft className="w-4 h-4" />
        {t('backToAuth')}
      </button>

      {/* Header */}
      <div className="flex items-center justify-between mb-8">
        <div className="flex items-center space-x-4">
          {workspaces && workspaces.length > 1 && (
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-100 border border-gray-200">
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
          )}
        </div>
        {!showCreate && (
          <Button
            size="sm"
            onClick={() => setShowCreate(true)}
            disabled={!workspaceId}
            className="h-7 text-xs px-2.5 bg-violet-600 hover:bg-violet-700 border border-violet-900 text-white"
          >
            <Plus className="w-3.5 h-3.5 mr-1" />
            {t('new')}
          </Button>
        )}
      </div>

      {/* Newly Created App Secret Display */}
      {newlyCreatedApp && (
        <div className="mb-8 p-5 rounded-2xl bg-gradient-to-r from-emerald-50 to-teal-50 border border-emerald-200">
          <div className="flex items-start gap-4">
            <div className="w-10 h-10 rounded-full flex items-center justify-center flex-shrink-0">
              <Check className="w-5 h-5 text-emerald-600" />
            </div>
            <div className="flex-1 min-w-0">
              <p className="font-medium text-emerald-800">{t('created.title')}</p>
              <p className="text-sm text-emerald-700 mt-1">{t('created.warning')}</p>

              <div className="mt-4 space-y-3">
                <div>
                  <label className="block text-xs font-medium text-emerald-700 mb-1">Client ID</label>
                  <div className="flex items-center gap-2">
                    <code className="flex-1 px-3 py-2 bg-white rounded-lg border border-emerald-200 text-sm font-mono text-gray-800 truncate">
                      {newlyCreatedApp.client_id}
                    </code>
                    <Button
                      size="sm"
                      variant={copiedId ? "default" : "outline"}
                      className={copiedId ? "bg-emerald-600 hover:bg-emerald-600" : ""}
                      onClick={handleCopyClientId}
                    >
                      {copiedId ? "Copied!" : tCommon('copy')}
                    </Button>
                  </div>
                </div>
                <div>
                  <label className="block text-xs font-medium text-emerald-700 mb-1">Client Secret</label>
                  <div className="flex items-center gap-2">
                    <code className="flex-1 px-3 py-2 bg-white rounded-lg border border-emerald-200 text-sm font-mono text-gray-800 truncate">
                      {newlyCreatedApp.client_secret}
                    </code>
                    <Button
                      size="sm"
                      variant={copiedSecret ? "default" : "outline"}
                      className={copiedSecret ? "bg-emerald-600 hover:bg-emerald-600" : ""}
                      onClick={handleCopySecret}
                    >
                      {copiedSecret ? "Copied!" : tCommon('copy')}
                    </Button>
                  </div>
                </div>
              </div>

              <button
                onClick={() => setNewlyCreatedApp(null)}
                className="mt-4 text-sm text-emerald-600 hover:text-emerald-800"
              >
                {t('created.dismiss')}
              </button>
            </div>
            <button
              onClick={() => setNewlyCreatedApp(null)}
              className="text-emerald-400 hover:text-emerald-600 transition-colors"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>
      )}

      {/* Create Form */}
      {showCreate && workspaceId && (
        <CreateOAuthAppForm
          workspaceId={workspaceId}
          onClose={() => setShowCreate(false)}
          onCreated={handleAppCreated}
          t={t}
          tCommon={tCommon}
          tApiErrors={tApiErrors}
        />
      )}

      {/* OAuth Apps List */}
      <div>
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

function CreateOAuthAppForm({
  workspaceId,
  onClose,
  onCreated,
  t,
  tCommon,
  tApiErrors,
}: {
  workspaceId: string;
  onClose: () => void;
  onCreated: (app: OAuthApp) => void;
  t: (key: string) => string;
  tCommon: (key: string) => string;
  tApiErrors: (key: string) => string;
}) {
  const queryClient = useQueryClient();
  const [name, setName] = useState('');
  const [redirectUri, setRedirectUri] = useState('https://claude.ai/api/mcp/auth_callback');
  const [selectedServerId, setSelectedServerId] = useState<string | null>(null);
  const [selectedScopes, setSelectedScopes] = useState<string[]>(['*']);
  const [customScope, setCustomScope] = useState('');
  const [serverSearchQuery, setServerSearchQuery] = useState('');
  const [isServerListOpen, setIsServerListOpen] = useState(false);

  // Fetch servers for this workspace (minimal data only)
  const { data: servers } = useQuery<McpServerMinimal[]>({
    queryKey: ['servers-minimal'],
    queryFn: () => api.get('/servers/minimal'),
  });

  // Filter servers for this workspace
  const workspaceServers = useMemo(
    () => servers?.filter(s => s.workspace_id === workspaceId) || [],
    [servers, workspaceId]
  );

  // Filter servers based on search query
  const filteredServers = useMemo(() => {
    const query = serverSearchQuery.toLowerCase();
    return workspaceServers.filter(
      (server) => server.name.toLowerCase().includes(query)
    );
  }, [workspaceServers, serverSearchQuery]);

  // Get selected server details
  const selectedServer = useMemo(
    () => selectedServerId && selectedServerId !== 'all'
      ? workspaceServers.find(s => s.id === selectedServerId)
      : null,
    [selectedServerId, workspaceServers]
  );

  const handleSelectServer = (serverId: string) => {
    setSelectedServerId(serverId);
    setServerSearchQuery('');
    setIsServerListOpen(false);
  };

  const SCOPE_OPTIONS = [
    { id: 'tools', label: 'Tools', scope: 'tools:*', desc: t('scopes.toolsDesc') },
    { id: 'resources', label: 'Resources', scope: 'resources:*', desc: t('scopes.resourcesDesc') },
    { id: 'prompts', label: 'Prompts', scope: 'prompts:*', desc: t('scopes.promptsDesc') },
  ];

  const createMutation = useMutation({
    mutationFn: async (data: { client_name: string; redirect_uris: string[]; server_id?: string; scopes: string[] }): Promise<OAuthApp> =>
      api.post(`/workspaces/${workspaceId}/oauth-apps`, data) as Promise<OAuthApp>,
    onSuccess: (response) => {
      queryClient.invalidateQueries({ queryKey: ['workspaces', workspaceId, 'oauth-apps'] });
      onCreated(response);
    },
  });

  const toggleScope = (scope: string) => {
    if (scope === '*') {
      setSelectedScopes((prev) => prev.includes('*') ? [] : ['*']);
    } else {
      setSelectedScopes((prev) => {
        const filtered = prev.filter((s) => s !== '*');
        if (filtered.includes(scope)) {
          return filtered.filter((s) => s !== scope);
        } else {
          return [...filtered, scope];
        }
      });
    }
  };

  const addCustomScope = () => {
    if (customScope && !selectedScopes.includes(customScope)) {
      setSelectedScopes((prev) => {
        const filtered = prev.filter((s) => s !== '*');
        return [...filtered, customScope];
      });
      setCustomScope('');
    }
  };

  const createErrorMessage = useMemo(() => {
    if (!createMutation.isError) return null;
    const error = createMutation.error as any;
    const errorCode = error?.code;
    if (errorCode) {
      try {
        const translated = tApiErrors(errorCode);
        if (translated && translated !== errorCode) {
          return translated;
        }
      } catch {
        // Translation not found
      }
    }
    return error?.message || tCommon('error');
  }, [createMutation.isError, createMutation.error, tApiErrors, tCommon]);

  const removeScope = (scope: string) => {
    setSelectedScopes((prev) => {
      const result = prev.filter((s) => s !== scope);
      return result.length === 0 ? ['*'] : result;
    });
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!selectedServerId) return;
    createMutation.mutate({
      client_name: name,
      redirect_uris: redirectUri ? [redirectUri] : [],
      server_id: selectedServerId === 'all' ? undefined : selectedServerId,
      scopes: selectedScopes,
    });
  };

  return (
    <div className="mb-8 p-6 rounded-2xl bg-gray-50 border border-gray-200">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-sm font-medium text-gray-400 uppercase tracking-wide">{t('create.title')}</h2>
        <button onClick={onClose} className="text-gray-400 hover:text-gray-600 transition-colors">
          <X className="w-5 h-5" />
        </button>
      </div>

      <form onSubmit={handleSubmit} className="space-y-6">
        <div>
          <Label htmlFor="name" className="text-gray-700">{t('create.name')}</Label>
          <Input
            id="name"
            placeholder={t('create.namePlaceholder')}
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
            className="mt-2 bg-white"
          />
        </div>

        <div>
          <Label className="text-gray-700">{t('create.server')}</Label>
          <div className="mt-2 relative">
            {/* Selected Server Display / Trigger */}
            <button
              type="button"
              onClick={() => setIsServerListOpen(!isServerListOpen)}
              className="w-full flex items-center justify-between px-4 py-3 rounded-xl border border-gray-200 bg-white hover:border-gray-300 transition-colors text-left"
            >
              {selectedServerId ? (
                <span className="font-medium text-gray-900">
                  {selectedServerId === 'all' ? t('create.allServers') : selectedServer?.name}
                </span>
              ) : (
                <span className="text-gray-400">{t('create.selectServer')}</span>
              )}
              <ChevronDown className={`w-5 h-5 text-gray-400 transition-transform ${isServerListOpen ? 'rotate-180' : ''}`} />
            </button>

            {/* Dropdown List */}
            {isServerListOpen && (
              <div className="absolute z-10 mt-2 w-full rounded-xl border border-gray-200 bg-white shadow-lg overflow-hidden">
                <div className="p-3 border-b border-gray-100">
                  <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-gray-50">
                    <Search className="w-4 h-4 text-gray-400" />
                    <input
                      type="text"
                      placeholder={t('create.searchServers')}
                      value={serverSearchQuery}
                      onChange={(e) => setServerSearchQuery(e.target.value)}
                      className="flex-1 bg-transparent text-sm focus:outline-none"
                    />
                  </div>
                </div>
                <div className="max-h-64 overflow-y-auto divide-y divide-gray-100">
                  {/* All Servers Option */}
                  <button
                    type="button"
                    onClick={() => handleSelectServer('all')}
                    className={`w-full flex items-center justify-between p-3 transition-colors text-left ${
                      selectedServerId === 'all'
                        ? 'bg-violet-50'
                        : 'hover:bg-gray-50'
                    }`}
                  >
                    <span className="font-medium text-gray-900">{t('create.allServers')}</span>
                    <div className="flex items-center gap-2 flex-shrink-0">
                      <span className="px-2 py-0.5 text-xs rounded-full bg-gray-100 text-gray-600">
                        {workspaceServers.length}
                      </span>
                      {selectedServerId === 'all' && (
                        <Check className="w-5 h-5 text-violet-600" />
                      )}
                    </div>
                  </button>

                  {/* Individual Servers */}
                  {filteredServers.length === 0 ? (
                    <div className="p-6 text-center text-gray-500">
                      <p className="text-sm">{serverSearchQuery ? t('create.noServersFound') : t('create.noServersAvailable')}</p>
                    </div>
                  ) : (
                    filteredServers.map((server) => (
                      <button
                        key={server.id}
                        type="button"
                        onClick={() => handleSelectServer(server.id)}
                        className={`w-full flex items-center justify-between p-3 transition-colors text-left ${
                          selectedServerId === server.id
                            ? 'bg-violet-50'
                            : 'hover:bg-gray-50'
                        }`}
                      >
                        <span className="font-medium text-gray-900 truncate">{server.name}</span>
                        {selectedServerId === server.id && (
                          <Check className="w-5 h-5 text-violet-600 flex-shrink-0" />
                        )}
                      </button>
                    ))
                  )}
                </div>
              </div>
            )}
          </div>
        </div>

        <div>
          <Label htmlFor="redirectUri" className="text-gray-700">{t('create.redirectUri')}</Label>
          <Input
            id="redirectUri"
            placeholder="https://claude.ai/api/mcp/auth_callback"
            value={redirectUri}
            onChange={(e) => setRedirectUri(e.target.value)}
            className="mt-2 bg-white"
          />
          <p className="text-xs text-gray-500 mt-2">{t('create.redirectUriHelp')}</p>
        </div>

        <div>
          <Label className="text-gray-700 mb-3 block">{t('scopes.title')}</Label>

          <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
            {/* Full Access */}
            <div
              onClick={() => toggleScope('*')}
              className="flex items-center gap-2 cursor-pointer select-none"
            >
              <span className="text-sm font-medium text-gray-700">{t('scopes.fullAccess')}</span>
              <div className={`w-11 h-6 rounded-full p-0.5 transition-colors ${
                selectedScopes.includes('*') ? 'bg-violet-600' : 'bg-gray-300'
              }`}>
                <div className={`w-5 h-5 rounded-full bg-white shadow-sm transition-transform duration-200 ${
                  selectedScopes.includes('*') ? 'translate-x-5' : 'translate-x-0'
                }`} />
              </div>
            </div>

            <div className="w-px h-6 bg-gray-200" />

            {/* Individual Scopes */}
            {SCOPE_OPTIONS.map((option) => {
              const isChecked = selectedScopes.includes(option.scope) || selectedScopes.includes('*');
              const isDisabled = selectedScopes.includes('*');
              return (
                <div
                  key={option.id}
                  onClick={() => !isDisabled && toggleScope(option.scope)}
                  className={`flex items-center gap-2 select-none ${isDisabled ? 'opacity-40 cursor-not-allowed' : 'cursor-pointer'}`}
                >
                  <span className="text-sm font-medium text-gray-700">{option.label}</span>
                  <div className={`w-11 h-6 rounded-full p-0.5 transition-colors ${
                    isChecked ? 'bg-violet-600' : 'bg-gray-300'
                  }`}>
                    <div className={`w-5 h-5 rounded-full bg-white shadow-sm transition-transform duration-200 ${
                      isChecked ? 'translate-x-5' : 'translate-x-0'
                    }`} />
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        <div>
          <Label htmlFor="customScope" className="text-gray-700">{t('customScope')}</Label>
          <div className="flex gap-2 mt-2">
            <Input
              id="customScope"
              placeholder="tools:call:specific_tool_name"
              value={customScope}
              onChange={(e) => setCustomScope(e.target.value)}
              className="bg-white"
            />
            <Button type="button" variant="outline" onClick={addCustomScope}>
              {tCommon('add')}
            </Button>
          </div>
          <p className="text-xs text-gray-500 mt-2">{t('customScopeExamples')}</p>
        </div>

        {selectedScopes.length > 0 && !selectedScopes.includes('*') && (
          <div>
            <Label className="text-gray-700 mb-2 block">{t('scopes.selected')}</Label>
            <div className="flex flex-wrap gap-1.5">
              {selectedScopes.map((scope) => (
                <span
                  key={scope}
                  className="inline-flex items-center gap-1.5 px-2.5 py-1 text-sm bg-violet-100 text-violet-700 rounded-md"
                >
                  <code className="text-xs font-mono">{scope}</code>
                  <button
                    type="button"
                    onClick={() => removeScope(scope)}
                    className="text-violet-400 hover:text-violet-600 transition-colors"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                </span>
              ))}
            </div>
          </div>
        )}

        <div className="flex justify-end gap-2.5 pt-4 border-t border-gray-200">
          <Button
            type="button"
            variant="outline"
            onClick={onClose}
            className="h-10 px-4 rounded-lg border-[#d1d5db] text-[#374151] text-sm font-medium hover:bg-[#f3f4f6] transition-colors duration-200"
          >
            {tCommon('cancel')}
          </Button>
          <Button
            type="submit"
            disabled={createMutation.isPending || !selectedServerId}
            className="h-10 px-4 rounded-lg bg-violet-500 hover:bg-violet-600 border border-violet-600 text-white text-sm font-medium transition-colors duration-200"
          >
            {createMutation.isPending ? t('create.creating') : t('create.submit')}
          </Button>
        </div>

        {createMutation.isError && (
          <p className="text-sm text-red-600">{createErrorMessage}</p>
        )}
      </form>
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
      className={`group flex items-center gap-4 px-4 py-3 bg-white border-x border-b border-gray-200 hover:bg-gray-50 transition-colors ${
        isFirst ? 'border-t rounded-t-lg' : ''
      } ${isLast ? 'rounded-b-lg' : ''}`}
    >
      {/* OAuth Icon */}
      <div className="w-8 h-8 rounded-lg bg-gray-100 flex items-center justify-center flex-shrink-0">
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
        <div className="flex items-center gap-2 mt-1.5">
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
