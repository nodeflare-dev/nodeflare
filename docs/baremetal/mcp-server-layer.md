# MCP Server層の最適化

MCPサーバーのコンテナイメージと言語別の最適化設定。

## 前提条件

- **全言語対応**: Python, Node.js, Go, Rust
- **効果が明確なもののみ採用**: 文書化された改善効果があること

---

## 1. コンテナイメージ軽量化

### 効果

| 最適化 | 効果 | ソース |
|-------|------|-------|
| マルチステージビルド | 87%サイズ削減 | [iximiuz Labs](https://labs.iximiuz.com/tutorials/docker-multi-stage-builds) |
| slim イメージ | 7x小さい | [OneUptime](https://oneuptime.com/blog/post/2026-02-08-how-to-reduce-docker-image-size-for-python-applications/view) |
| devDependencies削除 | 60-80%削減 | [OneUptime](https://oneuptime.com/blog/post/2026-02-08-how-to-reduce-docker-image-size-for-nodejs-applications/view) |

### なぜ重要か

- **起動速度**: イメージが小さいほどpullが速い（Nydusでも効果あり）
- **メモリ**: 不要なライブラリがロードされない
- **セキュリティ**: 攻撃対象が減少

---

## 2. ベースイメージ選択

### Python

| イメージ | サイズ | 採用 |
|---------|-------|------|
| `python:3.12` | 1.0GB | ❌ |
| **`python:3.12-slim`** | **150MB** | ✅ |
| `python:3.12-alpine` | 55MB | ❌ |

**alpine不採用の理由**:
- musl libc による互換性問題
- numpy, pandas等のコンパイルが必要になり逆に大きくなる
- Node.jsでも同様の問題（OS層で言及済み）

### Node.js

| イメージ | サイズ | 採用 |
|---------|-------|------|
| `node:20` | 1.1GB | ❌ |
| **`node:20-slim`** | **200MB** | ✅ |
| `node:20-alpine` | 140MB | △ 互換性注意 |

### Go / Rust

| イメージ | サイズ | 採用 |
|---------|-------|------|
| `golang:1.22` / `rust:1.75` | 800MB+ | ビルドのみ |
| **`scratch`** | **0MB** | ✅ 最終イメージ |
| `gcr.io/distroless/static` | 2MB | ✅ デバッグ用 |

**Go/Rustはスタティックバイナリ**: ランタイム依存なし、`scratch`イメージで最小化。

---

## 3. Python最適化

### Dockerfile

```dockerfile
# ============================================
# Python MCP Server - 最適化版
# ============================================

# ビルドステージ
FROM python:3.12-slim AS builder

WORKDIR /app

# 依存関係のみ先にコピー（キャッシュ活用）
COPY requirements.txt .

# --no-cache-dir: pipキャッシュを作らない（100MB以上削減可能）
# --user: /root/.local にインストール（コピーしやすい）
RUN pip install --no-cache-dir --user -r requirements.txt

# ============================================
# 本番ステージ
FROM python:3.12-slim

WORKDIR /app

# ビルドステージから依存関係のみコピー
COPY --from=builder /root/.local /root/.local
ENV PATH=/root/.local/bin:$PATH

# アプリケーションコード
COPY . .

# 不要ファイル削除
RUN find /root/.local -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true && \
    find /root/.local -type d -name "tests" -exec rm -rf {} + 2>/dev/null || true && \
    find /root/.local -type f -name "*.pyc" -delete 2>/dev/null || true

CMD ["python", "server.py"]
```

### 最適化ポイント

| 最適化 | 効果 |
|-------|------|
| `--no-cache-dir` | 100MB以上削減 |
| `python:3.12-slim` | 850MB削減 (vs full) |
| `__pycache__`削除 | 数MB削減 |
| マルチステージ | ビルドツール除外 |

### 環境変数

```dockerfile
# Pythonの最適化設定
ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1
```

| 変数 | 効果 |
|-----|------|
| `PYTHONDONTWRITEBYTECODE=1` | .pyc生成しない |
| `PYTHONUNBUFFERED=1` | ログ即時出力 |
| `PIP_NO_CACHE_DIR=1` | pipキャッシュ無効 |

---

## 4. Node.js最適化

### Dockerfile

```dockerfile
# ============================================
# Node.js MCP Server - 最適化版
# ============================================

# ビルドステージ
FROM node:20-slim AS builder

WORKDIR /app

# 依存関係ファイルのみ先にコピー
COPY package*.json ./

# 全依存関係インストール（ビルドに必要）
RUN npm ci

# ソースコードコピー & ビルド
COPY . .
RUN npm run build 2>/dev/null || true

# devDependencies削除（60-80%削減）
RUN npm prune --production

# 不要ファイル削除
RUN find node_modules -type f \( \
    -name "*.md" -o \
    -name "*.txt" -o \
    -name "LICENSE*" -o \
    -name "CHANGELOG*" -o \
    -name "*.map" -o \
    -name "*.ts" ! -name "*.d.ts" \
    \) -delete 2>/dev/null || true && \
    find node_modules -type d -name "test" -exec rm -rf {} + 2>/dev/null || true && \
    find node_modules -type d -name "__tests__" -exec rm -rf {} + 2>/dev/null || true

# ============================================
# 本番ステージ
FROM node:20-slim

WORKDIR /app

# 本番用node_modulesのみコピー
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/dist ./dist 2>/dev/null || true
COPY --from=builder /app/package*.json ./
COPY . .

CMD ["node", "server.js"]
```

### 最適化ポイント

| 最適化 | 効果 |
|-------|------|
| `npm prune --production` | 60-80%削減 |
| `node:20-slim` | 900MB削減 (vs full) |
| 不要ファイル削除 | 10-30%追加削減 |
| マルチステージ | ビルドツール除外 |

### 代替: `--omit=dev`

```dockerfile
# devDependenciesを最初からインストールしない
RUN npm ci --omit=dev
```

ビルド不要な場合はこちらがシンプル。

---

## 5. Go最適化

### Dockerfile

```dockerfile
# ============================================
# Go MCP Server - 最適化版
# ============================================

# ビルドステージ
FROM golang:1.22 AS builder

WORKDIR /app

# 依存関係
COPY go.mod go.sum ./
RUN go mod download

# ビルド
COPY . .
RUN CGO_ENABLED=0 GOOS=linux go build -ldflags="-s -w" -o server .

# ============================================
# 本番ステージ（スクラッチ）
FROM scratch

# CA証明書（HTTPS通信用）
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# バイナリのみコピー
COPY --from=builder /app/server /server

ENTRYPOINT ["/server"]
```

### 最適化ポイント

| 最適化 | 効果 |
|-------|------|
| `CGO_ENABLED=0` | 完全スタティックバイナリ |
| `-ldflags="-s -w"` | デバッグ情報除去（30%削減） |
| `scratch`ベース | OS層ゼロ |

### 最終イメージサイズ

| ステージ | サイズ |
|---------|-------|
| ビルドステージ | 800MB |
| **本番イメージ** | **5-20MB** |

---

## 6. Rust最適化

### Dockerfile

```dockerfile
# ============================================
# Rust MCP Server - 最適化版
# ============================================

# ビルドステージ
FROM rust:1.75 AS builder

WORKDIR /app

# 依存関係キャッシュ用の空プロジェクト
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# 実際のソース
COPY . .
RUN cargo build --release

# ============================================
# 本番ステージ
FROM scratch

COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=builder /app/target/release/server /server

ENTRYPOINT ["/server"]
```

### Cargo.toml最適化

```toml
[profile.release]
lto = true          # Link Time Optimization
codegen-units = 1   # 単一コード生成ユニット
panic = "abort"     # パニック時即終了（バイナリ縮小）
strip = true        # シンボル除去
```

### 最終イメージサイズ

| ステージ | サイズ |
|---------|-------|
| ビルドステージ | 1.5GB |
| **本番イメージ** | **3-15MB** |

---

## 7. .dockerignore

全言語共通で必須:

```
# .dockerignore

# Git
.git
.gitignore

# IDE
.idea
.vscode
*.swp

# ドキュメント
*.md
LICENSE
docs/

# テスト
tests/
__tests__/
*_test.go
*_test.rs

# ビルド成果物（ホスト側）
node_modules/
__pycache__/
*.pyc
target/
dist/

# 環境設定
.env
.env.*
*.local

# Docker
Dockerfile*
docker-compose*
.dockerignore
```

**効果**: ビルドコンテキスト転送時間短縮、不要ファイル除外。

---

## 8. レイヤーキャッシュ最適化

### 悪い例

```dockerfile
# 毎回全て再ビルド
COPY . .
RUN npm install
```

### 良い例

```dockerfile
# 依存関係ファイルのみ先にコピー
COPY package*.json ./
RUN npm install

# アプリコードは後（依存関係キャッシュ活用）
COPY . .
```

**効果**: ビルド時間70%削減（依存関係が変わらない限りキャッシュ利用）。

---

## 9. イメージサイズ比較

### 最適化前後

| 言語 | 最適化前 | 最適化後 | 削減率 |
|-----|---------|---------|-------|
| Python | 1.0GB | 150-200MB | 80-85% |
| Node.js | 1.1GB | 100-200MB | 80-90% |
| Go | 800MB | 5-20MB | 97-99% |
| Rust | 1.5GB | 3-15MB | 99% |

---

## 10. Nydus形式への変換

Container Runtime層でNydus lazy pullを採用するため、最適化したDockerイメージをNydus形式に変換する。

### nydusifyツール

```bash
# nydusifyインストール
wget https://github.com/dragonflyoss/nydus/releases/latest/download/nydus-static-v2.2.4-linux-amd64.tgz
tar -xzf nydus-static-*.tgz
sudo mv nydus-static/nydusify /usr/local/bin/
```

### 変換コマンド

```bash
# Python MCPサーバーイメージを変換
nydusify convert \
  --source registry.example.com/mcp-python:latest \
  --target registry.example.com/mcp-python:latest-nydus

# Node.js MCPサーバーイメージを変換
nydusify convert \
  --source registry.example.com/mcp-nodejs:latest \
  --target registry.example.com/mcp-nodejs:latest-nydus
```

### Builderでの自動変換

```rust
// crates/builder/src/nydus.rs
async fn convert_to_nydus(source_image: &str, target_image: &str) -> Result<()> {
    let output = Command::new("nydusify")
        .args(&["convert", "--source", source_image, "--target", target_image])
        .output()
        .await?;

    if !output.status.success() {
        return Err(anyhow!("Nydus conversion failed"));
    }
    Ok(())
}
```

### 言語別効果

| 言語 | イメージサイズ | Nydus効果 |
|-----|--------------|----------|
| Python | 150-200MB | ◎ 起動時に必要な部分のみDL |
| Node.js | 100-200MB | ◎ node_modulesの遅延読み込み |
| Go | 5-20MB | △ 小さいので効果薄 |
| Rust | 3-15MB | △ 小さいので効果薄 |

**注**: Go/Rustは元々小さいためNydus変換は任意。Python/Node.jsは必須。

---

## 11. nodeflareテンプレート更新

### Unix Socket対応

ベアメタル環境では全MCPサーバーがUnix Socketでリッスンする（Network層参照）。

#### Node.js (stdio-adapter.cjs)

```javascript
// 変更前: TCPポート
// app.listen(PORT, '0.0.0.0');

// 変更後: Unix Socket
const fs = require('fs');
const socketPath = process.env.MCP_SOCKET_PATH || '/var/run/mcp/default.sock';

// 既存のソケットファイルを削除
if (fs.existsSync(socketPath)) {
    fs.unlinkSync(socketPath);
}

const server = http.createServer(app);
server.listen(socketPath, () => {
    fs.chmodSync(socketPath, '666');
    console.log(`MCP server listening on ${socketPath}`);
});

// グレースフルシャットダウン
process.on('SIGTERM', () => {
    server.close(() => {
        if (fs.existsSync(socketPath)) fs.unlinkSync(socketPath);
        process.exit(0);
    });
});
```

#### Python (server.py)

```python
import os
import socket

sock_path = os.environ.get('MCP_SOCKET_PATH', '/var/run/mcp/default.sock')

# Gunicorn使用時
# gunicorn --bind unix:$MCP_SOCKET_PATH app:app
```

### 推奨テンプレート構成

```
templates/
├── python/
│   ├── Dockerfile.optimized
│   ├── requirements.txt.example
│   └── server.py.example
├── nodejs/
│   ├── Dockerfile.optimized
│   ├── package.json.example
│   └── server.js.example  # Unix Socket対応
├── go/
│   ├── Dockerfile.optimized
│   └── main.go.example
└── rust/
    ├── Dockerfile.optimized
    ├── Cargo.toml.example
    └── main.rs.example
```

---

## 12. ランタイム最適化

### Python: Gunicorn設定

```python
# gunicorn.conf.py
workers = 2  # CPU数に応じて調整
worker_class = "uvicorn.workers.UvicornWorker"  # 非同期
keepalive = 65  # Proxyのtimeoutより長く
```

### Node.js: UV_THREADPOOL_SIZE

```dockerfile
ENV UV_THREADPOOL_SIZE=4
```

デフォルト4で通常十分。I/O heavy な場合のみ増加。

### Go: GOMAXPROCS

```dockerfile
ENV GOMAXPROCS=2
```

コンテナに割り当てたCPU数に合わせる。

---

## 13. 検証コマンド

### イメージサイズ確認

```bash
docker images | grep mcp
```

### レイヤー分析（dive）

```bash
# diveインストール
brew install dive  # macOS
apt install dive   # Debian

# 分析
dive <image-name>
```

### 不要ファイル検出

```bash
# コンテナ内で確認
docker run --rm -it <image> sh -c "du -sh /app/node_modules/*" | sort -h
```

---

## 14. 言語互換性

| 最適化 | Python | Node.js | Go | Rust |
|-------|--------|---------|-----|------|
| slimベース | ✅ | ✅ | N/A | N/A |
| マルチステージ | ✅ | ✅ | ✅ | ✅ |
| scratchベース | ❌ | ❌ | ✅ | ✅ |
| キャッシュ無効 | ✅ `--no-cache-dir` | ✅ `npm ci` | ✅ | ✅ |
| devDeps除外 | ✅ | ✅ `--omit=dev` | N/A | N/A |
| バイナリstrip | ❌ | ❌ | ✅ `-ldflags` | ✅ `strip=true` |

**全言語で大幅なサイズ削減が可能。**

---

## 参考資料

- [Docker Multi-Stage Builds](https://labs.iximiuz.com/tutorials/docker-multi-stage-builds)
- [Python Docker Image Optimization](https://oneuptime.com/blog/post/2026-02-08-how-to-reduce-docker-image-size-for-python-applications/view)
- [Node.js Docker Image Optimization](https://oneuptime.com/blog/post/2026-02-08-how-to-reduce-docker-image-size-for-nodejs-applications/view)
- [pip --no-cache-dir](https://protsenko.dev/infrastructure-security/using-pip-install-without-no-cache-dir/)
- [DevOpsCube: Reduce Docker Image Size](https://devopscube.com/reduce-docker-image-size/)