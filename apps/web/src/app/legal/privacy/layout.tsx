import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Privacy Policy',
  alternates: {
    canonical: '/legal/privacy',
    languages: {
      'ja': '/legal/privacy',
      'en': '/legal/privacy',
    },
  },
};

export default function PrivacyLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
