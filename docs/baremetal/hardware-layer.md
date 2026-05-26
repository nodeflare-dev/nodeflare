# Hardware層の最適化

MCP専用ベアメタルサーバーにおけるハードウェア選定。

## 前提条件

- **全言語対応**: Python, Node.js, Go, Rust等のMCPサーバーが動作すること
- **コスト効率**: Fly.io比でコスト削減が目的
- **スケーラビリティ**: 同時稼働MCPサーバー数に応じた構成

---

## 1. MCPワークロードの特性

| 特性 | MCP | 影響するリソース |
|-----|-----|----------------|
| 処理内容 | JSON-RPC (数KB) | CPU: 低負荷 |
| 同時接続数 | 数百〜数千 | メモリ: 高需要 |
| コンテナ起動 | Nydus lazy pull | ストレージ: IOPS重要 |
| 外部通信 | TLS終端 | ネットワーク: 帯域重要 |

**結論**: MCPはCPUバウンドではなく、**メモリとストレージIOPSが重要**。

---

## 2. CPU

### 要件

| 用途 | CPU要件 |
|-----|--------|
| JSON-RPC処理 | 低 (I/Oバウンド) |
| コンテナ起動 | 中 (イメージ展開時) |
| Python/Node.js実行 | 中 (インタプリタ) |
| Go/Rust実行 | 低 (ネイティブバイナリ) |

### 推奨

| 規模 | 同時MCP数 | CPU |
|-----|----------|-----|
| 小規模 | 〜50 | 8コア |
| 中規模 | 50-200 | 16コア |
| 大規模 | 200+ | 32コア |

**AMD EPYC推奨**: Hetznerのベンチマークで Intel Xeon より高性能、かつ低価格。

---

## 3. メモリ

### コンテナあたりのメモリ使用量

| コンポーネント | メモリ |
|--------------|-------|
| コンテナオーバーヘッド | 50-300MB |
| Python MCP (典型) | 100-500MB |
| Node.js MCP (典型) | 50-200MB |
| Go/Rust MCP (典型) | 10-50MB |

### 計算式

```
必要メモリ = システム予約 + (コンテナ数 × 平均メモリ) × 1.3 (バッファ)
```

### 推奨構成

| 規模 | 同時MCP数 | 必要メモリ | 推奨 |
|-----|----------|----------|------|
| 小規模 | 〜50 | 20-30GB | **64GB** |
| 中規模 | 50-200 | 50-100GB | **128GB** |
| 大規模 | 200+ | 100-200GB | **256GB** |

**ECC推奨**: 長時間稼働サーバーでは単ビットエラー検出・訂正が重要。

---

## 4. ストレージ

### IOPS比較

| ストレージ | ランダムRead IOPS | コンテナイメージpull |
|-----------|-----------------|-------------------|
| HDD | 100 | 60秒以上 |
| SATA SSD | 80,000 | 数秒 |
| **NVMe SSD** | **500,000+** | **数秒 (5000x高速)** |

> "Docker containers and microservices generate significant random I/O during image pulls, layer extraction, log writes, and volume mounts."
> — [MassiveGRID](https://massivegrid.com/blog/nvme-vs-ssd-vs-hdd-vps-performance-benchmarks/)

### Nydusとの相性

Nydus lazy pullはオンデマンドでチャンクを読み込むため、ランダムIOPSが重要。

| ストレージ | Nydus効果 |
|-----------|----------|
| HDD | △ ランダムI/Oがボトルネック |
| SATA SSD | ○ 効果あり |
| **NVMe** | **◎ 最大効果** |

### 推奨構成

| 用途 | ストレージ | 容量 |
|-----|----------|------|
| OS + containerd | NVMe | 500GB |
| コンテナイメージ | NVMe | 1TB+ |
| ログ | SATA SSD/HDD | 500GB |

**RAID不要**: コンテナは揮発的、イメージはレジストリから再取得可能。単一NVMeで十分。

---

## 5. ネットワーク

### 要件

| 用途 | 帯域要件 |
|-----|---------|
| 外部接続 (Client→Proxy) | 1Gbps+ |
| イメージpull | バースト時に高帯域 |
| 内部通信 (Unix Socket) | N/A |

### 推奨

| 規模 | 帯域 |
|-----|------|
| 小〜中規模 | **1Gbps** (標準) |
| 大規模 | **10Gbps** (オプション) |

**無制限トラフィック推奨**: Hetzner/OVHは無制限または大容量トラフィック込み。

---

## 6. プロバイダー比較

### 推奨: Hetzner

| 項目 | Hetzner | OVH | Vultr |
|-----|---------|-----|-------|
| 価格 | ◎ 最安 | ○ | △ |
| 性能 | ◎ AMD EPYC | ○ | ○ |
| 拠点 | ドイツ, フィンランド, US, シンガポール | ◎ グローバル | ○ |
| 帯域 | 無制限 (20TB/月 fair use) | 無制限 | 従量制 |
| DDoS保護 | 標準 | ◎ 強力 | ○ |

**選定理由**:
- AMD EPYCプロセッサで高性能
- NVMeドライブが標準
- 価格が最も競争力あり
- Server Auctionでさらに安価な構成が入手可能

> "Hetzner offers better performance at lower prices with a cleaner experience."
> — [GetDeploying](https://getdeploying.com/hetzner-vs-ovh)

### Server Auction

Hetznerの[Server Auction](https://www.hetzner.com/sb/)では、カスタマイズ済み中古サーバーが割安で入手可能。

[Server Radar](https://radar.iodev.org/)で価格監視・アラート設定が可能。

---

## 7. 推奨構成

### 小規模 (スタートアップ)

| 項目 | スペック | 参考価格 |
|-----|---------|---------|
| サーバー | Hetzner AX41-NVMe | €44/月 |
| CPU | AMD Ryzen 5 3600 (6コア) | - |
| メモリ | 64GB DDR4 ECC | - |
| ストレージ | 512GB NVMe | - |
| 帯域 | 1Gbps 無制限 | - |
| **同時MCP** | **〜50** | - |

### 中規模 (成長期)

| 項目 | スペック | 参考価格 |
|-----|---------|---------|
| サーバー | Hetzner AX52 | €75/月 |
| CPU | AMD Ryzen 7 5800X (8コア) | - |
| メモリ | 128GB DDR4 ECC | - |
| ストレージ | 1TB NVMe | - |
| 帯域 | 1Gbps 無制限 | - |
| **同時MCP** | **50-150** | - |

### 大規模 (本番)

| 項目 | スペック | 参考価格 |
|-----|---------|---------|
| サーバー | Hetzner AX102 | €130/月 |
| CPU | AMD EPYC 7443P (24コア) | - |
| メモリ | 256GB DDR4 ECC | - |
| ストレージ | 2x 1TB NVMe | - |
| 帯域 | 1Gbps 無制限 (10Gbpsオプション) | - |
| **同時MCP** | **200+** | - |

---

## 8. Fly.ioとのコスト比較

### 現状 (Fly.io)

| 項目 | Fly.io |
|-----|--------|
| 256MB VM | $1.94/月 |
| 100 MCP servers | $194/月 |
| 500 MCP servers | $970/月 |

### ベアメタル (Hetzner)

| 構成 | 月額 | 同時MCP | コスト/MCP |
|-----|------|--------|----------|
| AX41 (64GB) | €44 | 〜50 | €0.88 |
| AX52 (128GB) | €75 | 〜150 | €0.50 |
| AX102 (256GB) | €130 | 〜300 | €0.43 |

### 削減効果

| 規模 | Fly.io | Hetzner | 削減率 |
|-----|--------|---------|-------|
| 50 MCP | $97/月 | €44/月 | **55%削減** |
| 150 MCP | $291/月 | €75/月 | **74%削減** |
| 300 MCP | $582/月 | €130/月 | **78%削減** |

**注**: 上記は概算。実際のコストは使用パターン、リージョン、為替レートにより変動。

---

## 9. 冗長化

### 単一サーバー構成 (推奨スタート)

MCPサーバーはステートレスであり、障害時はコンテナを再起動するだけで復旧可能。

```
[Client] → [Caddy] → [Proxy] → [MCP Containers]
              └── 単一サーバー ──┘
```

### 複数サーバー構成 (将来)

スケールアウト時:

```
                    ┌─────────────────┐
[Client] → [LB] → │ Server 1        │
                    │ - Proxy         │
                    │ - MCP 1-100     │
                    ├─────────────────┤
                    │ Server 2        │
                    │ - Proxy         │
                    │ - MCP 101-200   │
                    └─────────────────┘
```

**将来検討**: Consul/etcdによるサービスディスカバリ、複数サーバー間のロードバランシング。

---

## 10. チェックリスト

### 発注前

- [ ] 同時稼働MCP数の見積もり
- [ ] 平均メモリ使用量の測定
- [ ] リージョン選定 (ユーザー近接)

### セットアップ時

- [ ] Debian 12インストール
- [ ] NVMeパーティション設定 (OS + containerd分離)
- [ ] ECC有効確認
- [ ] ネットワーク帯域テスト

### 運用時

- [ ] メモリ使用率監視 (80%アラート)
- [ ] ストレージIOPS監視
- [ ] コンテナ数監視

---

## 11. 状態管理

MCPサーバーコンテナはステートレスだが、**nodeflare自体はステートフル**。

### 状態を持つコンポーネント

| コンポーネント | 状態 | 重要度 |
|--------------|------|-------|
| PostgreSQL | ユーザー、サーバー設定、認証情報 | **最重要** |
| OAuth トークン | 暗号化されたアクセストークン | 重要 |
| Audit ログ | 操作履歴 | 中 |
| コンテナイメージ | レジストリにキャッシュ | 低（再取得可能） |

### データベース構成

#### 選択肢

| 構成 | コスト | 可用性 | 推奨 |
|-----|-------|-------|------|
| **Supabase継続** | 無料〜$25/月 | ◎ マネージド | ✅ 小〜中規模 |
| 自前PostgreSQL (同一サーバー) | €0 | △ 単一障害点 | △ |
| 自前PostgreSQL (別サーバー) | €10-20/月 | ○ | △ |
| SQLite (単一サーバー時) | €0 | △ 単一障害点 | △ テスト用 |

#### 推奨: Supabase継続

ベアメタル移行後もSupabaseを継続使用:
- マネージドで高可用性
- 自動バックアップ
- 接続プーリング (pgbouncer)
- 追加インフラ不要

```
┌─────────────────────┐        ┌─────────────────────┐
│  Hetzner Server     │        │  Supabase           │
│  ┌───────────────┐  │        │  ┌───────────────┐  │
│  │ Proxy         │──┼────────┼─→│ PostgreSQL    │  │
│  │ Builder       │  │  TLS   │  │ (managed)     │  │
│  └───────────────┘  │        │  └───────────────┘  │
│  ┌───────────────┐  │        └─────────────────────┘
│  │ MCP Containers│  │
│  └───────────────┘  │
└─────────────────────┘
```

### バックアップ戦略

#### Supabase使用時

Supabaseの自動バックアップを利用（Pro以上で日次バックアップ）。

#### 自前PostgreSQL使用時

```bash
# /etc/cron.d/pg-backup
0 */6 * * * postgres pg_dump nodeflare | gzip > /backup/nodeflare-$(date +\%Y\%m\%d\%H).sql.gz

# 古いバックアップを削除（7日保持）
0 1 * * * root find /backup -name "nodeflare-*.sql.gz" -mtime +7 -delete
```

### シークレット管理

| シークレット | 保存場所 | 暗号化 |
|------------|---------|-------|
| DB接続情報 | 環境変数 | - |
| OAuthトークン | PostgreSQL | AES-256-GCM |
| APIキー | PostgreSQL | AES-256-GCM |

```bash
# /etc/systemd/system/nodeflare-proxy.service
[Service]
EnvironmentFile=/etc/nodeflare/secrets.env
```

```bash
# /etc/nodeflare/secrets.env (chmod 600)
DATABASE_URL=postgres://...
ENCRYPTION_KEY=...
```

---

## 12. Observability

### 監視スタック

| コンポーネント | 用途 | 推奨 |
|--------------|------|------|
| **Prometheus** | メトリクス収集 | ✅ |
| **Grafana** | ダッシュボード | ✅ |
| **Loki** | ログ集約 | ○ |
| node_exporter | ホストメトリクス | ✅ |
| cadvisor | コンテナメトリクス | ✅ |

### 最小構成

```bash
# node_exporter インストール
apt install prometheus-node-exporter

# cadvisor インストール
nerdctl run -d \
  --name cadvisor \
  -v /:/rootfs:ro \
  -v /var/run:/var/run:ro \
  -v /sys:/sys:ro \
  -v /var/lib/containerd:/var/lib/containerd:ro \
  -p 8080:8080 \
  gcr.io/cadvisor/cadvisor:latest
```

### 監視対象メトリクス

| メトリクス | 閾値 | アラート |
|-----------|------|---------|
| `node_memory_MemAvailable_bytes` | <20% | Critical |
| `node_filesystem_avail_bytes` | <10GB | Warning |
| `container_memory_usage_bytes` | >limit*0.9 | Warning |
| `container_cpu_usage_seconds_total` | >limit*0.9 | Warning |
| `container_last_seen` | >5min ago | Critical |

### Grafanaダッシュボード

```bash
# Grafana インストール
apt install grafana

# 起動
systemctl enable --now grafana-server

# http://localhost:3000 でアクセス
# デフォルト: admin/admin
```

推奨ダッシュボード:
- Node Exporter Full (ID: 1860)
- cAdvisor (ID: 14282)

### ログ管理

```bash
# journald でログを確認
journalctl -u nodeflare-proxy -f

# コンテナログ
nerdctl logs mcp-server-xxx

# Loki使用時（オプション）
# 全コンテナログを Loki に送信
```

---

## 参考資料

- [Hetzner Dedicated Servers](https://www.hetzner.com/dedicated-rootserver/matrix-ax/)
- [Hetzner Server Auction](https://www.hetzner.com/sb/)
- [Server Radar - Price Tracker](https://radar.iodev.org/)
- [NVMe vs SSD vs HDD Performance](https://massivegrid.com/blog/nvme-vs-ssd-vs-hdd-vps-performance-benchmarks/)
- [Hetzner vs OVH Comparison](https://getdeploying.com/hetzner-vs-ovh)
- [Container Memory Overhead](https://www.baeldung.com/ops/docker-container-perf-cost)
- [Prometheus + Grafana Setup](https://prometheus.io/docs/visualization/grafana/)
- [cAdvisor Container Metrics](https://github.com/google/cadvisor)
