'use client';

import { Header, Footer } from '@/components/layout';

export default function PrivacyPolicyPage() {
  return (
    <div className="min-h-screen flex flex-col bg-white">
      <Header />
      <main className="flex-1 py-16">
        <div className="max-w-3xl mx-auto px-4">
          <h1 className="text-3xl font-bold text-gray-900 mb-8">プライバシーポリシー</h1>
          <p className="text-sm text-gray-500 mb-8">最終更新日: 2024年1月1日</p>

          <div className="prose prose-gray max-w-none">
            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">1. はじめに</h2>
              <p className="text-gray-600">
                Nodeflare（以下「当社」といいます）は、お客様のプライバシーを尊重し、個人情報の保護に努めております。
                本プライバシーポリシーは、当社が提供するMCPサーバーホスティングサービス（以下「本サービス」といいます）における
                個人情報の取り扱いについて説明するものです。
              </p>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">2. 収集する情報</h2>
              <p className="text-gray-600 mb-4">当社は、以下の情報を収集することがあります。</p>

              <h3 className="text-lg font-medium text-gray-800 mb-2">2.1 アカウント情報</h3>
              <ul className="list-disc list-inside space-y-1 text-gray-600 mb-4">
                <li>氏名</li>
                <li>メールアドレス</li>
                <li>GitHubアカウント情報（ユーザー名、プロフィール画像）</li>
              </ul>

              <h3 className="text-lg font-medium text-gray-800 mb-2">2.2 支払い情報</h3>
              <ul className="list-disc list-inside space-y-1 text-gray-600 mb-4">
                <li>クレジットカード情報（Stripeを通じて処理され、当社では保存しません）</li>
                <li>請求先住所</li>
                <li>取引履歴</li>
              </ul>

              <h3 className="text-lg font-medium text-gray-800 mb-2">2.3 利用データ</h3>
              <ul className="list-disc list-inside space-y-1 text-gray-600 mb-4">
                <li>IPアドレス</li>
                <li>ブラウザの種類とバージョン</li>
                <li>アクセス日時</li>
                <li>サービスの利用状況（デプロイ回数、APIリクエスト数等）</li>
                <li>エラーログ</li>
              </ul>

              <h3 className="text-lg font-medium text-gray-800 mb-2">2.4 ユーザーコンテンツ</h3>
              <ul className="list-disc list-inside space-y-1 text-gray-600">
                <li>アップロードされたコード・設定ファイル</li>
                <li>環境変数・シークレット（暗号化して保存）</li>
                <li>サーバーログ</li>
              </ul>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">3. 情報の利用目的</h2>
              <p className="text-gray-600 mb-2">収集した情報は、以下の目的で利用します。</p>
              <ul className="list-disc list-inside space-y-1 text-gray-600">
                <li>本サービスの提供・運営</li>
                <li>ユーザーサポートの提供</li>
                <li>サービスの改善・新機能の開発</li>
                <li>利用状況の分析</li>
                <li>不正利用の検出・防止</li>
                <li>請求処理</li>
                <li>重要なお知らせの送信</li>
                <li>法令に基づく対応</li>
              </ul>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">4. 情報の共有</h2>
              <p className="text-gray-600 mb-4">当社は、以下の場合を除き、お客様の個人情報を第三者と共有しません。</p>
              <ul className="list-disc list-inside space-y-2 text-gray-600">
                <li>
                  <strong>サービスプロバイダー:</strong> 本サービスの提供に必要な外部サービス（Stripe、Fly.io、GitHub等）との間で、
                  必要最小限の情報を共有します。
                </li>
                <li>
                  <strong>法的要請:</strong> 法令に基づく開示要請があった場合、または当社の権利を保護するために必要な場合。
                </li>
                <li>
                  <strong>同意がある場合:</strong> お客様から明示的な同意を得た場合。
                </li>
                <li>
                  <strong>事業譲渡:</strong> 合併、買収、または事業譲渡の際に、承継先に情報を移転することがあります。
                </li>
              </ul>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">5. データの保存と保護</h2>
              <ul className="list-disc list-inside space-y-2 text-gray-600">
                <li>お客様のデータは、業界標準の暗号化技術を用いて保護されます。</li>
                <li>シークレット・環境変数は、AES-256で暗号化して保存されます。</li>
                <li>データは、サービスの提供に必要な期間、または法令で定められた期間保存されます。</li>
                <li>アカウントを削除した場合、関連するデータは30日以内に削除されます。</li>
              </ul>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">6. Cookieの使用</h2>
              <p className="text-gray-600 mb-4">当社は、以下の目的でCookieを使用します。</p>
              <ul className="list-disc list-inside space-y-1 text-gray-600">
                <li>ログイン状態の維持</li>
                <li>セッション管理</li>
                <li>サービスの利用状況の分析</li>
              </ul>
              <p className="text-gray-600 mt-4">
                ブラウザの設定でCookieを無効にすることができますが、一部の機能が利用できなくなる場合があります。
              </p>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">7. お客様の権利</h2>
              <p className="text-gray-600 mb-4">お客様は、ご自身の個人情報について以下の権利を有します。</p>
              <ul className="list-disc list-inside space-y-1 text-gray-600">
                <li><strong>アクセス権:</strong> 当社が保有するお客様の個人情報へのアクセス</li>
                <li><strong>訂正権:</strong> 不正確な情報の訂正</li>
                <li><strong>削除権:</strong> 個人情報の削除（アカウント設定から実行可能）</li>
                <li><strong>データポータビリティ:</strong> データのエクスポート</li>
              </ul>
              <p className="text-gray-600 mt-4">
                これらの権利を行使する場合は、<a href="/contact" className="text-violet-600 hover:underline">お問い合わせフォーム</a>よりご連絡ください。
              </p>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">8. 子供のプライバシー</h2>
              <p className="text-gray-600">
                本サービスは、13歳未満のお子様を対象としていません。
                13歳未満のお子様から個人情報を収集していることが判明した場合は、速やかに削除いたします。
              </p>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">9. 国際データ転送</h2>
              <p className="text-gray-600">
                お客様のデータは、日本国外のサーバーに保存・処理される場合があります。
                当社は、適切な保護措置を講じた上でデータを転送します。
              </p>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">10. プライバシーポリシーの変更</h2>
              <p className="text-gray-600">
                当社は、必要に応じて本プライバシーポリシーを変更することがあります。
                重要な変更がある場合は、メールまたは本サービス上で通知いたします。
              </p>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">11. お問い合わせ</h2>
              <p className="text-gray-600">
                本プライバシーポリシーに関するご質問やご要望は、
                <a href="/contact" className="text-violet-600 hover:underline">お問い合わせフォーム</a>よりご連絡ください。
              </p>
            </section>
          </div>
        </div>
      </main>
      <Footer />
    </div>
  );
}
