'use client';

import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { usePathname } from 'next/navigation';
import { useTranslations } from 'next-intl';
import { LayoutDashboard, Server, Lock, Shield, Users, FileText, CreditCard, Settings } from 'lucide-react';

type HeaderContent = { title: ReactNode; icon: ReactNode } | null;

const PageHeaderContext = createContext<{
  header: HeaderContent;
  setHeader: (h: HeaderContent) => void;
}>({ header: null, setHeader: () => {} });

export function PageHeaderProvider({ children }: { children: ReactNode }) {
  const [header, setHeader] = useState<HeaderContent>(null);
  return (
    <PageHeaderContext.Provider value={{ header, setHeader }}>
      {children}
    </PageHeaderContext.Provider>
  );
}

// 各ページが自分のタイトル/アイコンをヘッダーに登録する。title が変わった時のみ更新。
export function useSetPageHeader(title: ReactNode, icon: ReactNode) {
  const { setHeader } = useContext(PageHeaderContext);
  const iconRef = useRef(icon);
  iconRef.current = icon;
  // title をキーにする（文字列/数値想定）。ReactNodeの場合はString化して比較。
  const titleKey = typeof title === 'string' || typeof title === 'number' ? String(title) : '';
  useEffect(() => {
    setHeader({ title, icon: iconRef.current });
    return () => setHeader(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [setHeader, titleKey]);
}

// トップレベルページ（h1を削除済み）のフォールバック定義。pathnameから判定。
const HEADER_PAGES: { href: string; exact?: boolean; icon: ReactNode; titleKey: string }[] = [
  { href: '/dashboard', exact: true, icon: <LayoutDashboard className="w-4 h-4" />, titleKey: 'dashboard.title' },
  { href: '/dashboard/servers', icon: <Server className="w-4 h-4" />, titleKey: 'servers.title' },
  { href: '/dashboard/auth', icon: <Lock className="w-4 h-4" />, titleKey: 'auth.settings.title' },
  { href: '/dashboard/vpn', icon: <Shield className="w-4 h-4" />, titleKey: 'vpn.title' },
  { href: '/dashboard/team', icon: <Users className="w-4 h-4" />, titleKey: 'team.title' },
  { href: '/dashboard/logs', icon: <FileText className="w-4 h-4" />, titleKey: 'logs.title' },
  { href: '/dashboard/billing', icon: <CreditCard className="w-4 h-4" />, titleKey: 'billing.title' },
  { href: '/dashboard/settings', icon: <Settings className="w-4 h-4" />, titleKey: 'settings.title' },
];

// ヘッダーに表示するタイトル。ページが登録した値を優先し、無ければpathnameでフォールバック。
export function DashboardHeaderTitle() {
  const { header } = useContext(PageHeaderContext);
  const pathname = usePathname();
  const tRoot = useTranslations();

  let icon: ReactNode;
  let title: ReactNode;

  if (header) {
    icon = header.icon;
    title = header.title;
  } else {
    const page = HEADER_PAGES.find((p) =>
      p.exact ? pathname === p.href : pathname === p.href || pathname.startsWith(p.href + '/')
    );
    if (!page) return null;
    icon = page.icon;
    title = tRoot(page.titleKey);
  }

  return (
    <h1 className="text-sm sm:text-base font-medium flex items-center gap-2 text-[#323232] min-w-0">
      <span className="flex-shrink-0 [&>svg]:w-4 [&>svg]:h-4">{icon}</span>
      <span className="truncate">{title}</span>
    </h1>
  );
}
