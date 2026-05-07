import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Terms of Service',
  alternates: {
    canonical: '/legal/terms',
    languages: {
      'ja': '/legal/terms',
      'en': '/legal/terms',
    },
  },
};

export default function TermsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
