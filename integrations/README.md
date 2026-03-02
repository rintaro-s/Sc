# CR-Bridge Integrations

このディレクトリは、各ツールのローカル座標を CAS へ変換するアダプタ群です。

- Blender: [blender/crb_coordinate_addon.py](blender/crb_coordinate_addon.py)
- Unity: [unity/CRBCoordinateAdapter.cs](unity/CRBCoordinateAdapter.cs)
- Three.js: [threejs/crbCoordinateAdapter.js](threejs/crbCoordinateAdapter.js)

## CAS の定義

- Right-Handed
- Up = +Y
- Forward = +Z
- Unit = meter
- Quaternion = [x, y, z, w]

詳細は [docs/CRS_PROTOCOL.md](../docs/CRS_PROTOCOL.md) を参照してください。
