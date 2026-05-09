import { api } from './api';
import { GitHubRepo } from '@/types';

export interface LinkedGitHubAccount {
  id: string;
  github_id: number;
  github_username: string;
  github_avatar_url: string | null;
  is_primary: boolean;
  created_at: string;
}

export interface MessageResponse {
  message: string;
}

/**
 * Get all linked GitHub accounts for the current user
 */
export const getLinkedAccounts = (): Promise<LinkedGitHubAccount[]> => {
  return api.get('/github/accounts');
};

/**
 * Unlink a GitHub account
 */
export const unlinkAccount = (accountId: string): Promise<MessageResponse> => {
  return api.delete(`/github/accounts/${accountId}`);
};

/**
 * Set a linked account as primary
 */
export const setPrimaryAccount = (accountId: string): Promise<MessageResponse> => {
  return api.post(`/github/accounts/${accountId}/primary`);
};

/**
 * Get repositories from a linked GitHub account
 * @param accountId - Optional account ID. If not provided, uses primary or any linked account
 */
export const getRepos = (accountId?: string): Promise<GitHubRepo[]> => {
  const path = accountId ? `/github/repos?account_id=${accountId}` : '/github/repos';
  return api.get(path);
};

/**
 * Get the link URL to initiate GitHub OAuth for account linking
 * @param returnTo - Optional path to redirect to after linking
 */
export const getLinkUrl = (returnTo?: string): string => {
  const apiBase = process.env.NEXT_PUBLIC_API_URL || '';
  const params = returnTo ? `?return_to=${encodeURIComponent(returnTo)}` : '';
  return `${apiBase}/api/v1/github/accounts/link${params}`;
};
