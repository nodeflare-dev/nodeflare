-- Add locale support to error_hints for internationalization
ALTER TABLE error_hints ADD COLUMN locale VARCHAR(10) NOT NULL DEFAULT 'en';

-- Create index for efficient locale-based queries
CREATE INDEX idx_error_hints_locale ON error_hints(locale);

-- Add locale preference to user_preferences
ALTER TABLE user_preferences ADD COLUMN locale VARCHAR(10) NOT NULL DEFAULT 'en';

-- Insert Japanese translations for all existing error hints

-- Git-related errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['remote branch', 'not found'],
'💡 ヒント: 指定されたブランチがリポジトリに存在しません。
MCPサーバーの設定でブランチ名を確認してください。
一般的なブランチ名: ''main'', ''master'', ''develop''', 100, 'git', 'ja'),

(ARRAY['repository not found'],
'💡 ヒント: リポジトリが見つかりませんでした。
- リポジトリURLが正しいか確認してください
- プライベートリポジトリの場合、GitHub Appがインストールされているか確認してください
- リポジトリが存在し、アクセス可能か確認してください', 90, 'git', 'ja'),

(ARRAY['authentication failed'],
'💡 ヒント: リポジトリへの認証に失敗しました。
- プライベートリポジトリの場合、GitHub Appを再インストールしてください
- リポジトリの権限設定を確認してください', 90, 'git', 'ja'),

(ARRAY['could not read from remote'],
'💡 ヒント: リモートリポジトリから読み込めませんでした。
- プライベートリポジトリの場合、GitHub Appを再インストールしてください
- リポジトリの権限設定を確認してください', 85, 'git', 'ja');

-- Dockerfile/Build errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['dockerfile', 'not found'],
'💡 ヒント: リポジトリにDockerfileが見つかりませんでした。
- ルートディレクトリ（または指定したroot_directory）にDockerfileが存在するか確認してください
- ファイル名が正確に''Dockerfile''（大文字小文字を区別）であるか確認してください', 80, 'docker', 'ja'),

(ARRAY['no such file or directory'],
'💡 ヒント: 必要なファイルまたはディレクトリが見つかりませんでした。
- root_directory設定が正しいか確認してください
- 必要なファイルがすべてリポジトリにコミットされているか確認してください', 70, 'docker', 'ja');

-- npm/Node.js errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['postinstall'],
'💡 ヒント: npm installのpostinstallスクリプトが失敗しました。
これはおそらく、devDependency（simple-git-hooks、huskyなど）が
postinstallスクリプトで使用されているが、本番環境でインストールされていないためです。

package.jsonでの修正方法:
変更前: "postinstall": "simple-git-hooks"
変更後: "postinstall": "simple-git-hooks || true"
または: "postinstall": "npx simple-git-hooks || true"

これらのツールはローカル開発用であり、本番環境では不要です。', 95, 'npm', 'ja'),

(ARRAY['simple-git-hooks'],
'💡 ヒント: simple-git-hooksのpostinstallスクリプトが失敗しました。
このツールはローカル開発専用です。

package.jsonでの修正方法:
変更前: "postinstall": "simple-git-hooks"
変更後: "postinstall": "simple-git-hooks || true"', 96, 'npm', 'ja'),

(ARRAY['husky'],
'💡 ヒント: Huskyのpostinstall/prepareスクリプトが失敗しました。
このツールはローカル開発専用です。

package.jsonでの修正方法:
変更前: "prepare": "husky install"
変更後: "prepare": "husky install || true"', 96, 'npm', 'ja'),

(ARRAY['lefthook'],
'💡 ヒント: Lefthookのpostinstallスクリプトが失敗しました。
このツールはローカル開発専用です。

package.jsonでの修正方法:
変更前: "postinstall": "lefthook install"
変更後: "postinstall": "lefthook install || true"', 96, 'npm', 'ja'),

(ARRAY['npm err'],
'💡 ヒント: npmがインストール中にエラーを検出しました。
- package.jsonが有効か確認してください
- すべての依存関係が正しく指定されているか確認してください
- 認証が必要なプライベートパッケージがないか確認してください
- postinstallスクリプトを使用している場合、本番環境で動作するか確認してください', 50, 'npm', 'ja'),

(ARRAY['npm error'],
'💡 ヒント: npmがインストール中にエラーを検出しました。
- package.jsonが有効か確認してください
- すべての依存関係が正しく指定されているか確認してください
- 認証が必要なプライベートパッケージがないか確認してください', 50, 'npm', 'ja'),

(ARRAY['cannot find module'],
'💡 ヒント: 必要なNode.jsモジュールが見つかりませんでした。
- モジュールがdependenciesに記載されているか確認してください（devDependenciesではなく）
- package.jsonにモジュールが含まれているか確認してください
- import文にタイプミスがないか確認してください', 75, 'npm', 'ja');

-- Python errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['pip', 'error'],
'💡 ヒント: pipがインストール中にエラーを検出しました。
- requirements.txtまたはpyproject.tomlが有効か確認してください
- すべての依存関係がPyPIで利用可能か確認してください
- バージョンの競合がないか確認してください', 60, 'python', 'ja'),

(ARRAY['pip', 'failed'],
'💡 ヒント: pipのインストールが失敗しました。
- requirements.txtまたはpyproject.tomlが有効か確認してください
- すべての依存関係がPyPIで利用可能か確認してください
- バージョンの競合がないか確認してください', 60, 'python', 'ja'),

(ARRAY['uv', 'error'],
'💡 ヒント: uv（Pythonパッケージマネージャー）がエラーを検出しました。
- pyproject.tomlが有効か確認してください
- すべての依存関係が利用可能か確認してください
- 依存関係のバージョン競合がないか確認してください', 60, 'python', 'ja'),

(ARRAY['uv', 'failed'],
'💡 ヒント: uv（Pythonパッケージマネージャー）が失敗しました。
- pyproject.tomlが有効か確認してください
- すべての依存関係が利用可能か確認してください
- 依存関係のバージョン競合がないか確認してください', 60, 'python', 'ja'),

(ARRAY['modulenotfounderror'],
'💡 ヒント: 必要なPythonモジュールが見つかりませんでした。
- モジュールがrequirements.txtまたはpyproject.tomlに記載されているか確認してください
- モジュール名が正しく入力されているか確認してください
- import文にタイプミスがないか確認してください', 75, 'python', 'ja');

-- Rust/Cargo errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['cargo', 'error'],
'💡 ヒント: Cargo（Rust）のビルドが失敗しました。
- Cargo.tomlが有効か確認してください
- すべての依存関係が正しくコンパイルされるか確認してください
- コードにコンパイルエラーがないか確認してください', 60, 'rust', 'ja');

-- TypeScript errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['typescript'],
'💡 ヒント: TypeScriptのコンパイルが失敗しました。
- コードの型エラーを確認してください
- tsconfig.jsonが正しく設定されているか確認してください
- すべての型依存関係（@types/*）がインストールされているか確認してください', 55, 'typescript', 'ja'),

(ARRAY['tsc'],
'💡 ヒント: TypeScriptコンパイラ（tsc）が失敗しました。
- コードの型エラーを確認してください
- tsconfig.jsonが正しく設定されているか確認してください', 55, 'typescript', 'ja');

-- Syntax errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['syntaxerror'],
'💡 ヒント: コードに構文エラーが検出されました。
- エラーメッセージでファイルと行番号を確認してください
- よくある原因: 括弧の不足、無効なJSON、タイプミス
- コードが言語/ランタイムのバージョンに対して有効か確認してください', 70, 'syntax', 'ja'),

(ARRAY['syntax error'],
'💡 ヒント: コードに構文エラーが検出されました。
- エラーメッセージでファイルと行番号を確認してください
- よくある原因: 括弧の不足、無効なJSON、タイプミス', 70, 'syntax', 'ja');

-- ES Module errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['unexpected token', 'export'],
'💡 ヒント: ES Moduleのimport/exportエラーです。
- Node.jsの場合: package.jsonに "type": "module" を追加するか、.mjs拡張子を使用してください
- またはexportsをmodule.exports構文に変更してCommonJSにしてください
- package.jsonの "type" フィールドがコードスタイルと一致しているか確認してください', 80, 'esmodule', 'ja'),

(ARRAY['cannot use import statement'],
'💡 ヒント: ES Moduleのimport文エラーです。
- Node.jsの場合: package.jsonに "type": "module" を追加するか、.mjs拡張子を使用してください
- またはimportsをrequire()構文に変更してCommonJSにしてください
- package.jsonの "type" フィールドがコードスタイルと一致しているか確認してください', 80, 'esmodule', 'ja');

-- JSON errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['json', 'parse'],
'💡 ヒント: JSONパースエラーです。
- package.jsonやその他のJSONファイルの構文エラーを確認してください
- よくある問題: 末尾のカンマ、引用符の不足、無効な文字
- JSONバリデーターを使用してファイルを確認してください', 65, 'json', 'ja'),

(ARRAY['json', 'unexpected token'],
'💡 ヒント: 予期しないトークンのJSONパースエラーです。
- package.jsonやその他のJSONファイルの構文エラーを確認してください
- よくある問題: 末尾のカンマ、引用符の不足、無効な文字', 65, 'json', 'ja');

-- Port/Network errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['port', 'already in use'],
'💡 ヒント: ポートの競合が検出されました。
- アプリケーションが既に使用中のポートを使用しようとしている可能性があります
- アプリケーションのポート設定を確認してください', 70, 'network', 'ja'),

(ARRAY['address already'],
'💡 ヒント: アドレスが既に使用中です。
- 別のプロセスが同じポートを使用しています
- アプリケーションのポート設定を確認してください', 70, 'network', 'ja');

-- Memory/Resource errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['out of memory'],
'💡 ヒント: ビルド中にメモリが不足しました。
- 依存関係のサイズを削減してみてください
- ビルドプロセスの最適化を検討してください
- 問題が続く場合はサポートにお問い合わせください', 80, 'resource', 'ja'),

(ARRAY['oom'],
'💡 ヒント: メモリ不足（OOM）エラーです。
- 依存関係のサイズを削減してみてください
- ビルドプロセスの最適化を検討してください', 80, 'resource', 'ja'),

(ARRAY['killed'],
'💡 ヒント: プロセスが強制終了されました（OOMの可能性）。
- ビルド中にメモリが不足した可能性があります
- 依存関係のサイズを削減してみてください', 60, 'resource', 'ja');

-- Timeout errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['timeout'],
'💡 ヒント: 操作がタイムアウトしました。
- ビルドまたはデプロイに時間がかかりすぎました
- ビルドプロセスを簡素化してみてください
- ビルド中の遅いネットワーク操作を確認してください', 65, 'timeout', 'ja'),

(ARRAY['timed out'],
'💡 ヒント: 操作がタイムアウトしました。
- ビルドまたはデプロイに時間がかかりすぎました
- ビルドプロセスを簡素化してみてください', 65, 'timeout', 'ja');

-- Permission errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['permission denied'],
'💡 ヒント: 権限拒否エラーです。
- リポジトリのファイル権限を確認してください
- 必要に応じてスクリプトに実行権限があるか確認してください', 70, 'permission', 'ja'),

(ARRAY['eacces'],
'💡 ヒント: 権限拒否（EACCES）エラーです。
- リポジトリのファイル権限を確認してください
- 必要に応じてスクリプトに実行権限があるか確認してください', 70, 'permission', 'ja');

-- Entry point errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['entrypoint'],
'💡 ヒント: エントリーコマンドを実行できませんでした。
- entry_command設定を確認してください
- コンテナ内に実行ファイルが存在するか確認してください
- コマンドパスが正しいか確認してください', 75, 'entrypoint', 'ja'),

(ARRAY['command not found'],
'💡 ヒント: コマンドが見つかりませんでした。
- entry_command設定を確認してください
- コンテナ内に実行ファイルが存在するか確認してください
- コマンドパスが正しいか確認してください', 75, 'entrypoint', 'ja'),

(ARRAY['no such file', 'exec'],
'💡 ヒント: 実行ファイルが見つかりませんでした。
- entry_command設定を確認してください
- コンテナ内に実行ファイルが存在するか確認してください', 75, 'entrypoint', 'ja');

-- Health check errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['health check'],
'💡 ヒント: ヘルスチェックが失敗しました。
- サーバーが起動し、正しいポートでリッスンしているか確認してください
- SSEトランスポートの場合: ポート3000（Node）、8000（Python）、8080（Go/Rust）
- STDIOトランスポートの場合: アダプターはポート8000で実行されます
- サーバーが起動時にクラッシュしていないか確認してください', 85, 'health', 'ja'),

(ARRAY['healthcheck'],
'💡 ヒント: ヘルスチェックが失敗しました。
- サーバーが起動し、正しいポートでリッスンしているか確認してください
- SSEトランスポートの場合: ポート3000（Node）、8000（Python）、8080（Go/Rust）
- STDIOトランスポートの場合: アダプターはポート8000で実行されます', 85, 'health', 'ja');

-- SSL/TLS errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['ssl'],
'💡 ヒント: SSL/TLSエラーが発生しました。
- アプリケーションが有効な証明書でHTTPSリクエストを行っているか確認してください
- 外部APIエンドポイントが正しく設定されているか確認してください', 55, 'ssl', 'ja'),

(ARRAY['certificate'],
'💡 ヒント: 証明書エラーが発生しました。
- アプリケーションが有効な証明書でHTTPSリクエストを行っているか確認してください
- 外部APIエンドポイントが正しく設定されているか確認してください', 55, 'ssl', 'ja'),

(ARRAY['tls'],
'💡 ヒント: TLSエラーが発生しました。
- アプリケーションが有効な証明書でHTTPSリクエストを行っているか確認してください
- 外部APIエンドポイントが正しく設定されているか確認してください', 55, 'ssl', 'ja');

-- Environment variable errors (Japanese)
INSERT INTO error_hints (keywords, hint_message, priority, category, locale) VALUES
(ARRAY['environment variable'],
'💡 ヒント: 必要な環境変数がありません。
- サーバー設定で必要なシークレットを追加してください
- アプリケーションの環境要件を確認してください', 75, 'env', 'ja'),

(ARRAY['env var'],
'💡 ヒント: 必要な環境変数がありません。
- サーバー設定で必要なシークレットを追加してください', 75, 'env', 'ja'),

(ARRAY['missing required'],
'💡 ヒント: 必要な設定がありません。
- サーバー設定で必要なシークレットを追加してください
- アプリケーションの環境要件を確認してください', 60, 'env', 'ja');
