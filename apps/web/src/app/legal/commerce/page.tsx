'use client';

import { useTranslations } from 'next-intl';
import { Header, Footer } from '@/components/layout';

export default function CommercePage() {
  const t = useTranslations('commercePage');

  return (
    <div className="min-h-screen flex flex-col bg-white">
      <Header />
      <main className="flex-1 py-16">
        <div className="max-w-3xl mx-auto px-4">
          <h1 className="text-3xl font-bold text-gray-900 mb-8">{t('title')}</h1>

          <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
            <table className="w-full">
              <tbody className="divide-y divide-gray-200">
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50 w-1/3">
                    {t('seller')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    {t('sellerValue')}
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('operator')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    {t('operatorValue')}
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('address')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    {t('addressValue')}
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('phone')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    {t('phoneValue')}
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('contact')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    <a href="/contact" className="text-violet-600 hover:underline">{t('contactLink')}</a>{t('contactValue')}
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('price')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    <a href="/pricing" className="text-violet-600 hover:underline">{t('priceLink')}</a>{t('priceValue')}
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('payment')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    {t('paymentValue')}
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('paymentTiming')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    {t('paymentTimingValue')}
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('serviceDelivery')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    {t('serviceDeliveryValue')}
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('cancellation')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    <p className="mb-2">{t('cancellationValue1')}</p>
                    <p className="mb-2">{t('cancellationValue2')}</p>
                    <p>{t('cancellationValue3')}</p>
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('environment')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    <ul className="list-disc list-inside space-y-1">
                      <li>{t('environmentInternet')}</li>
                      <li>{t('environmentBrowser')}</li>
                    </ul>
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('other')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    <p className="mb-2">
                      {t('otherValue1')}<a href="/legal/terms" className="text-violet-600 hover:underline">{t('otherTermsLink')}</a>{t('otherValue1End')}
                    </p>
                    <p>{t('otherValue2')}</p>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </main>
      <Footer />
    </div>
  );
}
