# CR-Bridge Unity Package (UPM)
# Packages/com.cr-bridge.cas-adapter/package.json

このディレクトリを Unity プロジェクトの `Packages/` にコピーして
UPM として利用します。

## 配置構成

```
YourUnityProject/
  Packages/
    com.cr-bridge.cas-adapter/
      package.json
      Runtime/
        CRBCoordinateAdapter.cs
        CRBCoordinateAdapter.cs.meta
      Runtime.meta
```

## package.json

```json
{
  "name": "com.cr-bridge.cas-adapter",
  "version": "0.4.0",
  "displayName": "CR-Bridge CAS Adapter",
  "description": "CAS coordinate adapter and Metaverse client for Unity",
  "unity": "2022.3",
  "unityRelease": "0f1",
  "author": {
    "name": "CR-Bridge Contributors",
    "url": "https://github.com/CrBridge/cr-bridge"
  },
  "keywords": ["CAS", "CR-Bridge", "metaverse", "coordinate"],
  "license": "MIT"
}
```

## 依存ライブラリ

- WebSocket クライアント: [websocket-sharp](https://github.com/sta/websocket-sharp)
  または NativeWebSocket (Asset Store) が必要です。
  `CRBMetaverseClient` を使う場合は事前にインストールしてください。

## 使い方

### UDP 同期 (CAS Bridge)
GameObject に `CRBSyncComponent` をアタッチするだけ。

### 静的変換関数
```csharp
using CRBridge.Integrations;

// Unity → CAS
var casPos = CRBCoordinateConverter.ToCASPosition(transform.position);
var casRot = CRBCoordinateConverter.ToCASRotation(transform.rotation);

// CAS → Unity
transform.position = CRBCoordinateConverter.FromCASPosition(casPos);
transform.rotation = CRBCoordinateConverter.FromCASRotation(casRot);
```

### GLB エクスポートメタデータ
```csharp
var meta = new CASGLBMeta {
    exported_at = DateTimeOffset.UtcNow.ToUnixTimeMilliseconds(),
    transforms  = new[] { CRBCoordinateConverter.FromTransform(transform) },
};
var json = JsonUtility.ToJson(meta, true);
File.WriteAllText("export.cas.json", json);
```
