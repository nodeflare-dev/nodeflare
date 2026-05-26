# コンテナランタイム層の最適化

MCP専用ベアメタルサーバーにおけるコンテナランタイム層の最適化設定。

## 前提条件

- **全言語対応**: Python, Node.js, Go, Rust等のMCPサーバーが動作すること
- **nodeflareのコンセプトを維持**: 特定言語への最適化で他を犠牲にしない

---

## 1. コンテナエンジン

### 選択: containerd 直接使用

| 選択肢 | オーバーヘッド | 採用 |
|-------|--------------|------|
| Docker (dockerd + containerd) | 600-800ms | ❌ |
| **containerd 直接** | ベースライン | ✅ |
| Podman | 38%高速 | △ 検討可 |

**理由**:
- Docker daemonのREST APIオーバーヘッドを排除
- メモリ使用量 73MB 削減 (dockerd分)
- 言語無関係、全MCPサーバーで恩恵

**設定**:
```bash
# containerd インストール (Debian)
apt install containerd

# Docker CLI互換が必要な場合
apt install nerdctl
```

---

## 2. OCI Runtime

### 選択: crun

| Runtime | 言語 | 起動速度 | 安定性 | 採用 |
|---------|-----|---------|-------|------|
| runc | Go | ベースライン | ◎ | ❌ |
| **crun** | C | **21-49%高速** | ◎ | ✅ |
| youki | Rust | runc〜crunの間 | △ (3.6%エラー率) | ❌ |

**理由**:
- Goのガベージコレクションオーバーヘッドを排除
- OCI標準準拠、コンテナ内の言語に無関係
- 本番環境での安定性も確認済み

**設定**:
```bash
# crun インストール
apt install crun

# /etc/containerd/config.toml
[plugins."io.containerd.grpc.v1.cri".containerd]
  default_runtime_name = "crun"

[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.crun]
  runtime_type = "io.containerd.runc.v2"
  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.crun.options]
    BinaryName = "/usr/bin/crun"
```

---

## 3. ネットワークモード

### 選択: 専用bridgeネットワーク（マルチテナント隔離）

| モード | レイテンシ | スループット | 隔離 | 採用 |
|-------|-----------|-------------|------|------|
| host | ~0µs | 40Gbps | ❌ 危険 | ❌ |
| **bridge (専用)** | **+15-50µs** | **20-35Gbps** | **◎** | **✅** |
| none | N/A | N/A | ◎ | △ 外部API不可 |

**重要**: `--net=host`はマルチテナント環境では**使用禁止**。
- 全コンテナが同一ネットワーク名前空間を共有
- 他テナントのポートにアクセス可能
- ネットワークベースの攻撃が容易

### Unix Socketとネットワーク名前空間

**Unix Socketはネットワーク名前空間と無関係**。bridgeモードでも性能は維持される。

```
┌─────────────────────────────────────────────────────┐
│  ホスト                                              │
│  ┌─────────┐     Unix Socket      ┌───────────────┐│
│  │ Proxy   │ ←──────────────────→ │ Container     ││
│  │         │  /var/run/mcp/xxx/   │ (bridge mode) ││
│  └─────────┘                      └───────────────┘│
│       ↑                                  ↑         │
│  ファイルシステム経由              ネットワーク名前空間 │
│  （NW名前空間と無関係）            は分離されている    │
└─────────────────────────────────────────────────────┘
```

### ネットワーク設定

#### 外部API不要の場合（最も安全）

```bash
--network none
```

コンテナは完全にネットワーク隔離。Unix Socket経由の通信のみ許可。

#### 外部API必要の場合（OpenAI, GitHub等）

```bash
# 専用bridgeネットワーク作成（初回のみ）
nerdctl network create mcp-bridge

# コンテナ起動時
--network mcp-bridge
```

**egressのみ許可、テナント間通信は不可**（後述のiptables設定）。

### iptables設定（外部API許可時）

```bash
# /etc/iptables/rules.v4

# テナント間通信を禁止（同一bridge内）
-A FORWARD -i br-mcp -o br-mcp -j DROP

# 外向き通信は許可
-A FORWARD -i br-mcp -o eth0 -j ACCEPT
-A FORWARD -i eth0 -o br-mcp -m state --state RELATED,ESTABLISHED -j ACCEPT
```

### パフォーマンス影響

| 項目 | host | bridge | 差分 |
|-----|------|--------|------|
| TCP レイテンシ | ~0µs | +15-50µs | **Unix Socket使用で無関係** |
| スループット | 40Gbps | 20-35Gbps | **Unix Socket使用で無関係** |

**結論**: Unix Socketを使用するため、bridgeモードでも性能劣化なし。

---

## 4. セキュリティ機能（マルチテナント必須）

### 4.1 Capability削除

```bash
--cap-drop ALL
```

| 設定 | 効果 | 採用 |
|-----|------|------|
| デフォルト | 14個のcapability付与 | ❌ 危険 |
| **--cap-drop ALL** | **全capability削除** | **✅ 必須** |

**理由**: MCPサーバーはroot権限不要。全capabilityを削除してコンテナ脱獄リスクを最小化。

### 4.2 読み取り専用ファイルシステム

```bash
--read-only
```

| 設定 | 効果 | 採用 |
|-----|------|------|
| デフォルト | コンテナ内書き込み可能 | ❌ |
| **--read-only** | **ファイルシステム書き込み禁止** | **✅ 必須** |

**注意**: 一時ファイルが必要な場合は tmpfs をマウント:
```bash
--read-only --tmpfs /tmp:size=64M,mode=1777
```

### 4.3 seccomp

| 設定 | オーバーヘッド | 全言語対応 | 採用 |
|-----|--------------|----------|------|
| unconfined (無効) | 0% | ✅ | ❌ (セキュリティリスク) |
| カスタムプロファイル | 最小 | ❌ (言語依存) | ❌ |
| **RuntimeDefault** | 2-3% | ✅ | ✅ |

**理由**:
- 言語ごとに必要なsyscallが異なる
  - Python: インタプリタが多くのsyscall使用
  - Node.js: V8 + libuv 固有のsyscall
  - Go: goroutine用のclone, futex等
  - Rust: syscall直接呼び出し可能
- カスタムプロファイルは一部言語で動作しない危険性
- RuntimeDefaultで全言語をカバー

### 4.4 AppArmor

| 設定 | オーバーヘッド | 採用 |
|-----|--------------|------|
| **docker-default** | 2-3% | ✅ |
| カスタム | 最小 | △ 検討可 |
| 無効 | 0% | ❌ |

### 4.5 リソース上限（必須）

**1テナントの暴走で全テナント巻き添えを防止**。

```bash
--memory 512m      # メモリ上限
--cpus 0.5         # CPU上限（0.5コア）
--pids-limit 256   # プロセス数上限（fork bomb対策）
```

| 設定 | 推奨値 | 理由 |
|-----|-------|------|
| `--memory` | 256m-1g | 言語・ワークロード依存 |
| `--cpus` | 0.25-1.0 | MCP は CPU light |
| `--pids-limit` | 128-512 | fork bomb 対策 |

**言語別推奨値**:

| 言語 | memory | cpus | pids-limit |
|-----|--------|------|------------|
| Python | 512m | 0.5 | 256 |
| Node.js | 256m | 0.5 | 256 |
| Go | 128m | 0.25 | 128 |
| Rust | 128m | 0.25 | 128 |

### セキュリティ設定まとめ

```bash
# 全オプション
--cap-drop ALL \
--read-only \
--tmpfs /tmp:size=64M,mode=1777 \
--security-opt seccomp=unconfined \  # または RuntimeDefault
--memory 512m \
--cpus 0.5 \
--pids-limit 256
```

---

## 5. ストレージドライバ

### 選択: overlay2

| ドライバ | 特徴 | 採用 |
|---------|------|------|
| **overlay2** | ファイル単位CoW、ページキャッシュ共有 | ✅ |
| btrfs/zfs | ブロック単位CoW | ❌ |

**理由**:
- MCPサーバーは読み込み中心のワークロード
- ページキャッシュ共有で同一ベースイメージのMCPがメモリ効率化
- 言語無関係

**注**: XFS vs ext4 は MCP ワークロードで有意差が確認されていないため、Debianデフォルトの ext4 を推奨（OS層を参照）。

**設定**:
```bash
# /etc/fstab (ext4 + noatime)
/dev/sda1 /var/lib/containerd ext4 defaults,noatime 0 0
```

**注**: snapshotterの設定は次セクション（Nydus）で行う。Nydusを使用しない場合のみ`overlayfs`を設定。

---

## 6. イメージ配信 (Lazy Pull)

### 選択: Nydus

| 方式 | pull時間 | 初回起動 | 採用 |
|-----|---------|---------|------|
| 通常pull | 全レイヤーDL後 | 遅い | ❌ |
| **Nydus** | 必要部分のみ | **即起動** | ✅ |

**効果（言語別）**:

| 言語 | 典型サイズ | 効果 |
|-----|-----------|------|
| Python + deps | 500MB〜1GB | ◎ 効果大 |
| Node.js + node_modules | 300MB〜800MB | ◎ 効果大 |
| Go (static binary) | 10〜50MB | △ 効果小 |
| Rust (static binary) | 5〜30MB | △ 効果小 |

**理由**:
- 「pullはコンテナ起動時間の76%を占める」(Harter et al.)
- Python/Node.jsの大きなイメージで特に効果的
- Go/Rustでは効果薄いが、害もない
- nodeflareの主要ユースケース（Python/Node.js MCP）で恩恵大

**設定**:
```bash
# Nydus snapshotter インストール
# https://github.com/containerd/nydus-snapshotter

# /etc/containerd/config.toml
[proxy_plugins]
  [proxy_plugins.nydus]
    type = "snapshot"
    address = "/run/containerd-nydus/containerd-nydus-grpc.sock"
```

**Builder側**: イメージをNydus形式に変換
```bash
# nydusify でイメージ変換
nydusify convert \
  --source python:3.11-slim \
  --target registry.example.com/python:3.11-slim-nydus
```

### レジストリ可用性対策

**問題**: Nydus lazy pullはレジストリに依存 → レジストリ障害で新規起動・スケールが停止。

#### 対策1: ローカルキャッシュ (推奨)

```bash
# nydus-snapshotter のキャッシュ設定
# /etc/nydus/config.json
{
  "device": {
    "cache": {
      "type": "blobcache",
      "config": {
        "work_dir": "/var/lib/nydus/cache",
        "cache_size": "50GB"
      }
    }
  }
}
```

一度pullしたチャンクはローカルにキャッシュ。同一イメージの再起動はレジストリ不要。

#### 対策2: ローカルレジストリミラー

```bash
# Harbor または distribution をローカルで稼働
nerdctl run -d \
  --name registry \
  -p 5000:5000 \
  -v /var/lib/registry:/var/lib/registry \
  registry:2

# プルスルーキャッシュ設定
# /etc/docker/registry/config.yml
proxy:
  remoteurl: https://registry.example.com
```

#### 対策3: フォールバック（通常イメージ）

```rust
// Builder側: Nydusイメージが利用不可ならフォールバック
async fn pull_image(server: &Server) -> Result<String> {
    let nydus_image = format!("{}:nydus", server.image_url);

    // Nydusイメージを試行
    if try_pull(&nydus_image).await.is_ok() {
        return Ok(nydus_image);
    }

    // フォールバック: 通常イメージ（遅いが動作する）
    warn!("Nydus image unavailable, falling back to standard image");
    Ok(server.image_url.clone())
}
```

#### 推奨構成

| 対策 | コスト | 効果 | 採用 |
|-----|-------|------|------|
| ローカルキャッシュ | ストレージ50GB | ◎ 再起動高速 | ✅ 必須 |
| ローカルミラー | サーバーリソース | ◎ 完全冗長 | △ 大規模時 |
| フォールバック | コード変更 | ○ 最終手段 | ✅ 推奨 |

---

## 7. cgroups

### 選択: v2

| バージョン | 特徴 | 採用 |
|-----------|------|------|
| v1 | レガシー、tail latency高い | ❌ |
| **v2** | 統一階層、低レイテンシ | ✅ |

**理由**:
- 統一階層でリソース管理が効率的
- tail latency改善（研究で確認済み）
- Kubernetes等でもv2推奨
- 言語無関係

**確認**:
```bash
# cgroups v2 が有効か確認
mount | grep cgroup2
# cgroup2 on /sys/fs/cgroup type cgroup2 ...
```

**設定**:
```toml
# /etc/containerd/config.toml
[plugins."io.containerd.grpc.v1.cri"]
  systemd_cgroup = true
```

---

## 最終設定まとめ

### /etc/containerd/config.toml

```toml
version = 2

[plugins."io.containerd.grpc.v1.cri"]
  systemd_cgroup = true

[plugins."io.containerd.grpc.v1.cri".containerd]
  snapshotter = "nydus"
  default_runtime_name = "crun"

[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.crun]
  runtime_type = "io.containerd.runc.v2"
  [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.crun.options]
    BinaryName = "/usr/bin/crun"
    SystemdCgroup = true

[proxy_plugins]
  [proxy_plugins.nydus]
    type = "snapshot"
    address = "/run/containerd-nydus/containerd-nydus-grpc.sock"
```

### コンテナ起動コマンド例

```bash
# nerdctlを使用（Docker CLI互換、OS層で必須パッケージとして設定）
nerdctl run \
  -d \
  --name mcp-server-123 \
  # ネットワーク隔離（外部API不要なら none、必要なら mcp-bridge）
  --network none \
  # テナント専用ソケットディレクトリのみマウント
  -v /var/run/mcp/server-123:/var/run/mcp \
  # リソース上限
  --memory 512m \
  --cpus 0.5 \
  --pids-limit 256 \
  # 権限最小化
  --cap-drop ALL \
  --read-only \
  --tmpfs /tmp:size=64M,mode=1777 \
  # Nydus snapshotter
  --snapshotter nydus \
  # 環境変数
  -e MCP_SOCKET_PATH=/var/run/mcp/default.sock \
  # イメージ
  registry.example.com/python:3.11-slim-nydus \
  python /app/server.py
```

### Builder実装例

```rust
// crates/builder/src/container.rs

async fn start_mcp_container(server: &Server) -> Result<()> {
    let socket_dir = format!("/var/run/mcp/{}", server.id);

    // テナント専用ソケットディレクトリを作成
    std::fs::create_dir_all(&socket_dir)?;
    std::fs::set_permissions(&socket_dir, std::fs::Permissions::from_mode(0o770))?;

    let output = Command::new("nerdctl")
        .args(&[
            "run", "-d",
            "--name", &format!("mcp-{}", server.id),
            // ① ネットワーク隔離
            "--network", if server.needs_external_api { "mcp-bridge" } else { "none" },
            // ② テナント専用ソケットディレクトリのみマウント
            "-v", &format!("{}:/var/run/mcp", socket_dir),
            // ③ リソース上限
            "--memory", "512m",
            "--cpus", "0.5",
            "--pids-limit", "256",
            // ④ 権限最小化
            "--cap-drop", "ALL",
            "--read-only",
            "--tmpfs", "/tmp:size=64M,mode=1777",
            // 環境変数
            "-e", "MCP_SOCKET_PATH=/var/run/mcp/default.sock",
            // イメージ
            &server.image_url,
        ])
        .output()
        .await?;

    Ok(())
}
```

---

## 期待される効果

| 項目 | 改善 |
|-----|------|
| containerd直接 | 600-800ms 削減 |
| crun | 起動 20-50% 高速化 |
| Unix Socket (Network層) | レイテンシ 36% 削減 |
| Nydus (Python/Node.js) | 初回起動 76% 削減 |
| cgroups v2 | tail latency 改善 |
| マルチテナント隔離 | セキュリティ向上（性能影響なし） |

---

## 言語互換性

| 最適化 | Python | Node.js | Go | Rust |
|-------|--------|---------|-----|------|
| containerd | ✅ | ✅ | ✅ | ✅ |
| crun | ✅ | ✅ | ✅ | ✅ |
| network isolation (none/bridge) | ✅ | ✅ | ✅ | ✅ |
| --cap-drop ALL | ✅ | ✅ | ✅ | ✅ |
| --read-only | ✅ | ✅ | ✅ | ✅ |
| リソース上限 (memory/cpu/pids) | ✅ | ✅ | ✅ | ✅ |
| RuntimeDefault seccomp | ✅ | ✅ | ✅ | ✅ |
| overlay2 | ✅ | ✅ | ✅ | ✅ |
| Nydus | ◎ | ◎ | △ | △ |
| cgroups v2 | ✅ | ✅ | ✅ | ✅ |

**全言語で動作確認済み、マルチテナント隔離を維持しつつ性能を確保**

---

## 参考資料

- [crun vs runc vs youki](https://sumguy.com/crun-runc-youki-oci-runtimes/)
- [containerd vs dockerd benchmark](https://medium.com/norma-dev/benchmarking-containerd-vs-dockerd-performance-efficiency-and-scalability-64c9043924b1)
- [Docker network modes](https://eastondev.com/blog/en/posts/dev/20251217-docker-network-modes/)
- [seccomp profiles for production](https://www.systemshardening.com/articles/kubernetes/seccomp-profiles/)
- [Nydus lazy pulling](https://nydus.dev/)
- [cgroups v2 performance](https://www.net.in.tum.de/fileadmin/TUM/NET/NET-2023-11-1/NET-2023-11-1_03.pdf)
