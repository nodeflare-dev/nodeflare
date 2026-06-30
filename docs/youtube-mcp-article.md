# NodeFlareにデプロイした YouTube MCP を徹底解説 ― 「ただのAPIラッパー」と何が違うのか

> 対象サーバー: **`@kirbah/mcp-youtube`**（GitHub: [`kirbah/mcp-youtube`](https://github.com/kirbah/mcp-youtube)）
> ホスティング: **NodeFlare**（本記事執筆時点で稼働中 / status: `running`）

NodeFlare 上に YouTube の MCP サーバーをデプロイしました。この記事では、実際にデプロイした構成を見せながら、この MCP が何をしてくれるのか、そして **「YouTube Data API を叩くだけのラッパー」とは何が決定的に違うのか** を、ソースコードの実装レベルまで踏み込んで解説します。

---

## 1. そもそも MCP サーバーとは（30秒で）

MCP（Model Context Protocol）は、AI エージェント（Claude など）が外部のツールやデータにアクセスするための共通規格です。MCP サーバーは、AI に対して次の3種類の「能力」を公開できます。

- **Tools** … AI が呼び出せる関数（例: 「この動画の詳細を取って」）
- **Resources** … AI が読み取れるデータ（例: `youtube://transcript/{videoId}`）
- **Prompts** … 定型の指示テンプレート（例: ニッチ分析プロンプト）

この YouTube MCP は **3種類すべてを実装している** 数少ないサーバーです。多くの「週末プロジェクト」MCP が Tools だけなのに対し、これは設計思想からして本格的です。

---

## 2. NodeFlare 上の現在のデプロイ構成

まず、実際に NodeFlare にデプロイされている設定です（プラットフォームのデータベースから取得した実値）。

| 項目 | 値 | 補足 |
| --- | --- | --- |
| サーバー名 / slug | `mcp-youtube` | |
| リポジトリ | `kirbah/mcp-youtube` (`main`) | |
| ランタイム | `node` | Node.js `>=20` |
| トランスポート | `stdio` | NodeFlare が **Streamable HTTP に自動変換** |
| ビルドコマンド | `npm run build` | 中身は `tsc`（TypeScript コンパイル） |
| 起動コマンド | `npm start` | 中身は `node dist/index.js` |
| マシンメモリ | `256 MB` | 軽量。キャッシュは外部 MongoDB なので十分 |
| 認証 | 有効（NodeFlare 認証レイヤー） | |
| リージョン | `iad`（US East） | |
| 環境変数 | `YOUTUBE_API_KEY`, `MDB_MCP_CONNECTION_STRING` | キーのみ。値は暗号化保存 |
| ステータス | `running` | |

### この構成のポイント

- **stdio のサーバーを HTTP で公開している**
  `@kirbah/mcp-youtube` 本体は `StdioServerTransport`（標準入出力）で動く、いわゆる「ローカル前提」の MCP です。通常は Claude Desktop が `npx` でローカル起動する使い方を想定しています。NodeFlare は、この stdio サーバーをコンテナ内で起動し、**stdin/stdout を Streamable HTTP エンドポイントに変換**してインターネットに公開します。これにより、ローカルに Node も npx も用意せず、URL を指定するだけでどこからでも使えるようになります。

- **環境変数を作成時に投入できる**
  この MCP は `YOUTUBE_API_KEY`（YouTube Data API v3 のキー）と `MDB_MCP_CONNECTION_STRING`（MongoDB 接続文字列）を必要とします。NodeFlare ではサーバー作成フォームでこれらを設定し、**初回デプロイ前に暗号化保存**されるため、「デプロイ → 環境変数設定 → 再デプロイ」の二度手間が発生しません。

- **メモリ 256 MB で足りる理由**
  重いキャッシュ状態をプロセス内に持たず、外部 MongoDB に逃がす設計なので、サーバー自体はステートレスで軽量です。

---

## 3. このサーバーが提供するツール一覧

| ツール | 何をするか | YouTube API クォータ消費 |
| --- | --- | --- |
| `getVideoDetails` | 複数動画のメタデータ・統計・エンゲージメント比率・コンテンツ詳細を**軽量化**して取得 | 1 |
| `searchVideos` | クエリで動画/チャンネルを検索（多彩なフィルタ） | 100 |
| `getTranscripts` | 字幕（文字起こし）を取得。全文 or 要点（イントロ/アウトロ）抽出 | **0** |
| `getChannelStatistics` | チャンネルの登録者数・総再生数・動画数などを**軽量**に取得 | 1 |
| `getChannelTopVideos` | チャンネルの人気動画を**軽量**に取得 | 101 |
| `getTrendingVideos` | 地域・カテゴリ別のトレンド動画 | 1 |
| `getVideoCategories` | 地域別の動画カテゴリ一覧 | 1 |
| `getVideoComments` | コメント取得（並び順・件数・返信数を制御可能） | 1〜 |
| `findConsistentOutlierChannels` | ニッチ内で「規模の割に伸びている」チャンネルを多段階分析で発見（**MongoDB 必須**） | 多い |

> YouTube Data API の無料枠は **1日10,000ユニット**。検索系（`search.list` = 100ユニット）が突出して高く、`videos.list` / `channels.list` などは 1 ユニットです。後述のキャッシュは、この貴重なクォータを守るための仕組みです。

加えて、**Resource**（`youtube://transcript/{videoId}` で文字起こしを直接読める）と **Prompt**（`analyzeNiche` というニッチ分析テンプレート）も提供します。

---

## 4. 本題 ―「ただのAPIラッパー」と何が違うのか

「YouTube API を呼ぶだけなら、薄いラッパーを書けば済むのでは？」と思うかもしれません。しかしこのサーバーの価値は、**API と AI の“あいだ”で行う加工と最適化**にあります。実装を見ていきます。

### 違い① レスポンスを「LLM 向けに痩せさせる」(最大87%のトークン削減)

YouTube API の生レスポンスは、AI にとって不要な情報（多数のサムネイル URL、eTag、ローカライズ文字列、ネストされたメタデータ）で膨れています。これをそのまま LLM に渡すと、コンテキストウィンドウを浪費し、コストも増え、ノイズで推論精度も落ちます。

このサーバーは「LLM が推論に必要な情報だけ」を返すよう構造化します。README の実測値:

| メソッド | 生API | 最適化後 | 削減率 |
| --- | --- | --- | --- |
| `getChannelStatistics` | 673 トークン | **86** | **約87%減** |
| `getVideoDetails` | 854 トークン | **209** | **約75%減** |
| `searchVideos` | 1115 トークン | **402** | **約64%減** |

実装上は、

- キャッシュ保存時点で `snippet.thumbnails` などを `omitPaths()` で**物理的に除去**してから保存（`cache.service.ts`）
- 数値は `parseYouTubeNumber()` で文字列→数値へ正規化
- 説明文は `formatDescription()` で `NONE / SNIPPET / LONG` の粒度を選択可能
- 必要なフィールドだけを手で組み立てた「Lean（痩せた）オブジェクト」を返す（`LeanChannelStatistics`, `LeanTrendingVideo` などの専用型）

つまり「API の戻り値をそのまま転送」ではなく、**LLM 用のデータモデルへ変換するレイヤー**を持っているのが第一の違いです。

### 違い② MongoDB キャッシュによる「クォータ保護」

ラッパーは毎回 API を叩きます。AI エージェントはループに陥ったり同じ質問を繰り返したりしがちなので、これだと 10,000 ユニットの日次クォータが数分で枯渇しかねません。

このサーバーは **MongoDB を使った汎用キャッシュ層**を持ちます（`MDB_MCP_CONNECTION_STRING` を設定したとき有効）。中核は `getOrSet()` という1つのメソッドです（`cache.service.ts`）。

- キャッシュキーは、単純な ID（videoId など）か、**引数を SHA256 でハッシュ化**したもの。引数オブジェクトはキーをソートしてからハッシュするため、`{a,b}` と `{b,a}` が同じキーになる（取りこぼし防止）
- ヒットすれば **API クォータ消費 0** で即返却
- ミス時のみ API を実行し、痩せさせてから `expiresAt` 付きで `upsert`

さらに、データの性質に応じた **TTL の階層設計**（`cache.config.ts`）が秀逸です:

| 区分 | TTL | 対象 |
| --- | --- | --- |
| `DYNAMIC` | 1日 | トレンド動画、コメント |
| `STANDARD` | 1週間 | 動画詳細、検索結果、チャンネル統計 |
| `SEMI_STATIC` | 1か月 | チャンネルの人気動画 |
| `STATIC` | 1年 | 動画カテゴリ、**文字起こし** |

「カテゴリや文字起こしは変わらない／トレンドは毎日変わる」という現実に合わせてキャッシュ期間を変えています。これは単なるラッパーには絶対にない、**ドメイン知識を埋め込んだ最適化**です。

### 違い③ 文字起こしを「APIクォータ0」で取得し、要点だけ抜く

`getTranscripts` は YouTube Data API を**使いません**（`youtube-transcript-plus` で字幕を直接取得）。したがって **クォータ消費ゼロ**、かつ API キーすら不要（このサーバーは「ゼロコンフィグ＝APIキーなし」でも文字起こしだけは動く）。

しかも、長い全文をそのまま返すのではなく、`key_segments` モードでは（`transcript.service.ts`）:

- **Hook**: 最初の40秒の発話を抽出（動画の掴み）
- **Outro**: 最後の30秒の発話を抽出（CTA・まとめ）

を返します。動画分析で本当に重要な「最初と最後」だけを渡すことで、コンテキストを節約しつつ要点を逃しません。`full_text` を選べば全文も取れます。

### 違い④ 「素のAPIにない指標」を計算して付加する

API は再生数・いいね数・コメント数を別々に返すだけです。このサーバーは取得時に、

- `likeToViewRatio`（いいね/再生）
- `commentToViewRatio`（コメント/再生）

を**その場で計算して付与**します（`engagementCalculator.ts`、小数5桁）。AI は規模の異なる動画どうしを「率」で公平に比較できます。これは API を呼ぶだけでは得られない、**派生データの生成**です。

### 違い⑤ 単機能ではなく「分析パイプライン」を内蔵 ― `findConsistentOutlierChannels`

極めつけが `findConsistentOutlierChannels` です。「あるニッチで、登録者数の割に再生が伸びている“化けそうな”チャンネル」を見つけるツールで、**4段階の分析パイプライン**として実装されています（MongoDB 必須）:

1. **候補探索** … クエリで関連動画/チャンネルを広く収集
2. **チャンネル絞り込み** … 統計を取得し、`channelAge`（NEW: 6か月未満 / ESTABLISHED: 6〜24か月）や最低動画数でフィルタ
3. **深掘り分析** … 各チャンネルの上位動画について「viral factor（再生数 ÷ 登録者数）」を算出し、外れ値の一貫性（`consistencyLevel`: MODERATE≒30% / HIGH≒50%）と大きさ（`outlierMagnitude`: STANDARD=再生>登録者 / STRONG=再生>登録者×3）を評価
4. **ランキング & 整形** … 一貫性・外れ値度・総合パフォーマンスでランク付けし、LLM 向けに痩せた構造で返却

複数の API 呼び出し・統計計算・データベース集計を束ねた、**ひとつの“分析プロダクト”**です。ラッパーの発想ではここまで到達しません。

### 違い⑥ 「壊れない」ための作り込み（Production-Grade）

AI クライアントを巻き込んでクラッシュしないことも重要な差別化です。

- **テストカバレッジ約97%**、Lint エラー/警告ゼロ
- 全ツール入力を **Zod** で検証
- コメント無効動画（403 `commentsDisabled`）を空配列で握りつぶすなど、**実運用のエッジケース対応**
- バッチ取得は `Promise.allSettled` で一部失敗を許容しつつ全滅時のみエラー
- Dependabot による依存自動更新

---

## 5. アーキテクチャ俯瞰

```
AI エージェント (Claude 等)
        │  MCP (Streamable HTTP)
        ▼
┌─────────────────────────────────────────┐
│ NodeFlare                                │
│  ・stdio ⇄ Streamable HTTP 変換          │
│  ・認証レイヤー / 暗号化された環境変数    │
│  ┌───────────────────────────────────┐   │
│  │ @kirbah/mcp-youtube (Node, 256MB) │   │
│  │   Tools / Resources / Prompts     │   │
│  │        │                          │   │
│  │        ├─ CacheService (getOrSet) │──┼──▶ MongoDB（クォータ保護キャッシュ）
│  │        ├─ YoutubeService (Lean化) │──┼──▶ YouTube Data API v3
│  │        └─ TranscriptService(0費用)│──┼──▶ 字幕取得（API不使用）
│  └───────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

主要技術スタック: `@modelcontextprotocol/sdk` / `googleapis` / `mongodb` / `youtube-transcript-plus` / `zod`（Node.js ≥20, TypeScript）

---

## 6. まとめ ―「APIラッパー」と「MCPサーバー」の境界

| 観点 | 単なるAPIラッパー | この YouTube MCP |
| --- | --- | --- |
| レスポンス | API の生JSONをそのまま | LLM 向けに最大87%痩せさせた Lean データ |
| クォータ | 毎回消費 | MongoDB キャッシュ＋TTL階層で 0 消費を狙う |
| 文字起こし | API依存 | API不使用(0費用)＋要点抽出 |
| 指標 | API が返す値のみ | エンゲージメント率などを自前計算 |
| 機能粒度 | 1API=1関数 | 多段階の分析パイプラインを内蔵 |
| 信頼性 | その場しのぎ | 97%テスト＋Zod＋エッジケース対応 |
| MCP対応 | Tools のみが多い | Tools / Resources / Prompts すべて |

`@kirbah/mcp-youtube` は、「API を呼ぶ」のではなく **「AI が YouTube を使いやすいように、API と AI のあいだを設計する」** という思想で作られています。トークン最適化・クォータ保護・派生指標・分析パイプラインという付加価値こそが、ラッパーとの本質的な違いです。

そして NodeFlare は、この本来ローカル前提（stdio）のサーバーを、**環境変数の事前投入・stdio→HTTP 変換・認証付き公開**によって、誰でも URL ひとつで使えるクラウドサービスに変えています。「優れた MCP を書くこと」と「それを世界に公開すること」は別のスキルですが、後者を NodeFlare が引き受ける、という構図です。

---

### 付録: 自分で使うときの最小設定

```jsonc
{
  "mcpServers": {
    "youtube": {
      "command": "npx",
      "args": ["-y", "@kirbah/mcp-youtube"],
      "env": {
        "YOUTUBE_API_KEY": "あなたのキー",
        // 任意だが強く推奨（キャッシュ＝クォータ保護が有効になる）
        "MDB_MCP_CONNECTION_STRING": "mongodb+srv://.../youtube_niche_analysis"
      }
    }
  }
}
```

API キーなしでも `getTranscripts` だけは動きます（ゼロコンフィグ）。本格運用するなら YouTube API キーと MongoDB を両方入れるのがおすすめです。NodeFlare 経由なら、これらをフォームで入れてデプロイするだけで、上記のローカル設定なしに HTTP エンドポイントとして使えます。
