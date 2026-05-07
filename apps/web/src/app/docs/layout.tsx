import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Documentation',
  alternates: {
    canonical: '/docs',
    languages: {
      'ja': '/docs',
      'en': '/docs',
    },
  },
};

export default function DocsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
