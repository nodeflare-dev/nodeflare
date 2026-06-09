# OS層の最適化

MCP専用ベアメタルサーバーにおけるOS層の最適化設定。

## 前提条件

- **OS**: Debian 12 (Bookworm) 以降
- **カーネル**: Linux 5.15以上
- **全言語対応**: Python, Node.js, Go, Rust
- **glibcベース**: Alpine (musl) は Node.js 互換性問題のため不採用

---

## 1. 必須設定

### 1.1 ファイルディスクリプタ制限

Proxyは多数の同時接続を処理するため、デフォルト値（1024）では不足。

**ファイル: `/etc/sysctl.d/80-mcp.conf`**

```bash
# カーネル内最大オープンFD数
fs.file-max = 2097152
```

**ファイル: `/etc/security/limits.d/80-mcp.conf`**

```bash
# 全ユーザー
* soft nofile 131072
* hard nofile 262144

# root
root soft nofile 262144
root hard nofile 262144
```

**理由**: 1接続 = 1ファイルディスクリプタ。Proxy + MCPサーバーで数千〜数万の接続を想定。

### 1.2 TCPバックログ

**ファイル: `/etc/sysctl.d/80-mcp.conf`に追加**

```bash
# 接続キューのサイズ
net.core.somaxconn = 65535
```

**理由**: デフォルト（128〜4096）では高負荷時に接続拒否が発生する可能性。

### 1.3 スワップ抑制

**ファイル: `/etc/sysctl.d/80-mcp.conf`に追加**

```bash
# スワップ使用を最小化（MCPはレイテンシ敏感）
vm.swappiness = 10
```

**理由**: スワップ発生時のレイテンシは数ms〜数百ms。MCPリクエストには致命的。

---

## 2. 推奨設定

### 2.1 マウントオプション

**ファイル: `/etc/fstab`**

```bash
# ルートファイルシステム
/dev/nvme0n1p2 / ext4 defaults,noatime,errors=remount-ro 0 1

# containerdストレージ
/dev/nvme0n1p3 /var/lib/containerd ext4 defaults,noatime 0 0

# Unix Socketディレクトリ（tmpfs）
tmpfs /var/run/mcp tmpfs size=512M,mode=0775,nodev,nosuid,noexec 0 0
```

| オプション | 効果 | 理由 |
|-----------|------|------|
| `noatime` | アクセス時刻更新無効 | メタデータ書き込み削減、デメリットなし |
| `tmpfs` | メモリ上にファイルシステム | Unix Socketのディスク I/O ゼロ化 |

**注**: XFS vs ext4 は MCP ワークロードで有意差が確認されていないため、Debian デフォルトの ext4 を推奨。

### 2.2 不要サービス無効化

```bash
#!/bin/bash
# 明らかに不要なサービスのみ無効化

DISABLE_SERVICES=(
    "cups"           # 印刷
    "avahi-daemon"   # mDNS
    "bluetooth"      # Bluetooth
)

for service in "${DISABLE_SERVICES[@]}"; do
    systemctl disable --now "$service" 2>/dev/null || true
done
```

**維持するサービス**: sshd, systemd-journald, chrony/ntp, containerd

### 2.3 不要パッケージ・ファイル削除

MCP専用サーバーには不要なパッケージを削除してメモリ・ディスク使用量を削減。

> "This reduces your memory footprint to under 156MB and reduces your attack surface by over 90%"
> — [FOSS Linux](https://www.fosslinux.com/49956/install-debian-11-minimal-server.htm)

---

#### 2.3.1 APT設定: 推奨パッケージをインストールしない

**ファイル: `/etc/apt/apt.conf.d/01norecommend`**

```bash
APT::Install-Recommends "0";
APT::Install-Suggests "0";
```

今後のapt installで不要な推奨パッケージがインストールされなくなる。

---

#### 2.3.2 dpkg設定: man/docを最初からインストールしない

**ファイル: `/etc/dpkg/dpkg.cfg.d/01_nodoc`**

```bash
# manページをインストールしない
path-exclude=/usr/share/man/*

# ドキュメントをインストールしない（copyrightは残す）
path-exclude=/usr/share/doc/*
path-include=/usr/share/doc/*/copyright

# infoページをインストールしない
path-exclude=/usr/share/info/*

# 翻訳ファイルをインストールしない
path-exclude=/usr/share/locale/*/LC_MESSAGES/*.mo

# groffデータをインストールしない
path-exclude=/usr/share/groff/*

# lindaデータをインストールしない
path-exclude=/usr/share/linda/*

# lintianデータをインストールしない
path-exclude=/usr/share/lintian/*
```

> "These options set a glob-pattern as a path filter, either by excluding or re-including previously excluded paths"
> — [dpkg manual](https://manpages.ubuntu.com/manpages/trusty/man1/dpkg.1.html)

**注意**: 既存パッケージには適用されない。適用するには `apt reinstall $(dpkg -l | awk '/^ii/ {print $2}')` が必要。

---

#### 2.3.3 既存のman/doc/locale削除

```bash
#!/bin/bash
# 既存の不要ファイル削除

# manページ削除（50-100MB）
rm -rf /usr/share/man/*

# ドキュメント削除（100-200MB）
# copyrightは残す（ライセンス確認用）
find /usr/share/doc -mindepth 1 -maxdepth 1 -type d | while read dir; do
    find "$dir" -type f ! -name 'copyright' -delete
    find "$dir" -type d -empty -delete
done

# infoページ削除
rm -rf /usr/share/info/*

# 外国語manページ削除
rm -rf /usr/share/man/??
rm -rf /usr/share/man/??_*
```

---

#### 2.3.4 不要ロケール削除 (localepurge)

```bash
# インストール
apt install -y localepurge

# 設定: /etc/locale.nopurge に残すロケールを指定
cat > /etc/locale.nopurge << 'EOF'
MANDELETE
DONTBOTHERNEWLOCALE
en
en_US
en_US.UTF-8
EOF

# 実行（90%のロケールスペース削減）
localepurge
```

> "By limiting the number of locale files built you can save 90% of the space"
> — [Debian Wiki](https://wiki.debian.org/ReduceDebian)

---

#### 2.3.5 不要パッケージ削除

```bash
#!/bin/bash
# MCP専用サーバーで不要なパッケージを削除

# ドキュメント系
apt purge -y \
    man-db \
    manpages \
    manpages-dev \
    debian-faq \
    doc-debian \
    doc-linux-text \
    info

# エディタ（vim-tinyは緊急編集用に残す）
apt purge -y \
    nano \
    vim-common

# ネットワーク診断（デバッグ時のみ必要なら残す）
apt purge -y \
    traceroute \
    dnsutils \
    bind9-host \
    net-tools

# ハードウェア検出（ベアメタルでは初期設定後不要）
apt purge -y \
    laptop-detect \
    usbutils \
    pciutils \
    pci.ids

# 報告ツール
apt purge -y \
    reportbug \
    installation-report

# その他
apt purge -y \
    tasksel \
    tasksel-data \
    whiptail \
    at \
    acpi \
    acpid \
    eject \
    finger \
    mutt
```

---

#### 2.3.6 孤立パッケージ削除 (deborphan)

```bash
# インストール
apt install -y deborphan

# 孤立ライブラリを検出
deborphan

# 孤立パッケージを削除（繰り返し実行）
while [ -n "$(deborphan)" ]; do
    deborphan | xargs apt purge -y
done

# 全カテゴリで孤立パッケージを検出
deborphan --guess-all

# 自動インストールされた不要パッケージも削除
apt autoremove -y
```

> "deborphan takes a different approach - it analyzes what packages are installed and finds any that have no other packages depending on them"
> — [OneUptime](https://oneuptime.com/blog/post/2026-03-02/remove-orphaned-packages-ubuntu-deborphan/view)

---

#### 2.3.7 未使用カーネルモジュールのブラックリスト

```bash
# 現在ロードされているモジュールを確認
lsmod

# 不要なモジュールをブラックリスト
# ファイル: /etc/modprobe.d/blacklist-unused.conf

# Bluetooth（サーバーでは不要）
blacklist bluetooth
blacklist btusb

# サウンド（サーバーでは不要）
blacklist snd
blacklist soundcore
blacklist snd_hda_intel

# Webcam
blacklist uvcvideo

# Floppy（存在しない）
blacklist floppy

# initramfs更新
update-initramfs -u
```

**自動化ツール: ModuleJail**

```bash
# ModuleJailで未使用モジュールを自動ブラックリスト
# https://github.com/modulejail/modulejail
wget https://github.com/modulejail/modulejail/releases/latest/download/modulejail
chmod +x modulejail
./modulejail --apply
```

> "ModuleJail scans currently loaded modules, compares them to the full module tree, and creates a modprobe.d blacklist for unused modules"
> — [Linuxiac](https://linuxiac.com/modulejail-blocks-unused-linux-kernel-modules-to-limit-attack-surface/)

---

#### 2.3.8 古いカーネル削除

```bash
# インストール済みカーネル一覧
dpkg -l | grep linux-image

# 現在使用中のカーネル
uname -r

# 古いカーネルを削除（400-500MB/個）
apt purge -y $(dpkg -l | grep linux-image | grep -v $(uname -r) | awk '{print $2}')

# ヘッダーも削除
apt purge -y $(dpkg -l | grep linux-headers | grep -v $(uname -r | sed 's/-generic//') | awk '{print $2}')
```

---

#### 2.3.9 systemd ジャーナルサイズ制限

```bash
# ファイル: /etc/systemd/journald.conf.d/size.conf
[Journal]
SystemMaxUse=100M
RuntimeMaxUse=50M
```

```bash
# 即時適用
systemctl restart systemd-journald

# 既存ログを削減
journalctl --vacuum-size=100M
```

---

#### 2.3.10 APTキャッシュクリア

```bash
# ダウンロード済みパッケージ削除
apt clean

# パッケージリスト削除（apt updateで再取得可能）
rm -rf /var/lib/apt/lists/*
```

---

### 削除対象と効果まとめ

| カテゴリ | 削除対象 | 削減量 |
|---------|---------|-------|
| manページ | /usr/share/man | 50-100MB |
| ドキュメント | /usr/share/doc | 100-200MB |
| ロケール | localepurge | **100-300MB (90%削減)** |
| 古いカーネル | linux-image-* | **400-500MB/個** |
| 不要パッケージ | 上記リスト | 50-100MB |
| APTキャッシュ | /var/lib/apt/lists | 50-100MB |
| ジャーナル | /var/log/journal | 可変 → 100MB制限 |
| **合計** | - | **800MB-1.5GB+** |

---

### 維持するパッケージ

```bash
# 必須（削除禁止）
openssh-server    # リモートアクセス
systemd           # init
ca-certificates   # TLS証明書
curl              # デバッグ・ヘルスチェック
git               # Builder が clone_repo() で使用（必須）
containerd        # コンテナランタイム
crun              # OCIランタイム
nerdctl           # containerd CLI（Network層で使用）
iptables          # ファイアウォール
iproute2          # ip コマンド（ネットワーク設定に必須）
vim-tiny          # 緊急時の設定編集用（最小エディタ）
```

**注意**:
- `git` は Builder の `clone_repo()` 関数で使用されているため、削除するとビルドが失敗する
- `nerdctl` は Network層のコンテナ起動で使用される

---

### 最小インストール（新規構築時）

Hetznerなどで新規サーバー構築時は最小構成を選択:

```bash
# Debian netinstインストール時
# - 全てのソフトウェア選択を解除
# - "Standard system utilities" のみ選択
# - デスクトップ環境は選択しない

# インストール後に必要なものだけ追加
apt update
apt install -y \
    openssh-server \
    containerd \
    crun \
    nerdctl \
    ca-certificates \
    curl \
    git \
    iptables \
    iproute2 \
    vim-tiny
```

---

### 完全セットアップスクリプト

```bash
#!/bin/bash
# MCP専用サーバー最小化スクリプト
set -e

echo "=== APT設定 ==="
cat > /etc/apt/apt.conf.d/01norecommend << 'EOF'
APT::Install-Recommends "0";
APT::Install-Suggests "0";
EOF

echo "=== dpkg path-exclude設定 ==="
cat > /etc/dpkg/dpkg.cfg.d/01_nodoc << 'EOF'
path-exclude=/usr/share/man/*
path-exclude=/usr/share/doc/*
path-include=/usr/share/doc/*/copyright
path-exclude=/usr/share/info/*
path-exclude=/usr/share/locale/*/LC_MESSAGES/*.mo
path-exclude=/usr/share/groff/*
path-exclude=/usr/share/linda/*
path-exclude=/usr/share/lintian/*
EOF

echo "=== 既存man/doc削除 ==="
rm -rf /usr/share/man/* /usr/share/info/*
find /usr/share/doc -mindepth 1 -maxdepth 1 -type d | while read dir; do
    find "$dir" -type f ! -name 'copyright' -delete 2>/dev/null || true
done

echo "=== localepurge設定 ==="
apt install -y localepurge
cat > /etc/locale.nopurge << 'EOF'
MANDELETE
DONTBOTHERNEWLOCALE
en
en_US
en_US.UTF-8
EOF
localepurge

echo "=== 不要パッケージ削除 ==="
# 注意: git, vim-tiny, iproute2 は削除しない（nodeflare必須）
apt purge -y man-db manpages manpages-dev debian-faq doc-debian info \
    nano vim-common traceroute dnsutils bind9-host net-tools \
    laptop-detect usbutils pciutils reportbug installation-report \
    tasksel tasksel-data whiptail at 2>/dev/null || true

echo "=== 孤立パッケージ削除 ==="
apt install -y deborphan
while [ -n "$(deborphan 2>/dev/null)" ]; do
    deborphan | xargs apt purge -y 2>/dev/null || break
done
apt autoremove -y

echo "=== 古いカーネル削除 ==="
CURRENT_KERNEL=$(uname -r)
dpkg -l | grep linux-image | grep -v "$CURRENT_KERNEL" | awk '{print $2}' | xargs apt purge -y 2>/dev/null || true

echo "=== ジャーナルサイズ制限 ==="
mkdir -p /etc/systemd/journald.conf.d
cat > /etc/systemd/journald.conf.d/size.conf << 'EOF'
[Journal]
SystemMaxUse=100M
RuntimeMaxUse=50M
EOF
systemctl restart systemd-journald
journalctl --vacuum-size=100M

echo "=== APTキャッシュクリア ==="
apt clean
rm -rf /var/lib/apt/lists/*

echo "=== 完了 ==="
df -h /
```

---

## 3. 最終設定ファイル

### /etc/sysctl.d/80-mcp.conf

```bash
# ============================================
# MCP Server 最小限の最適化
# ============================================

# ファイルディスクリプタ
fs.file-max = 2097152

# TCPバックログ
net.core.somaxconn = 65535

# スワップ抑制
vm.swappiness = 10
```

### 適用

```bash
# sysctl適用
sudo sysctl --system

# 確認
sysctl fs.file-max net.core.somaxconn vm.swappiness
```

---

## 4. 検証

```bash
#!/bin/bash
echo "=== OS設定確認 ==="

# ファイルディスクリプタ
echo -n "fs.file-max: "
sysctl -n fs.file-max

# ulimit
echo -n "ulimit -n: "
ulimit -n

# somaxconn
echo -n "net.core.somaxconn: "
sysctl -n net.core.somaxconn

# swappiness
echo -n "vm.swappiness: "
sysctl -n vm.swappiness

# tmpfs確認
echo -n "/var/run/mcp: "
mount | grep /var/run/mcp || echo "NOT MOUNTED"
```

---

## 5. 採用しなかった設定

以下は効果が不明確または MCP ワークロードに関係ないため採用しない:

| 設定 | 理由 |
|-----|------|
| XFS ファイルシステム | ext4 との差が MCP で確認されていない |
| I/O スケジューラ変更 | MCP はディスク I/O heavy ではない |
| `vm.dirty_*` 設定 | ディスク書き込みがほぼない |
| `vm.overcommit_memory` | cgroup で制限済み |
| `kernel.threads-max` 変更 | デフォルトで十分 |
| カーネルセキュリティ設定 | セキュリティであってパフォーマンスではない |
| リアルタイムカーネル | 完全に不要 |

---

## 6. 言語互換性

| 設定 | Python | Node.js | Go | Rust |
|-----|--------|---------|-----|------|
| fs.file-max | ✅ | ✅ | ✅ | ✅ |
| ulimit | ✅ | ✅ | ✅ | ✅ |
| somaxconn | ✅ | ✅ | ✅ | ✅ |
| swappiness | ✅ | ✅ | ✅ | ✅ |
| noatime | ✅ | ✅ | ✅ | ✅ |
| tmpfs | ✅ | ✅ | ✅ | ✅ |
| パッケージ削減 | ✅ | ✅ | ✅ | ✅ |
| ロケール削減 | ✅ | ✅ | ✅ | ✅ |
| カーネルモジュール最小化 | ✅ | ✅ | ✅ | ✅ |

**全言語で対応可能。OS層の最適化はアプリケーション言語に依存しない。**

---

## 7. 参考資料

- [Debian Wiki: ReduceDebian](https://wiki.debian.org/ReduceDebian) - 公式のDebian削減ガイド
- [dpkg manual: path-exclude](https://manpages.ubuntu.com/manpages/trusty/man1/dpkg.1.html) - dpkgのパス除外設定
- [localepurge manpage](https://manpages.ubuntu.com/manpages/bionic/man8/localepurge.8.html) - ロケール削除ツール
- [Save disk space by excluding useless files with dpkg](https://raphaelhertzog.com/2010/11/15/save-disk-space-by-excluding-useless-files-with-dpkg/)
- [Debian Kernel Module Blacklisting](https://wiki.debian.org/KernelModuleBlacklisting)
- [ModuleJail](https://linuxiac.com/modulejail-blocks-unused-linux-kernel-modules-to-limit-attack-surface/) - 未使用カーネルモジュール自動ブラックリスト
- [deborphan: Orphaned package finder](https://manpages.ubuntu.com/manpages/bionic/man1/deborphan.1.html)
- [FOSS Linux: Minimal Debian Server Installation](https://www.fosslinux.com/49956/install-debian-11-minimal-server.htm)