# Network層の最適化

MCP専用ベアメタルサーバーにおけるNetwork層の最適化設定。

## 前提条件

- **全言語対応**: Python, Node.js, Go, Rust等のMCPサーバーが動作すること
- **Proxy → MCPサーバー間の通信最適化**が主目的
- **外部からのTLS終端はCaddyが担当**（この層では扱わない）
- **Unix Domain Socket採用**: 最高のパフォーマンスを実現

---

## 1. 通信方式: Unix Domain Socket

### 選択理由

| 方式 | レイテンシ | スループット | 採用 |
|-----|-----------|-------------|------|
| TCP over network (Fly.io現状) | 10-50ms | 制限あり | ❌ |
| TCP localhost | 3.6µs | ベースライン | ❌ |
| **Unix Domain Socket** | **2.3µs (36%減)** | **2-3x向上** | ✅ |

### ベンチマーク結果

| ソース | UDS vs TCP |
|-------|-----------|
| Go benchmark | 3x高速 (4.5ms vs 14.7ms per 100K pingpong) |
| PostgreSQL | 40%レイテンシ削減、67%スループット向上 |
| Redis | 50%スループット向上 |
| Node.js | 50%レイテンシ削減 (130µs vs 334µs) |
| Docker内 | 40-45%性能向上 |

### なぜ速いか

- TCP/IPスタックを完全バイパス（ACK、フロー制御、チェックサム等が不要）
- カーネル内でのデータコピーが少ない
- コンテキストスイッチが少ない
- ネットワークスタックのオーバーヘッドゼロ

### ベアメタルでのみ可能な理由

```
Fly.io (現状) - 別マシンなので不可能:
┌─────────────┐     HTTPS      ┌─────────────┐
│ Proxy       │ ──────────────→│ MCP Server  │
│ (Machine A) │    ネットワーク  │ (Machine B) │
└─────────────┘                └─────────────┘

ベアメタル - 同一マシンなので可能:
┌─────────────────────────────────────────┐
│            同一サーバー                   │
│  ┌───────┐  Unix Socket  ┌───────────┐  │
│  │ Proxy │ ────────────→ │MCP Server │  │
│  └───────┘               └───────────┘  │
└─────────────────────────────────────────┘
```

---

## 2. Proxy実装の変更

### 現状の課題

現在のProxyは`reqwest::Client`を使用:
- reqwestはUnix Domain Socketをネイティブサポートしていない
- `hyper` + `hyperlocal`への移行が必要

### 依存関係の変更

```toml
# crates/proxy/Cargo.toml

[dependencies]
# 削除
# reqwest = { version = "0.12", features = ["json", "rustls-tls", "stream"] }

# 追加
hyper = { version = "1.5", features = ["client", "http1"] }
hyper-util = { version = "0.1", features = ["client", "client-legacy", "tokio"] }
hyperlocal = "0.9"
http-body-util = "0.1"
tower-service = "0.3"
```

### HTTPクライアントの実装

```rust
// crates/proxy/src/unix_client.rs

use std::path::Path;
use hyper::body::Incoming;
use hyper::{Request, Response};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixClientExt, UnixConnector, Uri};
use http_body_util::{BodyExt, Full};
use bytes::Bytes;

/// Unix Socket用HTTPクライアント
pub struct UnixHttpClient {
    client: Client<UnixConnector, Full<Bytes>>,
}

impl UnixHttpClient {
    pub fn new() -> Self {
        let client = Client::unix();
        Self { client }
    }

    /// Unix Socket経由でリクエストを送信
    pub async fn request(
        &self,
        socket_path: &Path,
        path: &str,
        method: hyper::Method,
        headers: hyper::HeaderMap,
        body: Bytes,
    ) -> Result<Response<Incoming>, hyper_util::client::legacy::Error> {
        let uri = Uri::new(socket_path, path);

        let mut req = Request::builder()
            .method(method)
            .uri(uri);

        for (key, value) in headers.iter() {
            req = req.header(key, value);
        }

        let req = req.body(Full::new(body)).unwrap();

        self.client.request(req).await
    }
}

/// 接続プール付きクライアント
pub struct PooledUnixClient {
    // ソケットパスごとに接続を再利用
    // hyper-utilのClientは内部で接続プーリングを行う
    client: Client<UnixConnector, Full<Bytes>>,
}

impl PooledUnixClient {
    pub fn new() -> Self {
        Self {
            client: Client::unix(),
        }
    }
}
```

### forward_request の変更

```rust
// crates/proxy/src/main.rs

use crate::unix_client::UnixHttpClient;
use std::path::PathBuf;

pub struct ProxyState {
    // 変更前: pub http_client: reqwest::Client,
    pub unix_client: UnixHttpClient,
    // ... その他のフィールド
}

/// endpoint_url形式の変更
/// 変更前: "https://mcp-xxx.fly.dev/mcp"
/// 変更後: "/var/run/mcp/{server_id}.sock"
fn get_socket_path(server: &Server) -> PathBuf {
    PathBuf::from(format!("/var/run/mcp/{}.sock", server.id))
}

async fn forward_request(
    state: &ProxyState,
    server: &Server,
    path: &str,
    request: Request<Body>,
    credential: &AuthCredential,
) -> Result<(Response, McpRequestInfo), ProxyError> {
    let socket_path = get_socket_path(server);

    // リクエストボディを取得
    let (parts, body) = request.into_parts();
    let body_bytes = body.collect().await?.to_bytes();

    // ヘッダーを構築
    let mut headers = parts.headers.clone();
    // 認証ヘッダーを追加
    add_auth_headers(&mut headers, credential);

    // Unix Socket経由でリクエスト
    let response = state.unix_client
        .request(&socket_path, path, parts.method, headers, body_bytes)
        .await
        .map_err(|e| ProxyError::UpstreamError(e.to_string()))?;

    // レスポンスを変換
    Ok((convert_response(response).await?, build_request_info()))
}
```

### SSEストリーミングの対応

```rust
// crates/proxy/src/sse_streaming.rs

use futures::Stream;
use hyper::body::Incoming;
use http_body_util::BodyStream;

/// SSEストリーミング用のレスポンス変換
pub async fn execute_streaming_request(
    client: &UnixHttpClient,
    socket_path: &Path,
    path: &str,
    method: hyper::Method,
    headers: hyper::HeaderMap,
    body: Bytes,
) -> Result<impl Stream<Item = Result<Bytes, std::io::Error>>, ProxyError> {
    let response = client
        .request(socket_path, path, method, headers, body)
        .await?;

    // Incomingボディをストリームに変換
    let body_stream = BodyStream::new(response.into_body());

    Ok(body_stream.map(|result| {
        result
            .map(|frame| frame.into_data().unwrap_or_default())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }))
}
```

---

## 3. MCPサーバー側の変更

### 言語別 Unix Socket サポート

#### Python (Gunicorn + Flask/FastAPI)

```bash
# 起動コマンド
gunicorn --workers 4 --bind unix:/var/run/mcp/${SERVER_ID}.sock app:app
```

```python
# または直接指定
import socket
import os

sock_path = os.environ.get('MCP_SOCKET_PATH', '/var/run/mcp/default.sock')

# 既存のソケットファイルを削除
if os.path.exists(sock_path):
    os.unlink(sock_path)

# Unix Socketでリッスン
server = make_server('', 0, app, handler_class=WSGIRequestHandler)
server.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.socket.bind(sock_path)
server.socket.listen(128)
os.chmod(sock_path, 0o666)  # パーミッション設定
```

#### Node.js (Express/Fastify)

```javascript
// stdio-adapter.cjs の変更
const fs = require('fs');
const socketPath = process.env.MCP_SOCKET_PATH || '/var/run/mcp/default.sock';

// 既存のソケットファイルを削除
if (fs.existsSync(socketPath)) {
    fs.unlinkSync(socketPath);
}

const server = http.createServer(app);
server.listen(socketPath, () => {
    // パーミッション設定
    fs.chmodSync(socketPath, '666');
    console.log(`MCP server listening on ${socketPath}`);
});

// グレースフルシャットダウン
process.on('SIGTERM', () => {
    server.close(() => {
        if (fs.existsSync(socketPath)) {
            fs.unlinkSync(socketPath);
        }
        process.exit(0);
    });
});
```

#### Go

```go
package main

import (
    "net"
    "net/http"
    "os"
    "os/signal"
    "syscall"
)

func main() {
    socketPath := os.Getenv("MCP_SOCKET_PATH")
    if socketPath == "" {
        socketPath = "/var/run/mcp/default.sock"
    }

    // 既存のソケットファイルを削除
    os.Remove(socketPath)

    listener, err := net.Listen("unix", socketPath)
    if err != nil {
        panic(err)
    }
    defer listener.Close()

    // パーミッション設定
    os.Chmod(socketPath, 0666)

    // グレースフルシャットダウン
    go func() {
        sigChan := make(chan os.Signal, 1)
        signal.Notify(sigChan, syscall.SIGTERM, syscall.SIGINT)
        <-sigChan
        listener.Close()
        os.Remove(socketPath)
        os.Exit(0)
    }()

    http.Serve(listener, handler)
}
```

#### Rust (Axum)

```rust
use axum::{routing::post, Router};
use tokio::net::UnixListener;
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[tokio::main]
async fn main() {
    let socket_path = std::env::var("MCP_SOCKET_PATH")
        .unwrap_or_else(|_| "/var/run/mcp/default.sock".to_string());

    // 既存のソケットファイルを削除
    let _ = fs::remove_file(&socket_path);

    let app = Router::new()
        .route("/mcp", post(handle_mcp));

    let listener = UnixListener::bind(&socket_path).unwrap();

    // パーミッション設定
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o666)).unwrap();

    axum::serve(listener, app).await.unwrap();
}
```

### 互換性まとめ

| 言語 | サーバー | 変更量 |
|-----|---------|-------|
| Python | ✅ Gunicorn native | 起動オプションのみ |
| Node.js | ✅ http module | listen()の引数変更 |
| Go | ✅ net.Listen("unix") | リスナー変更 |
| Rust | ✅ tokio::net::UnixListener | リスナー変更 |

**全言語でUnix Socket対応可能**

---

## 4. Builder側の変更

### コンテナ起動設定

```rust
// crates/builder/src/container.rs

/// MCPサーバーコンテナを起動
async fn start_mcp_container(server: &Server) -> Result<()> {
    let socket_path = format!("/var/run/mcp/{}.sock", server.id);

    // ソケットディレクトリを確保
    std::fs::create_dir_all("/var/run/mcp")?;

    // containerd/nerdctl でコンテナ起動
    let output = Command::new("nerdctl")
        .args(&[
            "run",
            "-d",
            "--name", &format!("mcp-{}", server.id),
            "--net=host",  // host networkモード（ソケット共有のため）
            "-v", "/var/run/mcp:/var/run/mcp",  // ソケットディレクトリをマウント
            "-e", &format!("MCP_SOCKET_PATH={}", socket_path),
            &server.image_url,
        ])
        .output()
        .await?;

    Ok(())
}
```

### endpoint_url の形式変更

```rust
// データベーススキーマの変更は不要
// endpoint_url の意味が変わる：
//
// 変更前: "https://mcp-xxx.fly.dev/mcp" (HTTP URL)
// 変更後: "/var/run/mcp/{server_id}.sock" (Unix Socket パス)

impl Server {
    /// ソケットパスを取得（endpoint_urlフィールドを流用）
    pub fn socket_path(&self) -> PathBuf {
        PathBuf::from(&self.endpoint_url)
    }
}
```

---

## 5. TCP チューニング (外部接続用)

外部からの接続（Client → Caddy → Proxy）にはTCPを使用。

### MCPワークロードの特性

| 特性 | MCP | 高スループット設定が必要なケース |
|-----|-----|-------------------------------|
| メッセージサイズ | 数KB (JSON-RPC) | 数MB〜GB |
| パターン | リクエスト/レスポンス | バルク転送・ストリーミング |
| 重要な指標 | レイテンシ | スループット |

**結論**: 大きなTCPバッファ（16MB等）はMCPには不要。Linuxのautotuningで十分。

> "A too large value for the maximum buffer size can increase the latency"
> — [Cloudflare Blog](https://blog.cloudflare.com/optimizing-tcp-for-high-throughput-and-low-latency/)

### /etc/sysctl.d/99-mcp-network.conf

```bash
# ============================================
# MCP専用 Network チューニング
# ============================================

# --------------------------------------------
# 接続管理
# --------------------------------------------
# somaxconn は OS層 (/etc/sysctl.d/80-mcp.conf) で設定済み
net.core.netdev_max_backlog = 65535
net.ipv4.tcp_max_syn_backlog = 65535

# --------------------------------------------
# TIME_WAIT 管理
# --------------------------------------------
net.ipv4.tcp_fin_timeout = 10
net.ipv4.tcp_tw_reuse = 1
net.ipv4.tcp_max_tw_buckets = 1048576
net.ipv4.ip_local_port_range = 1024 65535

# --------------------------------------------
# Keepalive
# --------------------------------------------
net.ipv4.tcp_keepalive_time = 60
net.ipv4.tcp_keepalive_intvl = 10
net.ipv4.tcp_keepalive_probes = 6

# --------------------------------------------
# TCP Fast Open (1 RTT削減)
# --------------------------------------------
net.ipv4.tcp_fastopen = 3

# --------------------------------------------
# Congestion Control (BBR)
# --------------------------------------------
# BBR: Googleが開発した輻輳制御アルゴリズム
# - 高スループット、低レイテンシ、パケットロスに鈍感
# - WAN接続（Client→Proxy）で効果的
net.core.default_qdisc = fq
net.ipv4.tcp_congestion_control = bbr

# --------------------------------------------
# Low Latency Settings
# --------------------------------------------
net.ipv4.tcp_slow_start_after_idle = 0
net.ipv4.tcp_window_scaling = 1
net.ipv4.tcp_sack = 1
```

### 採用しなかった設定

| 設定 | 理由 |
|-----|------|
| `rmem_max/wmem_max = 16MB` | MCPは小メッセージ（数KB）。大バッファは逆にレイテンシ増加の原因 |
| `tcp_rmem/tcp_wmem` 最大値変更 | Linux autotuningがMCPワークロードに適切に調整 |
| `tcp_no_metrics_save = 1` | 効果が不明確 |

### TCP_NODELAY (アプリケーション側)

リクエスト/レスポンス型ではNagleアルゴリズム無効化が重要:

```rust
// Caddy/Proxyで設定
// hyper はデフォルトで TCP_NODELAY = true
```

```javascript
// Node.js
socket.setNoDelay(true);
```

### 適用方法

```bash
sudo cp 99-mcp-network.conf /etc/sysctl.d/
sudo sysctl --system
sysctl net.ipv4.tcp_congestion_control  # → bbr
```

---

## 6. 構成図

```
                    Internet
                        │
                        ▼
┌─────────────────────────────────────────────────────────┐
│  Caddy (TLS終端)                                        │
│  - HTTP/2, HTTP/3                                       │
│  - TLS 1.3                                              │
│  - TCP Fast Open                                        │
│  - TCP_NODELAY                                          │
└─────────────────────────────────────────────────────────┘
                        │
                        │ HTTP (localhost or Unix Socket)
                        ▼
┌─────────────────────────────────────────────────────────┐
│  MCP Proxy (Rust + hyper + hyperlocal)                  │
│  - Unix Domain Socket クライアント                       │
│  - 接続プーリング (hyper内蔵)                            │
│  - SO_REUSEPORT (複数ワーカー時)                         │
└─────────────────────────────────────────────────────────┘
                        │
                        │ Unix Socket (/var/run/mcp/*.sock)
                        ▼
┌─────────────────────────────────────────────────────────┐
│  MCP Server Containers (--net=host)                     │
│  - Python/Node.js/Go/Rust                               │
│  - Unix Socket でリッスン                                │
│  - /var/run/mcp/{server_id}.sock                        │
└─────────────────────────────────────────────────────────┘
```

---

## 7. ソケットファイル管理

### ディレクトリ構成

```
/var/run/mcp/
├── server-abc123.sock
├── server-def456.sock
└── server-ghi789.sock
```

### パーミッション

```bash
# ディレクトリ作成
sudo mkdir -p /var/run/mcp
sudo chmod 755 /var/run/mcp

# コンテナユーザーが書き込めるように
# （コンテナ内のユーザーIDに応じて調整）
sudo chown root:docker /var/run/mcp
sudo chmod 775 /var/run/mcp
```

### クリーンアップ

```bash
# systemd サービスで自動クリーンアップ
# /etc/systemd/system/mcp-socket-cleanup.service

[Unit]
Description=Cleanup stale MCP sockets
After=containerd.service

[Service]
Type=oneshot
ExecStart=/bin/rm -f /var/run/mcp/*.sock

[Install]
WantedBy=multi-user.target
```

---

## 8. ヘルスチェック

### Unix Socket経由のヘルスチェック

```bash
# curlでUnix Socketにアクセス
curl --unix-socket /var/run/mcp/server-abc123.sock http://localhost/health
```

```rust
// Proxy側でのヘルスチェック実装
async fn health_check(socket_path: &Path) -> bool {
    let client = UnixHttpClient::new();
    match client.request(
        socket_path,
        "/health",
        hyper::Method::GET,
        Default::default(),
        Bytes::new(),
    ).await {
        Ok(res) => res.status().is_success(),
        Err(_) => false,
    }
}
```

---

## 9. 期待される効果

| 最適化 | 効果 |
|-------|------|
| Unix Socket | TCP比 36%レイテンシ削減、2-3xスループット向上 |
| ネットワーク排除 | Fly.io比 10-50ms → <0.5ms (100倍改善) |
| TLS不要 | ハンドシェイク排除 |
| 接続プーリング | 接続確立コスト 0ms |
| sysctl チューニング | 外部接続の安定性向上 |
| TCP Fast Open | 外部接続で 1 RTT 削減 |

### 総合効果

| 経路 | 現状 (Fly.io) | ベアメタル (Unix Socket) |
|-----|--------------|------------------------|
| Proxy → MCP | 10-50ms | **<0.5ms** |
| 改善率 | - | **20-100倍** |

---

## 10. 実装タスク

### Proxy側 (crates/proxy)

1. [ ] `hyperlocal`依存関係を追加
2. [ ] `unix_client.rs`モジュールを作成
3. [ ] `forward_request()`をUnix Socket対応に変更
4. [ ] SSEストリーミングをhyper用に書き換え
5. [ ] reqwest依存を削除

### Builder側 (crates/builder)

1. [ ] コンテナ起動時にソケットパスを環境変数で渡す
2. [ ] `/var/run/mcp`をボリュームマウント
3. [ ] endpoint_urlをソケットパス形式に変更

### MCPサーバーテンプレート

1. [ ] stdio-adapter.cjs をUnix Socket対応に変更
2. [ ] Python/Go/Rustテンプレートを更新

---

## 11. 言語互換性確認

| 最適化 | Python | Node.js | Go | Rust |
|-------|--------|---------|-----|------|
| Unix Socket サーバー | ✅ | ✅ | ✅ | ✅ |
| グレースフルシャットダウン | ✅ | ✅ | ✅ | ✅ |
| パーミッション管理 | ✅ | ✅ | ✅ | ✅ |

**全言語で対応可能。nodeflareのコンセプトと矛盾なし。**

---

## 参考資料

- [Unix Domain Sockets: 50% Lower Latency](https://nodevibe.substack.com/p/the-nodejs-developers-guide-to-unix)
- [TCP Loopback vs UDS Performance](https://copyprogramming.com/howto/tcp-loopback-connection-vs-unix-domain-socket-performance)
- [hyperlocal - Rust](https://docs.rs/hyperlocal/latest/hyperlocal/)
- [Axum Unix Socket Example](https://github.com/tokio-rs/axum/blob/main/examples/unix-domain-socket/src/main.rs)
- [Linux Network Performance Parameters](https://github.com/leandromoreira/linux-network-performance-parameters)
- [Cloudflare TCP Optimization](https://blog.cloudflare.com/optimizing-tcp-for-high-throughput-and-low-latency/)