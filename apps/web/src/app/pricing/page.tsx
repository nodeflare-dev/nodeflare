'use client';

import { useState } from 'react';
import { useTranslations } from 'next-intl';
import { Check } from 'lucide-react';
import { Header, Footer } from '@/components/layout';
import { Button } from '@/components/ui/button';
import { PLANS, formatPrice } from '@/lib/plans';

function useComparisonFeatures() {
  const t = useTranslations('pricing');
  return [
    { name: t('features.mcpServers'), key: 'max_servers' as const },
    { name: t('features.deploysMonth'), key: 'max_deployments_per_month' as const },
    { name: t('features.requestsMonth'), key: 'max_requests_per_month' as const },
    { name: t('features.teamMembers'), key: 'max_team_members' as const },
    { name: t('features.logRetention'), key: 'log_retention_days' as const },
    { name: t('features.customDomains'), key: 'custom_domains' as const },
    { name: t('features.prioritySupport'), key: 'priority_support' as const },
    { name: t('features.ssoSaml'), key: 'sso_enabled' as const },
  ];
}

function useFormatLimitValue() {
  const t = useTranslations('pricing');
  return (key: string, value: number | boolean): string | boolean => {
    if (typeof value === 'boolean') return value;
    if (value === Infinity || value > 1_000_000_000) return t('unlimited');
    if (key === 'log_retention_days') return t('days', { days: value });
    if (key === 'max_requests_per_month') {
      if (value >= 1_000_000) return `${(value / 1_000_000).toLocaleString()}M`;
      if (value >= 1_000) return `${(value / 1_000).toLocaleString()}K`;
      return value.toLocaleString();
    }
    return value.toLocaleString();
  };
}

export default function PricingPage() {
  const t = useTranslations('pricing');
  const [isYearly, setIsYearly] = useState(false);
  const comparisonFeatures = useComparisonFeatures();
  const formatLimitValue = useFormatLimitValue();

  return (
    <div className="min-h-screen bg-white">
      <Header />

      <main>
        {/* Hero */}
        <section className="py-20">
          <div className="max-w-4xl mx-auto px-4 sm:px-6 text-center">
            <h1 className="text-xl sm:text-2xl font-extrabold mb-8" style={{ color: '#333333' }}>
              {t('title')}
            </h1>

            {/* Billing Toggle */}
            <div className="inline-flex items-center bg-gray-100 rounded-lg p-1">
              <button
                className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                  !isYearly ? 'bg-white shadow text-gray-900' : 'text-gray-600'
                }`}
                onClick={() => setIsYearly(false)}
              >
                {t('monthly')}
              </button>
              <button
                className={`px-4 py-2 rounded-md text-sm font-medium transition-colors ${
                  isYearly ? 'bg-white shadow text-gray-900' : 'text-gray-600'
                }`}
                onClick={() => setIsYearly(true)}
              >
                {t('yearly')} <span className="text-green-600 text-xs ml-1">{t('yearlySave')}</span>
              </button>
            </div>
          </div>
        </section>

        {/* Plans */}
        <section className="pb-20">
          <div className="max-w-6xl mx-auto px-4 sm:px-6">
            <div className="grid md:grid-cols-2 lg:grid-cols-4 gap-6">
              {PLANS.map((plan) => {
                const isPopular = plan.plan === 'pro';
                const isEnterprise = plan.plan === 'enterprise';
                const price = isYearly ? plan.price_yearly_jpy : plan.price_monthly_jpy;
                const monthlyEquivalent = price !== null ? (isYearly ? Math.round(price / 12) : price) : null;

                return (
                  <div key={plan.plan} className="relative group">
                    {isPopular && (
                      <div className="absolute -inset-[1px] bg-violet-500 rounded-2xl" />
                    )}
                    <div
                      className={`relative rounded-2xl p-8 h-full flex flex-col ${
                        isPopular
                          ? 'bg-gray-900 text-white'
                          : 'bg-white border border-gray-200 hover:border-gray-300 hover:shadow-lg transition-all'
                      }`}
                    >
                      <div className="flex items-center gap-2 mb-2">
                        <span className={`text-sm font-medium ${isPopular ? 'text-violet-300' : 'text-gray-500'}`}>
                          {plan.name}
                        </span>
                        {isPopular && (
                          <span className="px-2 py-0.5 rounded-full bg-violet-500/20 text-violet-300 text-xs font-medium">
                            {t('recommended')}
                          </span>
                        )}
                      </div>

                      <div className="flex items-baseline gap-1 mb-2">
                        {isEnterprise ? (
                          <span className="text-2xl font-bold">{t('contactUs')}</span>
                        ) : (
                          <>
                            <span className="text-4xl font-bold">{formatPrice(monthlyEquivalent)}</span>
                            <span className={isPopular ? 'text-gray-400' : 'text-gray-500'}>{t('perMonth')}</span>
                          </>
                        )}
                      </div>

                      {isYearly && price !== null && price > 0 && (
                        <p className={`text-sm mb-4 ${isPopular ? 'text-gray-400' : 'text-gray-500'}`}>
                          {t('yearlyPrice', { price: formatPrice(price) })}
                        </p>
                      )}
                      {isEnterprise && (
                        <p className={`text-sm mb-4 text-gray-500`}>
                          {t('enterpriseCustomPricing')}
                        </p>
                      )}

                      <p className={`mb-6 ${isPopular ? 'text-gray-400' : 'text-gray-600'}`}>
                        {plan.description}
                      </p>

                      <ul className="space-y-3 mb-8">
                        {plan.features.map((feature) => (
                          <li key={feature} className="flex items-center gap-3 text-sm">
                            <Check className={`w-5 h-5 flex-shrink-0 ${isPopular ? 'text-violet-400' : 'text-gray-400'}`} strokeWidth={2.5} />
                            <span className={isPopular ? 'text-gray-200' : 'text-gray-700'}>
                              {feature}
                            </span>
                          </li>
                        ))}
                      </ul>

                      <a href={isEnterprise ? '/contact' : '/api/v1/auth/github'} className="block mt-auto">
                        <Button
                          variant={isPopular ? 'default' : 'outline'}
                          className={`w-full h-10 ${
                            isPopular
                              ? 'bg-violet-600 hover:bg-violet-700 text-white'
                              : 'border-gray-300 text-gray-700 hover:bg-gray-50'
                          }`}
                        >
                          {plan.plan === 'free' ? t('startFree') : isEnterprise ? t('contactUs') : t('startPlan', { plan: plan.name })}
                        </Button>
                      </a>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </section>

        {/* Comparison Table */}
        <section className="py-20 border-t border-gray-200">
          <div className="max-w-5xl mx-auto px-4 sm:px-6">
            <h2 className="text-2xl font-bold text-gray-900 text-center mb-12">
              {t('comparison')}
            </h2>

            <div className="overflow-x-auto">
              <div className="overflow-hidden rounded-xl border-2 border-gray-300 shadow-sm min-w-[640px]">
                <table className="w-full">
                  <thead>
                    <tr className="bg-gray-100 border-b-2 border-gray-300">
                      <th className="text-left py-4 px-6 font-semibold text-gray-900">{t('feature')}</th>
                      {PLANS.map((plan) => (
                        <th
                          key={plan.plan}
                          className={`text-center py-4 px-4 font-semibold text-gray-900 border-l border-gray-300 ${
                            plan.plan === 'pro' ? 'bg-violet-100' : ''
                          }`}
                        >
                          {plan.name}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {comparisonFeatures.map((feature, idx) => (
                      <tr key={feature.key} className={`${idx % 2 === 0 ? 'bg-white' : 'bg-gray-50'} border-t border-gray-200`}>
                        <td className="py-4 px-6 text-gray-700 font-medium">{feature.name}</td>
                        {PLANS.map((plan) => {
                          const value = plan.limits[feature.key];
                          const displayValue = formatLimitValue(feature.key, value);

                          return (
                            <td
                              key={plan.plan}
                              className={`py-4 px-4 text-center border-l border-gray-200 ${
                                plan.plan === 'pro' ? (idx % 2 === 0 ? 'bg-violet-50' : 'bg-violet-100') : ''
                              }`}
                            >
                              {typeof displayValue === 'boolean' ? (
                                displayValue ? (
                                  <Check className="w-5 h-5 text-green-600 mx-auto" strokeWidth={2.5} />
                                ) : (
                                  <span className="text-gray-400">—</span>
                                )
                              ) : (
                                <span className="text-gray-900 font-medium">{displayValue}</span>
                              )}
                            </td>
                          );
                        })}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </section>

      </main>

      <Footer />
    </div>
  );
}

