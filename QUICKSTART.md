# CR-Bridge クイックスタートガイド

このガイドでは、CR-Bridgeを実際に使ってメタバース空間で複数のクリエイティブツールを同期させる方法を説明します。

---

## 🎯 CR-Bridgeで何ができるか

- **Blender / Unity / Three.js の3D座標を統一空間（CAS）で同期**
- **VRChatのOSCデータをリアルタイム受信・補間・配信**
- **低遅延メタバースサーバーでアバター同期**
- **GLBエクスポート時にCAS座標をメタデータ化**

---

## 📦 インストール

### 必須環境

- Linux (Ubuntu 22.04+ / Fedora 38+ / Arch など)
- Rust 1.75+ (自動インストール可)
- AVX-512対応CPU (推奨、非対応CPUでも動作)

### 1. リポジトリをクローン

```bash
git clone https://github.com/CrBridge/cr-bridge
cd cr-bridge
```

### 2. インストールスクリプト実行

```bash
./install.sh
```

これにより以下が実行されます：
- Rustの自動インストール（必要な場合）
- 全コンポーネントのビルド（`cargo build --release`）
- バイナリを `~/.local/bin/` にインストール
- systemd ユーザーサービスの設定

### 3. PATHを反映

新しいターミナルを開くか、以下を実行：

```bash
export PATH="$HOME/.local/bin:$PATH"
```

---

## 🚀 基本的な使い方

### シナリオ1: VRChatからのOSC受信

VRChatでアバターを動かし、その位置をCR-Bridgeで受信・補間します。

#### ステップ1: デーモンを起動

```bash
cr-bridge-daemon
```

出力例：
```
[INFO] ATP Engine initialized with 1000 slot capacity
[INFO] OSC Bridge listening on 0.0.0.0:9001
[INFO] CAS UDP broadcast on port 9101
[INFO] Stats: packets=0, extrapolated=0, smoothed=0
```

#### ステップ2: VRChatのOSC設定

VRChat起動オプションに `--enable-debug-gui --enable-sdk-log-levels` を追加し、OSC出力を有効化します。

VRChatのOSCアドレス `/avatar/parameters/VelocityX` などが自動的に `UDP:9001` に送信されます。

#### ステップ3: データを確認

別ターミナルで：

```bash
# CASプロトコル出力をモニタリング
nc -u -l 9101
```

VRChatでアバターを動かすと、CAS形式のUDPパケットが流れます。

---

### シナリオ2: Blenderとメタバースサーバーの同期

Blenderで編集中のオブジェクトをリアルタイムでメタバース空間に反映します。

#### ステップ1: メタバースサーバーを起動

```bash
cd metaverse-server
cargo run --release
```

出力例：
```
INFO metaverse_server: 🌐 メタバースサーバー起動: http://localhost:8080
INFO metaverse_server: 🔌 WebSocket: ws://localhost:8080/ws
INFO metaverse_server: 📁 静的ファイル: metaverse-server/static
```

#### ステップ2: Blenderアドオンをインストール

```bash
cd integrations/blender
./install_blender_addon.sh
```

Blenderを起動し、`Edit > Preferences > Add-ons` で `CR-Bridge Coordinate Synchronizer` を有効化します。

#### ステップ3: Blenderでオブジェクトを同期

1. Blenderで3Dオブジェクト (例: Cube) を選択
2. サイドバー (`N`キー) → `CR-Bridge` タブを開く
3. **Target IP**: `127.0.0.1`、**Port**: `9101`
4. 「Start Live Sync」をクリック

オブジェクトを移動・回転すると、リアルタイムで座標が送信されます。

#### ステップ4: ブラウザでメタバース空間を表示

```bash
xdg-open http://localhost:8080
```

ログイン画面で以下を入力：
- **名前**: 任意 (例: `TestUser`)
- **色**: 好きな色を選択
- **ワールド**: `default` or `arena`

「Join World」をクリックすると、3D空間が表示されます。

#### ステップ5: Blenderの動きを確認

Blenderでオブジェクトを動かすと、ブラウザの3D空間に反映されます（現在はアバターとして表示）。

---

### シナリオ3: UnityからWebSocket同期

Unityで作成したキャラクターをメタバース空間に同期します。

#### ステップ1: Unity Packageをインストール

1. Unityプロジェクトを開く
2. `Window > Package Manager > + > Add package from disk...`
3. `integrations/unity/package.json` を選択

または、手動で `Packages/manifest.json` に追加：

```json
{
  "dependencies": {
    "jp.crbridge.coordinate-adapter": "file:../../integrations/unity"
  }
}
```

#### ステップ2: GameObjectにアタッチ

1. Hierarchy で同期したいGameObject (例: Player) を選択
2. `Add Component` → `CRB Sync Component`
3. Inspector で以下を設定：
   - **Target IP**: `127.0.0.1`
   - **Target Port**: `9101`
   - **Update Rate**: `20` (Hz)

#### ステップ3: Play Mode で確認

Unityで Play を押すと、自動的にUDP送信が開始されます。ブラウザのメタバース空間で位置が更新されます。

---

### シナリオ4: Three.jsアプリから接続

独自のThree.jsアプリをメタバースサーバーに接続します。

#### ステップ1: npm packageをインストール

```bash
cd your-threejs-project
npm install ../path/to/cr-bridge/integrations/threejs
```

#### ステップ2: サンプルコード

```javascript
import * as THREE from 'three';
import { CRBMetaverseClient, applyCASTransform } from '@cr-bridge/threejs-adapter';

// WebSocket接続
const client = new CRBMetaverseClient('ws://localhost:8080/ws');

client.onEntityPose = (data) => {
  // リモートアバターの位置を更新
  const avatar = scene.getObjectByName(data.entity_id);
  if (avatar) {
    applyCASTransform(avatar, {
      position: data.position,
      rotation: data.rotation,
      scale: [1, 1, 1]
    });
  }
};

client.connect({
  displayName: 'ThreeJSUser',
  avatarColor: '#00ff00',
  worldId: 'default'
});

// アニメーションループで自分の位置を送信
function animate() {
  const pos = camera.position.toArray();
  const rot = camera.quaternion.toArray(); // [x,y,z,w]
  
  client.updatePose(pos, rot, [0, 0, 0]); // velocity
  
  requestAnimationFrame(animate);
}
animate();
```

---

## 🔧 高度な設定

### systemd自動起動

```bash
# デーモンを自動起動に設定
systemctl --user enable cr-bridge-daemon

# 今すぐ起動
systemctl --user start cr-bridge-daemon

# 状態確認
systemctl --user status cr-bridge-daemon
```

### ログ確認

```bash
# デーモンログ
tail -f ~/.local/state/cr-bridge/daemon.log

# メタバースサーバーログ
RUST_LOG=debug cargo run --release  # metaverse-server/
```

### 環境変数

#### cr-bridge-daemon

- `RUST_LOG`: ログレベル (`info`, `debug`, `trace`)
- `OSC_BIND`: OSC受信ポート (デフォルト: `0.0.0.0:9001`)
- `CAS_PORT`: CAS UDP送信ポート (デフォルト: `9101`)

#### metaverse-server

- `PORT`: HTTPサーバーポート (デフォルト: `8080`)
- `STATIC_DIR`: 静的ファイルディレクトリ (デフォルト: `./static`)
- `RUST_LOG`: ログレベル

例：

```bash
PORT=8082 RUST_LOG=debug cargo run --release
```

---

## 🎨 GLBエクスポート（CASメタデータ付き）

BlenderからGLBをエクスポートする際、CAS座標をメタデータとして付与できます。

### ステップ1: Blenderでモデルを作成

通常通りモデルを作成します。

### ステップ2: CR-Bridge パネルから Export GLB

1. サイドバー (`N`キー) → `CR-Bridge` タブ
2. 「Export GLB + .cas.json」ボタンをクリック
3. 保存先を選択

出力：
- `model.glb` - 通常のGLBファイル
- `model.cas.json` - CAS座標メタデータ

### ステップ3: Three.jsでロード

```javascript
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { applyCASTransform } from '@cr-bridge/threejs-adapter';

const loader = new GLTFLoader();

// GLBをロード
loader.load('/models/model.glb', async (gltf) => {
  const model = gltf.scene;
  
  // CASメタデータをロード
  const response = await fetch('/models/model.cas.json');
  const casData = await response.json();
  
  // CAS座標を適用
  applyCASTransform(model, {
    position: casData.position,
    rotation: casData.rotation,
    scale: casData.scale
  });
  
  scene.add(model);
});
```

---

## 🐛 トラブルシューティング

### `cr-bridge-daemon` が起動しない

**症状**: `bind: Address already in use`

**原因**: ポート9001が既に使用されている

**解決策**:

```bash
# ポート使用状況を確認
sudo lsof -i :9001

# 他のプロセスを停止するか、別ポートを指定
OSC_BIND=0.0.0.0:9002 cr-bridge-daemon
```

### Blenderアドオンが見つからない

**症状**: `Add-ons` に `CR-Bridge` が表示されない

**解決策**:

```bash
# 手動でコピー
cp integrations/blender/crb_coordinate_addon.py \
   ~/.config/blender/4.0/scripts/addons/
```

Blenderを再起動し、Preferences > Add-ons で検索。

### メタバースサーバーに接続できない

**症状**: ブラウザで `WebSocket connection failed`

**解決策**:

```bash
# サーバーが起動しているか確認
netstat -tuln | grep 8080

# ファイアウォールを確認
sudo ufw allow 8080/tcp
```

### パフォーマンスが悪い

**症状**: 補間がカクつく、遅延が大きい

**解決策**:

1. **AVX-512を有効化** (対応CPUの場合)
   ```bash
   export RUSTFLAGS="-C target-feature=+avx512f"
   cargo build --release
   ```

2. **Update Rate を調整** (Blender/Unity)
   - Blender: `Update Interval (ms)` を小さくする (例: 33ms = 30Hz)
   - Unity: `Update Rate` を上げる (例: 60Hz)

3. **ログレベルを下げる**
   ```bash
   RUST_LOG=warn cr-bridge-daemon
   ```

---

## 📚 次のステップ

- [CRS_PROTOCOL.md](docs/CRS_PROTOCOL.md) - CASプロトコル詳細
- [metaverse-server REST API](http://localhost:8080/api/info) - サーバーAPI仕様
- [integrations/README.md](integrations/README.md) - アダプタ詳細

---

## 🤝 コミュニティ

- バグ報告: [GitHub Issues](https://github.com/CrBridge/cr-bridge/issues)
- 質問: [GitHub Discussions](https://github.com/CrBridge/cr-bridge/discussions)

---

## ライセンス

MIT License - 詳細は [LICENSE](LICENSE) を参照
