import type { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Pricing',
  alternates: {
    canonical: '/pricing',
    languages: {
      'ja': '/pricing',
      'en': '/pricing',
    },
  },
};

export default function PricingLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return children;
}
