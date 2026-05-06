'use client';

import { Header, Footer } from '@/components/layout';

export default function TermsPage() {
  return (
    <div className="min-h-screen flex flex-col bg-white">
      <Header />
      <main className="flex-1 py-16">
        <div className="max-w-3xl mx-auto px-4">
          <h1 className="text-3xl font-bold text-gray-900 mb-8">利用規約</h1>
          <p className="text-sm text-gray-500 mb-8">最終更新日: 2024年1月1日</p>

          <div className="prose prose-gray max-w-none">
            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第1条（適用）</h2>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>本規約は、Nodeflare（以下「当社」といいます）が提供するMCPサーバーホスティングサービス（以下「本サービス」といいます）の利用条件を定めるものです。</li>
                <li>ユーザーは、本規約に同意の上、本サービスを利用するものとします。</li>
                <li>本規約は、本サービスの利用に関する当社とユーザーとの間の一切の関係に適用されます。</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第2条（定義）</h2>
              <p className="text-gray-600 mb-2">本規約において使用する用語の定義は、以下のとおりとします。</p>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>「ユーザー」とは、本サービスを利用する全ての方をいいます。</li>
                <li>「MCPサーバー」とは、Model Context Protocol（MCP）に準拠したサーバーアプリケーションをいいます。</li>
                <li>「コンテンツ」とは、ユーザーが本サービスを通じてアップロード、送信、または表示するデータ、テキスト、ソフトウェア、コード等をいいます。</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第3条（アカウント登録）</h2>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>本サービスの利用にはアカウント登録が必要です。</li>
                <li>ユーザーは、登録情報について正確かつ最新の情報を維持する責任を負います。</li>
                <li>ユーザーは、アカウントの管理について一切の責任を負い、第三者による使用を許可してはなりません。</li>
                <li>当社は、以下の場合にアカウントの登録を拒否または取り消すことができます。
                  <ul className="list-disc list-inside ml-4 mt-2 space-y-1">
                    <li>登録情報に虚偽の内容が含まれる場合</li>
                    <li>過去に本規約違反によりアカウントを削除されたことがある場合</li>
                    <li>その他当社が不適切と判断した場合</li>
                  </ul>
                </li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第4条（料金および支払い）</h2>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>本サービスには無料プランと有料プランがあります。各プランの内容は料金ページに記載のとおりとします。</li>
                <li>有料プランの料金は、Stripeを通じてクレジットカードにより支払うものとします。</li>
                <li>料金は前払いとし、契約期間の開始時に課金されます。</li>
                <li>いったん支払われた料金は、法令に定める場合を除き、返金いたしません。</li>
                <li>当社は、30日前までに通知することにより、料金を変更することができます。</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第5条（禁止事項）</h2>
              <p className="text-gray-600 mb-2">ユーザーは、本サービスの利用にあたり、以下の行為を行ってはなりません。</p>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>法令または公序良俗に違反する行為</li>
                <li>犯罪行為に関連する行為</li>
                <li>当社または第三者の知的財産権、肖像権、プライバシー、名誉、その他の権利または利益を侵害する行為</li>
                <li>本サービスのサーバーやネットワークに過度の負担をかける行為</li>
                <li>本サービスの運営を妨害する行為</li>
                <li>不正アクセスを試みる行為</li>
                <li>マルウェア、ウイルス、その他の有害なコードを配布する行為</li>
                <li>スパム、フィッシング、その他の迷惑行為</li>
                <li>他のユーザーになりすます行為</li>
                <li>本サービスを利用して違法なコンテンツをホスティングする行為</li>
                <li>当社が不適切と判断する行為</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第6条（サービスの停止・中断）</h2>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>当社は、以下の場合に、事前の通知なく本サービスの全部または一部を停止または中断することができます。
                  <ul className="list-disc list-inside ml-4 mt-2 space-y-1">
                    <li>システムの保守、点検、更新を行う場合</li>
                    <li>地震、落雷、火災、停電等の不可抗力により本サービスの提供が困難となった場合</li>
                    <li>その他当社が必要と判断した場合</li>
                  </ul>
                </li>
                <li>当社は、本条に基づく停止または中断によりユーザーに生じた損害について、一切の責任を負いません。</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第7条（知的財産権）</h2>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>本サービスに関する知的財産権は、当社または正当な権利を有する第三者に帰属します。</li>
                <li>ユーザーが本サービスを通じてアップロードしたコンテンツの知的財産権は、ユーザーに帰属します。</li>
                <li>ユーザーは、当社に対し、本サービスの提供に必要な範囲でコンテンツを利用する非独占的なライセンスを付与します。</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第8条（免責事項）</h2>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>当社は、本サービスに事実上または法律上の瑕疵がないことを保証しません。</li>
                <li>当社は、本サービスの利用に起因してユーザーに生じた損害について、当社の故意または重過失による場合を除き、一切の責任を負いません。</li>
                <li>当社が責任を負う場合でも、その責任は、ユーザーが当社に支払った直近12ヶ月分の利用料金を上限とします。</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第9条（サービス内容の変更・終了）</h2>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>当社は、ユーザーに事前に通知することなく、本サービスの内容を変更することができます。</li>
                <li>当社は、30日前までにユーザーに通知することにより、本サービスを終了することができます。</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第10条（利用規約の変更）</h2>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>当社は、必要に応じて本規約を変更することができます。</li>
                <li>変更後の利用規約は、本サービス上に掲載した時点から効力を生じるものとします。</li>
                <li>変更後に本サービスを利用した場合、ユーザーは変更後の規約に同意したものとみなします。</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第11条（準拠法・裁判管轄）</h2>
              <ol className="list-decimal list-inside space-y-2 text-gray-600">
                <li>本規約の解釈にあたっては、日本法を準拠法とします。</li>
                <li>本サービスに関して紛争が生じた場合には、東京地方裁判所を第一審の専属的合意管轄裁判所とします。</li>
              </ol>
            </section>

            <section className="mb-8">
              <h2 className="text-xl font-semibold text-gray-900 mb-4">第12条（お問い合わせ）</h2>
              <p className="text-gray-600">
                本規約に関するお問い合わせは、<a href="/contact" className="text-violet-600 hover:underline">お問い合わせフォーム</a>よりご連絡ください。
              </p>
            </section>
          </div>
        </div>
      </main>
      <Footer />
    </div>
  );
}
