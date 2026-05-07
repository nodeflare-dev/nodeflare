import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Commercial Transactions Act',
  alternates: {
    canonical: '/legal/commerce',
    languages: {
      'ja': '/legal/commerce',
      'en': '/legal/commerce',
    },
  },
};

export default function CommerceLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
