# CR-Bridge

**クロスリアリティ低レイヤー基盤ミドルウェア**  
*Cross-Reality Low-Layer Infrastructure Middleware*

> "VRChatの友達があなたの隣に立つ" ── そのために、OSカーネルの内側から書き直す

---

## 概要

CR-Bridge は、VRChat・Twitter/X・Instagram 等の既存インターネットサービスと、ARグラス・映像/音声出力等の現実インタフェースをリアルタイムに接続する **Linuxシステムデーモン** です。

eBPF・拡張カルマンフィルタ (EKF)・AVX-512 FMA・ゼロコピーmmap を組み合わせ、「**テレポート**」をアーキテクチャレベルで禁止します。

## アーキテクチャ

```
┌─────────────────────────────────────────────────────┐
│         アプリケーション層                             │
│   VRChat ／ ARグラス描画 ／ SNSブラウザ               │
└──────────────────┬──────────────────────────────────┘
                   │ mmap read（ゼロコピー）
┌──────────────────▼──────────────────────────────────┐
│     Shared Memory Spatial Ledger (SMSL)              │ ← ユーザーランド
│     SeqLock + S2インデックス + FlatBuffers            │
└──────────────────┬──────────────────────────────────┘
                   │ 書き込み
┌──────────────────▼──────────────────────────────────┐
│     ATPエンジン（Rustデーモン）                        │ ← ユーザーランド
│     EKF（AVX-512 FMA） + DeadReckoning + Hermite補間  │
└──────────────────┬──────────────────────────────────┘
                   │ AF_XDP / perf_event ringbuf
┌──────────────────▼──────────────────────────────────┐
│     eBPF プローブ（カーネル空間）                       │ ← カーネル層
│     XDP: UDPパケット検査・FlatBuffersヘッダ解析        │
│     TC ingress: VRChat OSCパケット識別・フィルタ       │
└──────────────────┬──────────────────────────────────┘
                   │
                NIC（物理 or 仮想）
```

## コンポーネント

| コンポーネント | 説明 |
|---|---|
| **ATP** (Anti-Teleport Protocol) | EKF + デッドレコニング + Hermite補間による予測同期エンジン |
| **SMSL** (Shared Memory Spatial Ledger) | SeqLock + S2 Geometry によるゼロコピー空間データベース |
| **SMA** (Spatial Math Accelerator) | AVX-512/NEON SIMD による高速空間演算ライブラリ |
| **Bridges** | VRChat OSC / SNS / AR-VPS アダプター群 |
| **eBPF Probes** | NIC直後のカーネル内パケットフィルタリング |

## クイックスタート

```bash
curl -fsSL https://raw.githubusercontent.com/CrBridge/cr-bridge/main/install.sh | bash
```

またはリポジトリをクローンしてローカルインストール:

```bash
git clone https://github.com/CrBridge/cr-bridge
cd cr-bridge
./install.sh
```

## デモメタバース

インストール後、デモを起動:

```bash
cr-bridge-demo
```

ブラウザで `http://localhost:3000` を開くと、パケットロス下での**テレポートあり vs CR-Bridge補間**の比較デモが表示されます。

## ビルド要件

- Rust 1.75+ (stable)
- Linux kernel 5.15+ (eBPF プローブ使用時)
- libbpf-dev (eBPF プローブのみ、オプション)
- clang/llvm (eBPF プローブコンパイル時)

## 性能目標

| 評価項目 | 目標値 |
|---|---|
| EKF計算（1エンティティ） | < 5μs / update |
| SIMD座標変換（100エンティティ） | < 0.1ms / frame |
| 受信→AR描画レイテンシ | < 3ms |
| テレポート発生率（パケロス10%時） | < 5% |

## ライセンス

MIT License
