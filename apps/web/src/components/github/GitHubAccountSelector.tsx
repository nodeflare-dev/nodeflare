'use client';

import { useState, useRef, useEffect } from 'react';
import { useTranslations } from 'next-intl';
import { Check, Plus, ChevronDown } from 'lucide-react';
import { SiGithub } from 'react-icons/si';
import { LinkedGitHubAccount, getLinkUrl } from '@/lib/github-api';

interface GitHubAccountSelectorProps {
  accounts: LinkedGitHubAccount[];
  selectedAccountId: string | null;
  onSelect: (accountId: string | null) => void;
  returnTo?: string;
  isLoading?: boolean;
  className?: string;
}

export function GitHubAccountSelector({
  accounts,
  selectedAccountId,
  onSelect,
  returnTo,
  isLoading,
  className = '',
}: GitHubAccountSelectorProps) {
  const t = useTranslations('github');
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const selectedAccount = accounts.find((a) => a.id === selectedAccountId);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleAddAccount = () => {
    window.location.href = getLinkUrl(returnTo);
  };

  if (isLoading) {
    return (
      <div className="inline-flex items-center gap-2 h-9 px-3 border border-gray-200 rounded-md bg-white text-sm text-gray-400">
        <div className="w-4 h-4 rounded-full border-2 border-gray-200 border-t-gray-500 animate-spin" />
        Loading...
      </div>
    );
  }

  return (
    <div ref={dropdownRef} className={`relative inline-block ${className}`}>
      {/* Select Box */}
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="inline-flex items-center gap-2 h-10 pl-3 pr-10 min-w-[200px] border border-gray-300 rounded-xl bg-white hover:border-gray-400 text-sm cursor-pointer"
      >
        <SiGithub className="w-4 h-4 text-gray-700" />
        <span className={accounts.length === 0 ? 'text-gray-400' : 'text-gray-900'}>
          {accounts.length === 0
            ? t('noAccounts')
            : selectedAccount?.github_username || t('selectAccount')
          }
        </span>
      </button>
      <ChevronDown className={`pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 transition-transform ${isOpen ? 'rotate-180' : ''}`} />

      {/* Dropdown */}
      {isOpen && (
        <div className="absolute top-full left-0 mt-1 min-w-full w-max bg-white border border-gray-300 rounded-xl shadow-lg z-50 py-1">
          {accounts.map((account) => (
            <button
              key={account.id}
              type="button"
              onClick={() => {
                onSelect(account.id);
                setIsOpen(false);
              }}
              className="w-full flex items-center gap-2 px-3 py-2 hover:bg-gray-100 text-left text-sm"
            >
              <SiGithub className="w-4 h-4 text-gray-700" />
              <span className="flex-1 text-gray-900">{account.github_username}</span>
              {account.id === selectedAccountId && (
                <Check className="w-4 h-4 text-gray-700" />
              )}
            </button>
          ))}

          <div className="border-t border-gray-100 mt-1 pt-1">
            <button
              type="button"
              onClick={handleAddAccount}
              className="w-full flex items-center gap-2 px-3 py-2 hover:bg-gray-100 text-left text-sm text-gray-600"
            >
              <Plus className="w-4 h-4" />
              <span>{t('addAccount')}</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
