import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'FAQ',
  alternates: {
    canonical: '/faq',
    languages: {
      'ja': '/faq',
      'en': '/faq',
    },
  },
};

export default function FAQLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
