# CR-Bridge CRS Protocol (CAS)

CR-Bridge 上で Blender / Unity / Three.js の 3D データを**絶対化**するための仕様です。

## 1. 目的

- 異なる座標系（手系・Up軸・Forward軸）を統一
- 単位系（m / cm / arbitrary unit）を統一
- 回転（Quaternion/Euler）の解釈差を解消
- CR-Bridge ATP/SMSL の基盤にそのまま流し込める形式にする

---

## 2. Canonical Space: CAS (CR-Absolute-Space)

CAS を唯一の基準座標系にします。

- Handedness: **Right-Handed**
- Up axis: **+Y**
- Forward axis: **+Z**
- Unit: **meter**
- Rotation: Quaternion **[x, y, z, w]**

すべての送信元は `source_frame` を宣言し、受信側で CAS に変換して扱います。

---

## 3. 主要変換

## 3.1 Blender -> CAS

Blender は通常 Right-Handed / Z-Up。これを Y-Up に回転します。

Position:

$$
\begin{aligned}
x' &= x \\
y' &= z \\
z' &= -y
\end{aligned}
$$

行列表現は $R_x(-\pi/2)$。

Rotation:

$$
q_{cas} = q_x(-\pi/2) \otimes q_{blender}
$$

($\otimes$ はクォータニオン積)

## 3.2 Unity -> CAS

Unity は通常 Left-Handed / Y-Up。CAS は Right-Handed なので Z反転を行います。

Position:

$$
(x',y',z') = (x, y, -z)
$$

Rotation (近似変換):

$$
(x',y',z',w') = (-x, -y, z, w)
$$

## 3.3 Three.js -> CAS

Three.js は Right-Handed / Y-Up なので基本は恒等。

$$
p' = p,\quad q' = q
$$

---

## 4. データ形式

- FlatBuffers: [schemas/crs_transform.fbs](../schemas/crs_transform.fbs)
- 送信単位: `AbsoluteTransformBatch`
- 1エンティティ: `AbsoluteTransform`

`AbsoluteTransform` は CAS 正規化後の値を持つため、CR-Bridge 側は補間/予測に専念できます。

---

## 5. CR-Bridge インフラ統合方針

1. 各アダプタ（Blender/Unity/Three.js）でローカル座標を CAS へ変換
2. `AbsoluteTransformBatch` を ATP パケットへ変換
3. ATP エンジンで EKF + Dead Reckoning + Hermite 補間
4. SMSL に格納し、AR/VR/UIへゼロコピー配信

### 実装済み受信口（本体）

- `cr-bridge-daemon` は CAS JSON を `UDP :9101` で受信
- 受信データは `CRSBridge` で `ATPPacket` へ変換され、ATP エンジンへ投入
- VRChat OSC (`UDP :9001`) と同時運用可能

---

## 6. 将来のメタバース実装に向けた拡張

- `source_frame.note` で DCC/リグ情報を付加
- `entity_id` にワールド/テナント空間をビットパッキング
- OpenXR / Unreal / Maya アダプタを追加
- ネットワーク層では CRDT と時刻同期（PTP/NTP）を組み合わせ

