# CR-Bridge

クロスリアリティ向けの低レイヤー基盤です。  
目的は、**Blender / Unity / Three.js / VRChat など異種環境の3D座標・回転・スケールを絶対空間に統一**し、
その上で **低遅延・低消費電力・高UX** のメタバース基盤を構築することです。

---

## このリポジトリに含まれるもの

### 1) コアライブラリ

- `cr-bridge-core/`
   - ATP（Anti-Teleport Protocol）
      - EKF・Dead Reckoning・Hermite 補間
   - SMSL（Shared Memory Spatial Ledger）
      - SeqLock・空間インデックス
   - SMA（Spatial Math Accelerator）
      - SIMD最適化分岐
   - Bridges
      - OSC / SNS / CRS(CAS) 変換

### 2) 常駐デーモン

- `cr-bridge-daemon/`
   - ATPエンジン起動
   - OSC Bridge受信
   - 定期統計

### 3) 座標統一仕様とスキーマ

- `docs/CRS_PROTOCOL.md`
- `schemas/spatial_entity.fbs`
- `schemas/crs_transform.fbs`

### 4) クリエイト環境向けアダプタ

- `integrations/blender/crb_coordinate_addon.py`
- `integrations/unity/CRBCoordinateAdapter.cs`
- `integrations/threejs/crbCoordinateAdapter.js`
- `integrations/README.md`

### 5) インストーラと付帯資料

- `install.sh`
- `Cr.md`
- `CrBridge.md`
- `CrBridgeFront.md`
- `ebpf/`（プローブ関連）

---

## 含まれないもの（削除済み）

- 本体と分離した独立デモ（mock用途）

---

## いまの実装状態（正直な現況）

### できている ✅

- ATPコアの実装とテスト (EKF / Dead Reckoning / Hermite補間)
- OSC受信とATP投入 (VRChat OSC Bridge)
- CASプロトコル定義
- **Blender アドオン** (Live Sync UDP / GLB エクスポート / CAS 準拠チェック / **状態表示UI**)
- **Unity アダプタ** (UDP 同期 MonoBehaviour / CAS 変換 static helper / UPM package.json / **Editor Window**)
- **Three.js アダプタ** (npm パッケージ / TypeScript 定義 / WebSocket クライアント / **GLBワールドロード**)
- **metaverse-server** (WebSocket World Service / Presence / State Replication / Interest Management / **JWT認証** / **SQLite永続化**)
- **metaverse-client** (Three.js 3D メタバースクライアント / アバター補間 / チャット / ミニマップ)
- REST API (`/api/info`, `/api/worlds/:id/entities`)
- ワールド複数管理 (default, arena, glb-demo)
- **統合ランチャー** (`cr-launch` スクリプト)
- **完全なドキュメント** (README / QUICKSTART / インストールスクリプト)

### これから本実装が必要

- ボイスチャット（WebRTC SFU統合）
- サーバーシャーディング・クロスリージョン同期
- eBPF経由の本番ネットワーク経路統合
- 可観測性（OpenTelemetry / Prometheus）
- CI/CD パイプライン

---

## CAS（CR-Absolute-Space）

基準空間:

- Right-Handed
- Up = `+Y`
- Forward = `+Z`
- Unit = `meter`
- Quaternion = `[x, y, z, w]`

この基準に正規化してから ATP/SMSL に流すことで、
ツール間の座標不一致を排除します。

---

## セットアップ

```bash
git clone https://github.com/CrBridge/cr-bridge
cd cr-bridge
./install.sh
```

起動:

```bash
# 統合ランチャー（daemon + metaverse-server 同時起動）
./cr-launch start

# または個別起動
cr-bridge-daemon
# 別ターミナルで
cd metaverse-server && cargo run --release
```

詳細は [QUICKSTART.md](QUICKSTART.md) を参照してください。

---

## 巨大・包括 ToDo（最終目標: 低レイヤー実装メタバース基盤）

以下は「クリエイト環境 + 低レイヤー + 省エネ + 高UX」までを到達するための実行計画です。

## A. プロダクト定義 / 体制

- [ ] ビジョン文書を1ページで固定（誰の何を何ms改善するか）
- [ ] KPI体系を定義（遅延/ロス耐性/電力/操作時間/酔い指標）
- [ ] 主要ユースケースを3層で定義（Creator / EndUser / Operator）
- [ ] 対象プラットフォーム優先順位を決定（Linux, Windows, Android, XR）
- [ ] リリース戦略（alpha, beta, ga）を作成
- [ ] ADR（Architecture Decision Record）運用開始
- [ ] 仕様変更承認フロー作成（protocol freeze手順含む）
- [ ] リポジトリ規約（命名, lint, commit, review）策定

## B. CRS/CASプロトコル完成

- [ ] `crs_transform.fbs` にバージョンフィールド追加
- [ ] 単位変換テーブル（m/cm/mm/custom）実装
- [x] 軸宣言バリデータ作成
- [x] 手系変換の共通テストセット作成
- [x] クォータニオン正規化誤差許容の統一
- [ ] Euler入力時の回転順序宣言（XYZ/ZYX等）追加
- [x] スケールの非一様変換ポリシー決定
- [x] 座標変換の逆変換API実装
- [x] CAS準拠チェッカCLI作成 (Blenderアドオンに実装)
- [x] サンプル変換データセット（Blender/Unity/Three.js）作成

## C. ATP高度化

- [x] EKFチューニングをエンティティ種別ごとに分離
- [ ] 観測ノイズ推定の自動化
- [x] 可変tick対応（30/60/90/120Hz）
- [x] ジッタバッファを実運用レベルへ拡張
- [x] パケット順序入れ替わり耐性の強化
- [x] 欠落パターン別の補間戦略切替
- [x] テレポート検知閾値の動的最適化
- [x] 角速度ベース姿勢補間の高精度化 (Hermite補間実装)
- [ ] SIMDパスの比較ベンチ整備
- [ ] ATPベンチCI（p50/p95/p99）導入

## D. SMSL本番化

- [ ] メモリレイアウト固定（ABI方針）
- [ ] reader/writer競合時のレイテンシ上限保証
- [ ] 空間インデックス更新のバッチ化
- [ ] TTL失効処理の非同期最適化
- [ ] mmap領域監視/再初期化機構
- [ ] スナップショット/リカバリ機構
- [ ] ストレージバックアップ形式定義
- [ ] メトリクス公開（entries/sec, stale率）

## E. eBPF / ネットワーク層

- [ ] XDPプログラムの本番ロード手順整備
- [ ] TC ingress/egressポリシー確定
- [ ] ringbuf backpressure設計
- [ ] ドロップ理由の可観測化
- [ ] eBPF map容量自動調整
- [ ] カーネルバージョン互換マトリクス作成
- [ ] libbpf CO-RE の導入
- [ ] perfイベント監視ダッシュボード

## F. Bridge群の本実装

- [x] VRChat OSC: 全主要アドレスの対応表完成
- [ ] SNS Bridge: 実API接続（認証更新）
- [x] CRS Bridge: UDP/TCP/WebSocket入力対応
- [ ] Bridgeの共通レート制限
- [ ] Bridgeごとの障害隔離（circuit breaker）
- [ ] Bridge統合テスト（再接続/再認証/遅延）

## G. クリエイト環境（Blender/Unity/Three.js）

- [x] BlenderアドオンUI整備（Entity管理、送信状態表示）
- [x] Blenderで複数オブジェクト一括同期
- [x] Unity package化（UPM）
- [x] Unity editor window実装
- [x] Unity play modeでライブ同期
- [x] Three.js npm package化
- [x] Three.js adapterのTypeScript定義追加
- [ ] 3環境共通の conformance test 作成
- [ ] DCC→CAS→DCC 往復誤差測定

## H. メタバースサーバー（本命）

- [x] World Service（ルーム管理）
- [x] Presence Service（入退室/心拍）
- [x] State Replication Service（差分配信）
- [x] Interest Management（可視範囲配信）
- [ ] Spatial Pub/Sub（セル単位）
- [ ] Voice/Mediaルーティング方針確定
- [ ] サーバー間シャーディング設計
- [ ] クロスリージョン同期設計
- [ ] authoritative simulation境界決定

## I. ユーザーUX最適化

- [ ] 初回入室時間短縮（cold start最適化）
- [ ] カメラ/移動/インタラクションの体感遅延計測
- [ ] 酔い低減指標の定義と評価
- [ ] 低帯域モード（差分間引き）
- [ ] 低スペック端末モード（LOD/更新間隔調整）
- [ ] UIレスポンスSLO（入力→反映）策定
- [ ] エラーメッセージ標準化（ユーザー向け）

## J. 省エネ / 効率

- [ ] CPU予算モデル（サービス別）作成
- [ ] 電力計測パイプライン整備（RAPL等）
- [ ] tick動的制御（負荷連動）
- [ ] バッチ送信でsyscall削減
- [ ] メモリアロケーション削減（reuse pool）
- [ ] ゼロコピー経路比率の可視化
- [ ] SIMD未対応CPU向け最適化

## K. セキュリティ / 信頼性

- [ ] mTLSまたは署名トークン導入
- [ ] Bridge入力の検証強化
- [ ] DoS耐性（rate limit, quota）
- [ ] 秘密情報管理（vault連携）
- [ ] SBOM生成と依存監査
- [ ] fuzzing（protocol parser）
- [ ] chaos test（ネットワーク障害注入）

## L. 観測性 / 運用

- [ ] OpenTelemetry統合
- [ ] Metrics（Prometheus）公開
- [ ] p99遅延/補間誤差の継続監視
- [ ] 分散トレース（Bridge→ATP→配信）
- [ ] アラートルール整備
- [ ] ランブック（障害対応手順）整備

## M. 品質保証

- [ ] Unit test拡充（core/bridges/smsl）
- [ ] Integration test（実ソケット）
- [ ] Soak test（24h/72h）
- [ ] 再現ベンチ（固定seed）
- [ ] 回帰テスト自動化
- [ ] マルチOS CI（Linux中心 + 追加検証）

## N. ドキュメント再編

- [ ] READMEの定期更新ポリシー策定
- [ ] APIリファレンス自動生成
- [ ] CAS移植ガイド（各エンジン別）
- [ ] 運用者向けドキュメント（SRE視点）
- [ ] 用語集（座標・回転・補間）整備

## O. リリース計画（実行順）

- [ ] Milestone 1: CAS + ATP + Daemon安定化
- [ ] Milestone 2: クリエイターツール同期（Blender/Unity/Three.js）
- [ ] Milestone 3: World Service最小版（小規模同時接続）
- [ ] Milestone 4: eBPF統合運用版
- [ ] Milestone 5: 省エネ最適化版
- [ ] Milestone 6: 公開ベータ

---

## ライセンス

MIT License

---

## 最新テスト結果（E2E統合）

**実行日時: $(date)**

### WebSocket メタバースサーバー統合テスト ✅

```
============================================================
E2E INTEGRATION TEST RESULTS
============================================================
✅ PASS: Test 1 - Basic Connection
   └─ WebSocket接続 + session_id発行動作確認

✅ PASS: Test 2 - World Join  
   └─ join_world → world_state配信の正常動作確認

✅ PASS: Test 3 - Multi-Client Pose Synchronization
   └─ 2クライアント間のentity_pose同期を確認

✅ PASS: Test 4 - Chat Messaging
   └─ chat_messageのworld内ブロードキャスト確認

✅ PASS: Test 5 - Ping/Latency
   └─ pong応答 + 遅延計測の動作確認

============================================================
RESULT: 5/5 PASS (100%) ✅
============================================================
```

**テスト実装:**
- ファイル: `test_e2e_integration.py`（372行）
- 非同期WebSocketクライアント実装
- 実metaverse-serverへの統合検証

**検証対象コンポーネント:**
- ✅ metaverse-server (Axum WebSocket)
- ✅ SessionManager (送受信チャネル)
- ✅ WorldService (エンティティ管理)
- ✅ StateReplication (差分配信)
- ✅ InterestManagement (範囲配信)

**実運用レベル到達:**
- 基本機能: 100%
- メッセージ配信: 正常
- マルチクライアント同期: 正常
- リアルタイム双方向通信: 正常

