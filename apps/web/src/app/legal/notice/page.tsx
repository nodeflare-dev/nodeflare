'use client';

import { Header, Footer } from '@/components/layout';

export default function NoticePage() {
  return (
    <div className="min-h-screen flex flex-col bg-white">
      <Header />
      <main className="flex-1 py-16">
        <div className="max-w-3xl mx-auto px-4">
          <h1 className="text-3xl font-bold text-gray-900 mb-8">公告</h1>
          <p className="text-sm text-gray-500 mb-8">掲載日: 2026年5月29日</p>

          <div className="prose prose-gray max-w-none">
            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">会社設立公告</h2>
              <p className="text-gray-600 mb-4">
                この度、下記の通り会社を設立いたしましたので、ここに公告いたします。
              </p>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">会社概要</h2>
              <table className="w-full border-collapse">
                <tbody>
                  <tr className="border-b border-gray-200">
                    <th className="py-3 pr-4 text-left text-gray-700 font-medium w-1/3">商号</th>
                    <td className="py-3 text-gray-600">NodeFlare株式会社</td>
                  </tr>
                  <tr className="border-b border-gray-200">
                    <th className="py-3 pr-4 text-left text-gray-700 font-medium">代表者</th>
                    <td className="py-3 text-gray-600">関口峻矢</td>
                  </tr>
                  <tr className="border-b border-gray-200">
                    <th className="py-3 pr-4 text-left text-gray-700 font-medium">本店所在地</th>
                    <td className="py-3 text-gray-600">群馬県高崎市上豊岡町180-7<br />シティパレス上豊岡210</td>
                  </tr>
                  <tr className="border-b border-gray-200">
                    <th className="py-3 pr-4 text-left text-gray-700 font-medium">設立年月日</th>
                    <td className="py-3 text-gray-600">2026年5月29日</td>
                  </tr>
                  <tr className="border-b border-gray-200">
                    <th className="py-3 pr-4 text-left text-gray-700 font-medium">資本金</th>
                    <td className="py-3 text-gray-600">500万円</td>
                  </tr>
                  <tr className="border-b border-gray-200">
                    <th className="py-3 pr-4 text-left text-gray-700 font-medium">事業内容</th>
                    <td className="py-3 text-gray-600">
                      MCPサーバーホスティングサービスの提供<br />
                      ソフトウェアの開発および販売<br />
                      その他上記に付帯する一切の事業
                    </td>
                  </tr>
                </tbody>
              </table>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">公告方法</h2>
              <p className="text-gray-600">
                当会社の公告は、電子公告により行います。<br />
                公告掲載URL: https://nodeflare.dev/legal/notice
              </p>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">お問い合わせ</h2>
              <p className="text-gray-600">
                本公告に関するお問い合わせは、<a href="/contact" className="text-violet-600 hover:underline">お問い合わせフォーム</a>よりご連絡ください。
              </p>
            </section>
          </div>
        </div>
      </main>
      <Footer />
    </div>
  );
}
