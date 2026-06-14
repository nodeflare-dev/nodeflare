'use client';

import { useTranslations } from 'next-intl';
import { Header, Footer } from '@/components/layout';

export default function CompanyPage() {
  const t = useTranslations('companyPage');

  return (
    <div className="min-h-screen flex flex-col bg-white">
      <Header />
      <main className="flex-1 py-16">
        <div className="max-w-3xl mx-auto px-4">
          <h1 className="text-3xl font-bold text-gray-900 mb-3">{t('title')}</h1>
          <p className="text-sm text-gray-600 mb-8">{t('intro')}</p>

          <div className="bg-white rounded-xl border border-gray-200 overflow-hidden">
            <table className="w-full">
              <tbody className="divide-y divide-gray-200">
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50 w-1/3">
                    {t('name')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">{t('nameValue')}</td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('nameEn')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">{t('nameEnValue')}</td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('address')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">{t('addressValue')}</td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('representative')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">{t('representativeValue')}</td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('capital')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">{t('capitalValue')}</td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('established')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-400">{t('tbd')}</td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('business')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">{t('businessValue')}</td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('employees')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">{t('employeesValue')}</td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('corporateNumber')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-400">{t('tbd')}</td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('contact')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">
                    <p className="mb-1">{t('contactPhone')}</p>
                    <p>
                      <a href="/contact" className="text-violet-600 hover:underline">{t('contactLink')}</a>
                      {t('contactValue')}
                    </p>
                  </td>
                </tr>
                <tr>
                  <th className="px-6 py-4 text-left text-sm font-medium text-gray-900 bg-gray-50">
                    {t('publicNotice')}
                  </th>
                  <td className="px-6 py-4 text-sm text-gray-600">{t('publicNoticeValue')}</td>
                </tr>
              </tbody>
            </table>
          </div>

          {/* 電子公告 / Electronic public notices */}
          <section className="mt-12">
            <h2 className="text-xl font-bold text-gray-900 mb-2">{t('noticeHeading')}</h2>
            <p className="text-sm text-gray-600 mb-4">{t('noticeDesc')}</p>
            <div className="bg-gray-50 rounded-xl border border-gray-200 px-6 py-10 text-center text-sm text-gray-500">
              {t('noticeEmpty')}
            </div>
          </section>
        </div>
      </main>
      <Footer />
    </div>
  );
}
