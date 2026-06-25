'use client';

import { useState, useCallback, useMemo, useEffect } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useRouter } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { Lock, Users, Globe, Server, Check, Link, Search, Folder, AlertCircle, Info, GitBranch, ChevronRight, Terminal, AlertTriangle, XCircle, Plus, ArrowRight, MonitorPlay } from 'lucide-react';
import { api } from '@/lib/api';
import { getLinkedAccounts, getRepos, LinkedGitHubAccount } from '@/lib/github-api';
import { CreateServerRequest, McpServer, Runtime, Visibility, GitHubRepo } from '@/types';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { GitHubAccountSelector } from '@/components/github/GitHubAccountSelector';
import { MemorySelect } from '@/components/servers/memory-select';
import { DEFAULT_MEMORY_MB } from '@/lib/plans';
import { SiNodedotjs, SiPython, SiGo, SiRust, SiDocker, SiGithub } from 'react-icons/si';
import { useSetPageHeader } from '../../page-header';

type SourceType = 'my-repos' | 'public-url';

export default function NewServerPage() {
  const t = useTranslations('servers');
  const tCommon = useTranslations('common');
  const tApiErrors = useTranslations('apiErrors');
  const router = useRouter();
  const queryClient = useQueryClient();

  useSetPageHeader(t('create.title'), <Server className="w-4 h-4" />);

  const { data: workspaces } = useQuery<{ id: string; name: string; plan?: string }[]>({
    queryKey: ['workspaces'],
    queryFn: () => api.get('/workspaces'),
  });

  const workspaceId = workspaces?.[0]?.id;

  // Plan limits drive which memory sizes are selectable (Free is capped at 256MB).
  const { data: plans } = useQuery<{ plan: string; limits: { max_memory_mb: number } }[]>({
    queryKey: ['billing-plans'],
    queryFn: () => api.get('/billing/plans'),
  });
  const currentPlan = workspaces?.[0]?.plan || 'free';
  const maxMemoryMb = plans?.find((p) => p.plan === currentPlan)?.limits.max_memory_mb ?? 256;

  // Linked GitHub accounts
  const { data: linkedAccounts, isLoading: accountsLoading } = useQuery<LinkedGitHubAccount[]>({
    queryKey: ['linked-github-accounts'],
    queryFn: getLinkedAccounts,
  });

  const [selectedAccountId, setSelectedAccountId] = useState<string | null>(null);

  // Auto-select primary account when accounts load
  useEffect(() => {
    if (linkedAccounts && linkedAccounts.length > 0 && !selectedAccountId) {
      const primary = linkedAccounts.find((a) => a.is_primary);
      setSelectedAccountId(primary?.id || linkedAccounts[0].id);
    }
  }, [linkedAccounts, selectedAccountId]);

  // Fetch repos from selected account
  const { data: repos, isLoading: reposLoading } = useQuery<GitHubRepo[]>({
    queryKey: ['github-repos', selectedAccountId],
    queryFn: () => getRepos(selectedAccountId || undefined),
    enabled: !!selectedAccountId,
  });

  const [sourceType, setSourceType] = useState<SourceType>('my-repos');
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedRepo, setSelectedRepo] = useState<GitHubRepo | null>(null);
  const [publicRepoUrl, setPublicRepoUrl] = useState('');
  const [publicRepoError, setPublicRepoError] = useState<string | null>(null);

  const filteredRepos = repos?.filter(
    (repo) =>
      repo.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      repo.full_name.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const [formData, setFormData] = useState<CreateServerRequest>({
    name: '',
    slug: '',
    description: '',
    github_repo: '',
    github_branch: 'main',
    runtime: 'node',
    visibility: 'private',
    transport: 'sse',
    root_directory: '',
    mcp_path: '/mcp',
    auth_enabled: true,
    memory_mb: DEFAULT_MEMORY_MB,
  });
  const [showAdvanced, setShowAdvanced] = useState(false);

  const generateSlug = useCallback((name: string) => {
    return name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '')
      .substring(0, 63);
  }, []);

  // Parse GitHub URL to extract owner/repo and, when present, the branch and
  // subdirectory from a `/tree/<branch>/<path>` or `/blob/<branch>/<path>` URL.
  // Note: branch is taken as the first segment after tree/blob, so slashed branch
  // names (e.g. `feature/x`) get split into the path — rare, and user-overridable.
  const parseGitHubUrl = useCallback((input: string): {
    owner: string;
    repo: string;
    branch?: string;
    subdir?: string;
  } | null => {
    // Full URLs, optionally pointing at a branch + path inside the repo.
    const urlMatch = input.match(
      /github\.com\/([^\/\s#?]+)\/([^\/\s#?]+)(?:\/(?:tree|blob)\/([^\/\s#?]+)(?:\/([^\s#?]+))?)?/
    );
    if (urlMatch) {
      return {
        owner: urlMatch[1],
        repo: urlMatch[2].replace(/\.git$/, ''),
        branch: urlMatch[3] || undefined,
        subdir: urlMatch[4]?.replace(/\/+$/, '') || undefined,
      };
    }
    // Handle owner/repo format
    const shortMatch = input.match(/^([^\/\s]+)\/([^\/\s]+)$/);
    if (shortMatch) {
      return { owner: shortMatch[1], repo: shortMatch[2].replace(/\.git$/, '') };
    }
    return null;
  }, []);

  // Handle public repo URL input
  const handlePublicRepoChange = useCallback((value: string) => {
    setPublicRepoUrl(value);
    setPublicRepoError(null);

    if (!value.trim()) {
      setFormData(prev => ({ ...prev, github_repo: '', name: '', slug: '' }));
      return;
    }

    const parsed = parseGitHubUrl(value);
    if (parsed) {
      // Name/slug from the subdir when the URL targets one (e.g. a monorepo
      // member like `src/filesystem`), otherwise from the repo.
      const leaf = parsed.subdir ? parsed.subdir.split('/').pop()! : parsed.repo;
      const slug = generateSlug(leaf);
      setFormData(prev => ({
        ...prev,
        github_repo: `${parsed.owner}/${parsed.repo}`,
        name: leaf,
        slug: slug,
        github_branch: parsed.branch || 'main',
        root_directory: parsed.subdir || '',
      }));
      // Surface the auto-filled root directory so the user can see/confirm it.
      if (parsed.subdir) {
        setShowAdvanced(true);
      }
    } else {
      setPublicRepoError('Invalid format. Use owner/repo or full GitHub URL');
      setFormData(prev => ({ ...prev, github_repo: '', name: '', slug: '' }));
    }
  }, [parseGitHubUrl, generateSlug]);

  const handleSelectRepo = (repo: GitHubRepo) => {
    setSelectedRepo(repo);
    const slug = generateSlug(repo.name);
    setFormData(prev => ({
      ...prev,
      name: repo.name,
      slug: slug,
      github_repo: repo.full_name,
      github_branch: repo.default_branch,
    }));
  };

  const createMutation = useMutation({
    mutationFn: (data: CreateServerRequest) => {
      if (!workspaceId) throw new Error('No workspace found');
      return api.post<McpServer>(`/workspaces/${workspaceId}/servers`, data);
    },
    onSuccess: (server) => {
      queryClient.invalidateQueries({ queryKey: ['servers'] });
      router.push(`/dashboard/servers/${server.id}`);
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    createMutation.mutate(formData);
  };

  const runtimes = useMemo(() => [
    { value: 'node', label: t('create.runtimeNode'), color: 'bg-green-600', icon: <SiNodedotjs className="w-5 h-5" /> },
    { value: 'python', label: t('create.runtimePython'), color: 'bg-blue-500', icon: <SiPython className="w-5 h-5" /> },
    { value: 'go', label: t('create.runtimeGo'), color: 'bg-cyan-500', icon: <SiGo className="w-6 h-6" /> },
    { value: 'rust', label: t('create.runtimeRust'), color: 'bg-orange-600', icon: <SiRust className="w-5 h-5" /> },
    { value: 'docker', label: t('create.runtimeDocker'), color: 'bg-sky-500', icon: <SiDocker className="w-5 h-5" /> },
  ], [t]);

  const visibilities = useMemo(() => [
    { value: 'private', label: t('create.visibilityPrivate'), desc: t('create.visibilityPrivateDesc'), icon: <Lock className="w-5 h-5" /> },
    { value: 'team', label: t('create.visibilityTeam'), desc: t('create.visibilityTeamDesc'), icon: <Users className="w-5 h-5" /> },
    { value: 'public', label: t('create.visibilityPublic'), desc: t('create.visibilityPublicDesc'), icon: <Globe className="w-5 h-5" /> },
  ], [t]);

  const errorMessage = useMemo(() => {
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
    return error?.message || t('create.failed');
  }, [createMutation.isError, createMutation.error, tApiErrors, t]);

  const errorSuggestion = useMemo(() => {
    if (!createMutation.isError) return null;
    const error = createMutation.error as any;
    return error?.details?.suggestion || null;
  }, [createMutation.isError, createMutation.error]);

  return (
    <div className="max-w-2xl">
      <form onSubmit={handleSubmit} className="space-y-8">
        {/* GitHub Repository Selection */}
        <section>
          <h2 className="text-sm font-medium text-gray-500 uppercase tracking-wider mb-4">{t('create.githubRepo')}</h2>

          {/* Source Type Cards */}
          <div className="grid grid-cols-2 gap-3 mb-4">
            <button
              type="button"
              onClick={() => {
                setSourceType('my-repos');
                setPublicRepoUrl('');
                setPublicRepoError(null);
                if (!selectedRepo) {
                  setFormData(prev => ({ ...prev, github_repo: '', name: '', slug: '' }));
                }
              }}
              className={`relative flex items-center gap-3 p-4 rounded-xl transition-all text-left ${
                sourceType === 'my-repos'
                  ? 'bg-violet-50 border border-violet-200 shadow-sm'
                  : 'bg-white border border-gray-200 hover:border-gray-300 hover:bg-gray-50'
              }`}
            >
              <div className="flex items-center justify-center flex-shrink-0">
                <SiGithub className={`w-6 h-6 ${sourceType === 'my-repos' ? 'text-violet-600' : 'text-gray-400'}`} />
              </div>
              <div className="flex-1 min-w-0">
                <span className="block font-semibold text-sm text-[#323232]">
                  {t('create.myRepos')}
                </span>
                <span className={`block text-xs mt-0.5 ${sourceType === 'my-repos' ? 'text-violet-600' : 'text-gray-500'}`}>
                  {t('create.myReposDesc')}
                </span>
              </div>
              {sourceType === 'my-repos' && (
                <div className="absolute top-3 right-3 w-5 h-5 rounded-full bg-violet-500 flex items-center justify-center">
                  <Check className="w-3 h-3 text-white" strokeWidth={3} />
                </div>
              )}
            </button>
            <button
              type="button"
              onClick={() => {
                setSourceType('public-url');
                setSelectedRepo(null);
                setFormData(prev => ({ ...prev, github_repo: '', name: '', slug: '' }));
              }}
              className={`relative flex items-center gap-3 p-4 rounded-xl transition-all text-left ${
                sourceType === 'public-url'
                  ? 'bg-violet-50 border border-violet-200 shadow-sm'
                  : 'bg-white border border-gray-200 hover:border-gray-300 hover:bg-gray-50'
              }`}
            >
              <div className="flex items-center justify-center flex-shrink-0">
                <Link className={`w-6 h-6 ${sourceType === 'public-url' ? 'text-violet-600' : 'text-gray-400'}`} />
              </div>
              <div className="flex-1 min-w-0">
                <span className="block font-semibold text-sm text-[#323232]">
                  {t('create.publicUrl')}
                </span>
                <span className={`block text-xs mt-0.5 ${sourceType === 'public-url' ? 'text-violet-600' : 'text-gray-500'}`}>
                  {t('create.publicUrlDesc')}
                </span>
              </div>
              {sourceType === 'public-url' && (
                <div className="absolute top-3 right-3 w-5 h-5 rounded-full bg-violet-500 flex items-center justify-center">
                  <Check className="w-3 h-3 text-white" strokeWidth={3} />
                </div>
              )}
            </button>
          </div>

          {sourceType === 'my-repos' ? (
            // My Repos Selection
            <div className="space-y-3">
              {/* GitHub Account Selector */}
              <div>
                <GitHubAccountSelector
                  accounts={linkedAccounts || []}
                  selectedAccountId={selectedAccountId}
                  onSelect={(id) => {
                    setSelectedAccountId(id);
                    setSelectedRepo(null);
                    setFormData(prev => ({ ...prev, github_repo: '', name: '', slug: '' }));
                  }}
                  returnTo="/dashboard/servers/new"
                  isLoading={accountsLoading}
                />
              </div>

              {selectedRepo ? (
                <div className="flex items-center justify-between p-4 rounded-xl bg-gray-50 border border-gray-200">
                  <div className="flex items-center gap-4">
                    <div className="w-12 h-12 rounded-xl bg-gray-900 flex items-center justify-center">
                      <SiGithub className="w-6 h-6 text-white" />
                    </div>
                    <div>
                      <p className="font-semibold text-gray-900">{selectedRepo.full_name}</p>
                      <div className="flex items-center gap-2 mt-1 text-sm text-gray-500">
                        {selectedRepo.private ? (
                          <span className="inline-flex items-center gap-1">
                            <Lock className="w-3.5 h-3.5" />
                            Private
                          </span>
                        ) : (
                          <span className="inline-flex items-center gap-1">
                            <Globe className="w-3.5 h-3.5" />
                            Public
                          </span>
                        )}
                        {selectedRepo.language && (
                          <>
                            <span>·</span>
                            <span>{selectedRepo.language}</span>
                          </>
                        )}
                      </div>
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      setSelectedRepo(null);
                      setFormData(prev => ({ ...prev, github_repo: '', name: '', slug: '' }));
                    }}
                    className="text-sm text-violet-600 hover:text-violet-700 font-medium"
                  >
                    {tCommon('change')}
                  </button>
                </div>
              ) : linkedAccounts && linkedAccounts.length > 0 ? (
                <div className="rounded-xl border border-gray-200 bg-white overflow-hidden">
                  <div className="p-3 border-b border-gray-100">
                    <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-gray-50">
                      <Search className="w-4 h-4 text-gray-400" />
                      <input
                        type="text"
                        placeholder={t('create.searchRepos')}
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        className="flex-1 bg-transparent text-sm focus:outline-none"
                      />
                    </div>
                  </div>
                  <div className="max-h-72 overflow-y-auto">
                    {reposLoading || accountsLoading ? (
                    <div className="p-8 flex items-center justify-center">
                      <div className="w-8 h-8 border-4 rounded-full border-gray-200 border-t-violet-600 animate-spin" />
                    </div>
                  ) : filteredRepos?.length === 0 ? (
                    <div className="p-8 text-center text-gray-500">
                      <Folder className="w-12 h-12 mx-auto mb-3 text-gray-300" />
                      {t('create.noRepos')}
                    </div>
                  ) : (
                    filteredRepos?.map((repo) => (
                      <button
                        key={repo.id}
                        type="button"
                        onClick={() => handleSelectRepo(repo)}
                        className="w-full flex items-center gap-3 p-3 hover:bg-violet-50 transition-colors text-left border-b border-gray-50 last:border-b-0"
                      >
                        <div className="w-10 h-10 rounded-lg bg-gray-100 flex items-center justify-center flex-shrink-0">
                          <SiGithub className="w-5 h-5 text-gray-600" />
                        </div>
                        <div className="flex-1 min-w-0">
                          <p className="font-medium text-gray-900 truncate">{repo.name}</p>
                          <p className="text-sm text-gray-500 truncate">
                            {repo.description || t('create.noDescription')}
                          </p>
                        </div>
                        <div className="flex items-center gap-2 flex-shrink-0">
                          {repo.private && (
                            <span className="px-2 py-0.5 text-xs rounded-full bg-gray-100 text-gray-600">Private</span>
                          )}
                          {repo.language && (
                            <span className="text-xs text-gray-400">{repo.language}</span>
                          )}
                        </div>
                      </button>
                    ))
                  )}
                  </div>
                </div>
              ) : null}
            </div>
          ) : (
            // Public URL Input
            <div className="space-y-3">
              <div className="flex items-center gap-3 px-4 py-3 rounded-xl border border-input bg-white focus-within:border-violet-400 focus-within:ring-2 focus-within:ring-violet-100 transition-all">
                <SiGithub className="w-5 h-5 text-gray-400 flex-shrink-0" />
                <input
                  type="text"
                  placeholder="owner/repo or https://github.com/owner/repo"
                  value={publicRepoUrl}
                  onChange={(e) => handlePublicRepoChange(e.target.value)}
                  className="flex-1 bg-transparent text-sm text-gray-900 placeholder:text-gray-400 focus:outline-none"
                />
              </div>
              {publicRepoError && (
                <p className="text-sm text-red-500 flex items-center gap-1.5">
                  <AlertCircle className="w-4 h-4 flex-shrink-0" />
                  {publicRepoError}
                </p>
              )}
              {formData.github_repo && !publicRepoError && (
                <div className="flex items-center gap-2 px-4 py-3 rounded-xl bg-green-50 border border-green-200 text-green-700">
                  <Check className="w-4 h-4 flex-shrink-0" />
                  <span className="text-sm font-medium">{formData.github_repo}</span>
                </div>
              )}
              <p className="text-xs text-gray-500 flex items-start gap-2">
                <Info className="w-4 h-4 flex-shrink-0 mt-0.5" />
                {t('create.publicUrlHelp')}
              </p>
            </div>
          )}
        </section>

        {/* Server Details */}
        <section>
          <h2 className="text-sm font-medium text-gray-500 uppercase tracking-wider mb-4">{t('create.configuration')}</h2>

          <div className="space-y-4">
            <div>
              <Label htmlFor="name" className="text-gray-700">{t('create.name')}</Label>
              <Input
                id="name"
                placeholder={t('create.namePlaceholder')}
                value={formData.name}
                onChange={(e) => {
                  const name = e.target.value;
                  setFormData(prev => ({ ...prev, name, slug: generateSlug(name) }));
                }}
                required
                className="mt-2"
              />
            </div>

            <div>
              <Label htmlFor="description" className="text-gray-700">{t('create.description')}</Label>
              <Input
                id="description"
                placeholder={t('create.descriptionBrief')}
                value={formData.description}
                onChange={(e) => setFormData(prev => ({ ...prev, description: e.target.value }))}
                className="mt-2"
              />
            </div>

            <div>
              <Label htmlFor="github_branch" className="text-gray-700">{t('create.branch')}</Label>
              <div className="mt-2 flex items-center gap-2 px-3 py-2 rounded-lg border border-gray-200 bg-white">
                <GitBranch className="w-4 h-4 text-gray-400" />
                <input
                  id="github_branch"
                  type="text"
                  placeholder={t('create.branchPlaceholder')}
                  value={formData.github_branch}
                  onChange={(e) => setFormData(prev => ({ ...prev, github_branch: e.target.value }))}
                  className="flex-1 bg-transparent text-sm focus:outline-none"
                />
              </div>
            </div>

            {/* Advanced Settings Toggle */}
            <button
              type="button"
              onClick={() => setShowAdvanced(!showAdvanced)}
              className="flex items-center gap-2 text-sm text-gray-500 hover:text-gray-700 transition-colors"
            >
              <ChevronRight className={`w-4 h-4 transition-transform ${showAdvanced ? 'rotate-90' : ''}`} />
              {t('create.advancedSettings')}
            </button>

            {/* Advanced Settings */}
            {showAdvanced && (
              <div className="pl-6 border-l-2 border-gray-100 space-y-4">
                <div>
                  <Label htmlFor="root_directory" className="text-gray-700">{t('create.rootDirectory')}</Label>
                  <p className="text-xs text-gray-500 mt-1 mb-2">{t('create.rootDirectoryHelp')}</p>
                  <div className="flex items-center gap-2 px-3 py-2 rounded-lg border border-gray-200 bg-white">
                    <Folder className="w-4 h-4 text-gray-400" />
                    <input
                      id="root_directory"
                      type="text"
                      placeholder="packages/mcp-server"
                      value={formData.root_directory || ''}
                      onChange={(e) => setFormData(prev => ({ ...prev, root_directory: e.target.value }))}
                      className="flex-1 bg-transparent text-sm focus:outline-none"
                    />
                  </div>
                </div>

                <div>
                  <Label htmlFor="mcp_path" className="text-gray-700">{t('create.mcpPath')}</Label>
                  <p className="text-xs text-gray-500 mt-1 mb-2">
                    {formData.transport === 'stdio' ? t('create.mcpPathAutoStdio') : t('create.mcpPathHelp')}
                  </p>
                  <div className={`flex items-center gap-2 px-3 py-2 rounded-lg border border-gray-200 ${
                    formData.transport === 'stdio' ? 'bg-gray-100 cursor-not-allowed' : 'bg-white'
                  }`}>
                    <Link className="w-4 h-4 text-gray-400" />
                    <input
                      id="mcp_path"
                      type="text"
                      placeholder="/mcp"
                      value={formData.transport === 'stdio' ? '/mcp' : (formData.mcp_path || '/mcp')}
                      onChange={(e) => setFormData(prev => ({ ...prev, mcp_path: e.target.value }))}
                      disabled={formData.transport === 'stdio'}
                      className={`flex-1 bg-transparent text-sm focus:outline-none ${
                        formData.transport === 'stdio' ? 'text-gray-500 cursor-not-allowed' : ''
                      }`}
                    />
                  </div>
                </div>

                <div>
                  <Label htmlFor="entry_command" className="text-gray-700">{t('create.entryCommand')}</Label>
                  <p className="text-xs text-gray-500 mt-1 mb-2">{t('create.entryCommandHelp')}</p>
                  <div className="flex items-center gap-2 px-3 py-2 rounded-lg border border-gray-200 bg-white">
                    <Terminal className="w-4 h-4 text-gray-400" />
                    <input
                      id="entry_command"
                      type="text"
                      placeholder="python server.py"
                      value={formData.entry_command || ''}
                      onChange={(e) => setFormData(prev => ({ ...prev, entry_command: e.target.value || undefined }))}
                      className="flex-1 bg-transparent text-sm focus:outline-none font-mono"
                    />
                  </div>
                </div>

                <div>
                  <Label htmlFor="build_command" className="text-gray-700">{t('create.buildCommand')}</Label>
                  <p className="text-xs text-gray-500 mt-1 mb-2">{t('create.buildCommandHelp')}</p>
                  <div className="flex items-center gap-2 px-3 py-2 rounded-lg border border-gray-200 bg-white">
                    <Terminal className="w-4 h-4 text-gray-400" />
                    <input
                      id="build_command"
                      type="text"
                      placeholder="npm run build"
                      value={formData.build_command || ''}
                      onChange={(e) => setFormData(prev => ({ ...prev, build_command: e.target.value || undefined }))}
                      className="flex-1 bg-transparent text-sm focus:outline-none font-mono"
                    />
                  </div>
                </div>

                {/* Machine memory */}
                <MemorySelect
                  value={formData.memory_mb ?? DEFAULT_MEMORY_MB}
                  onChange={(mb) => setFormData((prev) => ({ ...prev, memory_mb: mb }))}
                  maxMemoryMb={maxMemoryMb}
                />

                {/* Auth Enabled Toggle */}
                <div className="pt-4 border-t border-gray-100">
                  <div className="flex items-start gap-3">
                    <button
                      type="button"
                      onClick={() => setFormData(prev => ({ ...prev, auth_enabled: !prev.auth_enabled }))}
                      className={`mt-0.5 relative w-10 h-5 rounded-full transition-colors duration-200 flex-shrink-0 ${
                        formData.auth_enabled ? 'bg-violet-500' : 'bg-[#d1d5db]'
                      }`}
                    >
                      <span
                        className={`absolute top-0.5 w-4 h-4 bg-white rounded-full shadow-sm transition-transform duration-200 ${
                          formData.auth_enabled ? 'left-[22px]' : 'left-0.5'
                        }`}
                      />
                    </button>
                    <div className="flex-1">
                      <Label className="text-gray-700 cursor-pointer" onClick={() => setFormData(prev => ({ ...prev, auth_enabled: !prev.auth_enabled }))}>
                        {t('create.authEnabled')}
                      </Label>
                      <p className="text-xs text-gray-500 mt-1">{t('create.authEnabledHelp')}</p>
                      {!formData.auth_enabled && (
                        <div className="mt-2 px-3 py-2 rounded-lg bg-amber-50 border border-amber-200">
                          <p className="text-xs text-amber-700 flex items-start gap-2">
                            <AlertTriangle className="w-4 h-4 flex-shrink-0 mt-0.5" />
                            {t('create.authDisabledWarning')}
                          </p>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            )}
          </div>
        </section>

        {/* Transport Selection */}
        <section>
          <h2 className="text-sm font-medium text-gray-500 uppercase tracking-wider mb-4">{t('create.transport')}</h2>
          <p className="text-sm text-gray-500 mb-4">{t('create.transportHelp')}</p>

          <div className="grid grid-cols-2 gap-3">
            <button
              type="button"
              onClick={() => setFormData(prev => ({ ...prev, transport: 'sse' }))}
              className={`relative flex items-center gap-3 p-4 rounded-xl transition-all text-left ${
                formData.transport === 'sse'
                  ? 'bg-violet-50 border border-violet-200 shadow-sm'
                  : 'bg-white border border-gray-100 hover:border-gray-200 hover:bg-gray-50'
              }`}
            >
              <div className="flex items-center justify-center flex-shrink-0">
                <ArrowRight className={`w-6 h-6 ${formData.transport === 'sse' ? 'text-violet-600' : 'text-gray-400'}`} />
              </div>
              <div className="flex-1 min-w-0">
                <span className="block font-semibold text-sm text-[#323232]">
                  Streamable HTTP
                </span>
                <span className={`block text-xs mt-0.5 ${formData.transport === 'sse' ? 'text-violet-600' : 'text-gray-500'}`}>
                  {t('create.transportSseDesc')}
                </span>
              </div>
              {formData.transport === 'sse' && (
                <div className="absolute top-3 right-3 w-5 h-5 rounded-full bg-violet-500 flex items-center justify-center">
                  <Check className="w-3 h-3 text-white" strokeWidth={3} />
                </div>
              )}
            </button>
            <button
              type="button"
              onClick={() => setFormData(prev => ({ ...prev, transport: 'stdio' }))}
              className={`relative flex items-center gap-3 p-4 rounded-xl transition-all text-left ${
                formData.transport === 'stdio'
                  ? 'bg-violet-50 border border-violet-200 shadow-sm'
                  : 'bg-white border border-gray-100 hover:border-gray-200 hover:bg-gray-50'
              }`}
            >
              <div className="flex items-center justify-center flex-shrink-0">
                <MonitorPlay className={`w-6 h-6 ${formData.transport === 'stdio' ? 'text-violet-600' : 'text-gray-400'}`} />
              </div>
              <div className="flex-1 min-w-0">
                <span className="block font-semibold text-sm text-[#323232]">
                  STDIO
                </span>
                <span className={`block text-xs mt-0.5 ${formData.transport === 'stdio' ? 'text-violet-600' : 'text-gray-500'}`}>
                  {t('create.transportStdioDesc')}
                </span>
              </div>
              {formData.transport === 'stdio' && (
                <div className="absolute top-3 right-3 w-5 h-5 rounded-full bg-violet-500 flex items-center justify-center">
                  <Check className="w-3 h-3 text-white" strokeWidth={3} />
                </div>
              )}
            </button>
          </div>
        </section>

        {/* Runtime Selection */}
        <section>
          <h2 className="text-sm font-medium text-gray-500 uppercase tracking-wider mb-4">{t('create.runtime')}</h2>

          <div className="grid grid-cols-5 gap-3">
            {runtimes.map((runtime) => {
              const isSelected = formData.runtime === runtime.value;
              return (
                <button
                  key={runtime.value}
                  type="button"
                  onClick={() => setFormData(prev => ({ ...prev, runtime: runtime.value as Runtime }))}
                  className={`p-3 rounded-lg text-center transition-all duration-200 ${
                    isSelected
                      ? 'bg-white border border-gray-100 shadow-lg scale-110 z-10'
                      : 'bg-white border border-gray-50 opacity-40 hover:opacity-70'
                  }`}
                >
                  <div className={`${isSelected ? 'w-12 h-12' : 'w-10 h-10'} mx-auto mb-2 rounded-lg ${runtime.color} flex items-center justify-center text-white transition-all duration-200`}>
                    {runtime.icon}
                  </div>
                  <span className={`text-xs font-medium ${isSelected ? 'text-gray-900' : 'text-gray-600'}`}>{runtime.label}</span>
                </button>
              );
            })}
          </div>
        </section>

        {/* Visibility Selection */}
        <section>
          <h2 className="text-sm font-medium text-gray-500 uppercase tracking-wider mb-4">{t('create.visibility')}</h2>

          <div className="space-y-2">
            {visibilities.map((vis) => {
              const isSelected = formData.visibility === vis.value;
              return (
                <button
                  key={vis.value}
                  type="button"
                  onClick={() => setFormData(prev => ({ ...prev, visibility: vis.value as Visibility }))}
                  className={`w-full flex items-center gap-3 px-4 py-3 rounded-lg transition-all text-left ${
                    isSelected
                      ? ''
                      : 'hover:bg-gray-50'
                  }`}
                >
                  <div className={`w-5 h-5 rounded-full border-2 flex items-center justify-center flex-shrink-0 transition-all ${
                    isSelected
                      ? 'border-violet-500 bg-violet-500'
                      : 'border-gray-500'
                  }`}>
                    {isSelected && (
                      <div className="w-2 h-2 rounded-full bg-white" />
                    )}
                  </div>
                  <span className={`${isSelected ? 'text-violet-600' : 'text-gray-400'}`}>
                    {vis.icon}
                  </span>
                  <div className="flex-1">
                    <span className={`text-sm font-medium ${isSelected ? 'text-gray-900' : 'text-gray-700'}`}>
                      {vis.label}
                    </span>
                    <span className={`text-sm ml-2 ${isSelected ? 'text-gray-600' : 'text-gray-400'}`}>
                      - {vis.desc}
                    </span>
                  </div>
                </button>
              );
            })}
          </div>
        </section>

        {/* Error Message */}
        {createMutation.isError && (
          <div className="p-4 rounded-xl bg-red-50 border border-red-200">
            <div className="flex items-start gap-3">
              <div className="w-8 h-8 rounded-full bg-red-100 flex items-center justify-center flex-shrink-0">
                <XCircle className="w-4 h-4 text-red-600" />
              </div>
              <div>
                <p className="font-medium text-red-800">{errorMessage}</p>
                {errorSuggestion && (
                  <p className="text-sm text-red-600 mt-1">
                    {t('create.trySuggestion')} <code className="px-1.5 py-0.5 bg-red-100 rounded text-xs">{errorSuggestion}</code>
                  </p>
                )}
              </div>
            </div>
          </div>
        )}

        {/* Actions */}
        <div className="flex justify-end gap-2.5 pt-4 border-t border-gray-100">
          <Button
            type="button"
            variant="outline"
            onClick={() => router.back()}
            className="h-10 px-4 rounded-lg border-[#d1d5db] text-[#374151] text-sm font-medium hover:bg-[#f3f4f6] transition-colors duration-200"
          >
            {tCommon('cancel')}
          </Button>
          <Button
            type="submit"
            disabled={createMutation.isPending || !workspaceId || !formData.github_repo}
            className="h-10 px-4 rounded-lg bg-violet-500 hover:bg-violet-600 border border-violet-600 text-white text-sm font-medium gap-2 transition-colors duration-200"
          >
            {createMutation.isPending ? (
              <div className="w-4 h-4 border-2 rounded-full border-white/30 border-t-white animate-spin" />
            ) : (
              <>
                <Plus className="w-4 h-4" />
                {t('create.submit')}
              </>
            )}
          </Button>
        </div>
      </form>
    </div>
  );
}
