"""
CR-Bridge Coordinate Addon for Blender
=======================================
CAS (CR-Absolute-Space) 座標統一アドオン v0.4

【機能】
- Blender Z-up (RH) → CAS Y-up (RH) 座標変換
- UDP リアルタイム同期 (Live Sync)
- JSON クリップボードコピー
- GLB エクスポート (CAS メタデータ sidecar 付き)
- 選択オブジェクト一括送信・CAS 準拠チェック

【変換式】
  pos_CAS  = (Bx, Bz, -By)
  quat_CAS = rot_x(-90deg) * quat_Bl -> [x,y,z,w]
"""

bl_info = {
    "name": "CR-Bridge CAS Sync",
    "author": "CR-Bridge Contributors",
    "version": (0, 4, 0),
    "blender": (4, 0, 0),
    "location": "View3D > Sidebar > CR-Bridge",
    "description": "CAS 座標統一 / リアルタイム UDP 同期",
    "category": "3D View",
    "doc_url": "https://github.com/CrBridge/cr-bridge",
}

import bpy
import json
import socket
import time
import bpy.props
from bpy.types import Panel, Operator, AddonPreferences
from mathutils import Quaternion, Vector

# ----------------------------------------------------------------
# CAS 変換
# ----------------------------------------------------------------
_QX_NEG90 = Quaternion((0.70710678, -0.70710678, 0.0, 0.0))  # (w,x,y,z)

def blender_pos_to_cas(v: Vector) -> list:
    return [round(float(v.x), 6), round(float(v.z), 6), round(float(-v.y), 6)]

def blender_rot_to_cas(q: Quaternion) -> list:
    r = _QX_NEG90 @ q
    r.normalize()
    return [round(r.x, 6), round(r.y, 6), round(r.z, 6), round(r.w, 6)]

def blender_scale_to_cas(s: Vector) -> list:
    return [round(float(s.x), 6), round(float(s.z), 6), round(float(s.y), 6)]

def object_to_cas_transform(obj) -> dict:
    loc, rot, sca = obj.matrix_world.decompose()
    return {
        "entity_id":    obj.name,
        "position":     blender_pos_to_cas(loc),
        "rotation":     blender_rot_to_cas(rot),
        "scale":        blender_scale_to_cas(sca),
        "timestamp_ms": int(time.time() * 1000),
    }

# ----------------------------------------------------------------
# アドオン設定
# ----------------------------------------------------------------
class CRBPreferences(AddonPreferences):
    bl_idname = __name__
    server_host: bpy.props.StringProperty(name="Host", default="127.0.0.1")
    server_port: bpy.props.IntProperty(name="UDP Port", default=9101, min=1024, max=65535)
    sync_interval: bpy.props.FloatProperty(name="Interval (s)", default=0.05, min=0.016, max=2.0)

    def draw(self, context):
        row = self.layout.row()
        row.prop(self, "server_host"); row.prop(self, "server_port")
        self.layout.prop(self, "sync_interval")

# ----------------------------------------------------------------
# 同期状態
# ----------------------------------------------------------------
class _Sync:
    running = False; timer = None; sock = None
    sent_count = 0; error_count = 0; error_msg = ""
    host = ""; port = 0
    total_bytes = 0; last_send_time = 0.0; start_time = 0.0
    rate_history = []  # 直近10秒のレート計算用
_S = _Sync()

# ----------------------------------------------------------------
# Operators
# ----------------------------------------------------------------
class CRB_OT_SendOnce(Operator):
    bl_idname = "crb.send_once"; bl_label = "Send Selected (UDP)"
    bl_description = "選択オブジェクトを CAS JSON で UDP 送信"
    def execute(self, context):
        prefs = context.preferences.addons[__name__].preferences
        objs  = context.selected_objects
        if not objs:
            self.report({"WARNING"}, "オブジェクトを選択してください"); return {"CANCELLED"}
        payload = json.dumps({"transforms": [object_to_cas_transform(o) for o in objs]}).encode()
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.sendto(payload, (prefs.server_host, prefs.server_port)); s.close()
            self.report({"INFO"}, f"{len(objs)} 件送信")
        except Exception as e:
            self.report({"ERROR"}, str(e)); return {"CANCELLED"}
        return {"FINISHED"}

class CRB_OT_StartLiveSync(Operator):
    bl_idname = "crb.start_live_sync"; bl_label = "Start Live Sync"
    def execute(self, context):
        if _S.running:
            self.report({"WARNING"}, "既に実行中"); return {"CANCELLED"}
        prefs = context.preferences.addons[__name__].preferences
        try:
            _S.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        except Exception as e:
            self.report({"ERROR"}, str(e)); return {"CANCELLED"}
        _S.running = True; _S.sent_count = 0; _S.error_count = 0; _S.error_msg = ""
        _S.total_bytes = 0; _S.start_time = time.time(); _S.rate_history = []
        _S.host = prefs.server_host; _S.port = prefs.server_port
        wm = context.window_manager
        _S.timer = wm.event_timer_add(prefs.sync_interval, window=context.window)
        wm.modal_handler_add(self)
        return {"RUNNING_MODAL"}

    def modal(self, context, event):
        if not _S.running:
            self.cancel(context); return {"CANCELLED"}
        if event.type == "TIMER":
            objs = context.selected_objects
            if objs and _S.sock:
                payload = json.dumps({"transforms": [object_to_cas_transform(o) for o in objs]}).encode()
                try:
                    _S.sock.sendto(payload, (_S.host, _S.port))
                    _S.sent_count += 1
                    _S.total_bytes += len(payload)
                    _S.last_send_time = time.time()
                    # レート計算用（直近10秒）
                    _S.rate_history.append(time.time())
                    _S.rate_history = [t for t in _S.rate_history if time.time() - t < 10]
                except Exception as e:
                    _S.error_msg = str(e)
                    _S.error_count += 1
            context.area.tag_redraw()
        return {"PASS_THROUGH"}

    def cancel(self, context):
        wm = context.window_manager
        if _S.timer: wm.event_timer_remove(_S.timer); _S.timer = None
        if _S.sock:  _S.sock.close(); _S.sock = None
        _S.running = False

class CRB_OT_StopLiveSync(Operator):
    bl_idname = "crb.stop_live_sync"; bl_label = "Stop Live Sync"
    def execute(self, context):
        _S.running = False; self.report({"INFO"}, "Live Sync 停止"); return {"FINISHED"}

class CRB_OT_CopyJSON(Operator):
    bl_idname = "crb.copy_json"; bl_label = "Copy CAS JSON"
    def execute(self, context):
        objs = context.selected_objects
        if not objs:
            self.report({"WARNING"}, "オブジェクトを選択してください"); return {"CANCELLED"}
        context.window_manager.clipboard = json.dumps(
            {"transforms": [object_to_cas_transform(o) for o in objs]}, indent=2)
        self.report({"INFO"}, f"{len(objs)} 件コピー"); return {"FINISHED"}

class CRB_OT_ExportGLB(Operator):
    bl_idname = "crb.export_glb"; bl_label = "Export GLB (CAS)"
    filepath: bpy.props.StringProperty(subtype="FILE_PATH", default="//export.glb")
    def execute(self, context):
        objs = context.selected_objects
        if not objs:
            self.report({"WARNING"}, "オブジェクトを選択してください"); return {"CANCELLED"}
        path = bpy.path.abspath(self.filepath)
        if not path.endswith(".glb"): path += ".glb"
        bpy.ops.export_scene.gltf(
            filepath=path, use_selection=True, export_format="GLB",
            export_yup=True, export_apply=True, export_animations=True, export_extras=True)
        meta = {
            "cas_version": "0.1", "coordinate_space": "CAS",
            "right_handed": True, "up_axis": "+Y", "forward_axis": "+Z",
            "unit": "meter", "quaternion_order": "[x,y,z,w]",
            "source": "Blender/" + bpy.app.version_string,
            "exported_at": int(time.time() * 1000),
            "transforms": [object_to_cas_transform(o) for o in objs],
        }
        with open(path.replace(".glb", ".cas.json"), "w", encoding="utf-8") as f:
            json.dump(meta, f, indent=2)
        self.report({"INFO"}, f"GLB エクスポート完了: {path}")
        return {"FINISHED"}
    def invoke(self, context, event):
        context.window_manager.fileselect_add(self); return {"RUNNING_MODAL"}

class CRB_OT_Check(Operator):
    bl_idname = "crb.check_conformance"; bl_label = "CAS 準拠チェック"
    def execute(self, context):
        issues = []
        for obj in context.selected_objects:
            _, _, s = obj.matrix_world.decompose()
            if abs(s.x - s.y) > 0.001 or abs(s.y - s.z) > 0.001:
                issues.append(f"{obj.name}: 非一様スケール")
        if issues:
            self.report({"WARNING"}, "\n".join(issues))
        else:
            self.report({"INFO"}, f"{len(context.selected_objects)} 件 CAS 準拠 OK")
        return {"FINISHED"}

# ----------------------------------------------------------------
# パネル
# ----------------------------------------------------------------
class CR
        # 接続設定
        box = layout.box()
        box.label(text="UDP 送信先", icon="NETWORK_DRIVE")
        r = box.row(align=True)
        r.prop(prefs, "server_host", text=""); r.prop(prefs, "server_port", text="")
        
        # 同期状態
        box2 = layout.box()
        if _S.running:
            elapsed = time.time() - _S.start_time
            rate = len(_S.rate_history) / 10.0 if _S.rate_history else 0.0
            box2.label(text=f"🟢 Live Sync 実行中", icon="CHECKMARK")
            stats = box2.column(align=True)
            stats.label(text=f"  送信: {_S.sent_count} packets ({_S.total_bytes} bytes)")
            stats.label(text=f"  レート: {rate:.1f} pkt/s")
            stats.label(text=f"  稼働時間: {int(elapsed)}s")
            if _S.error_count > 0:
                stats.label(text=f"  エラー: {_S.error_count} 回", icon="ERROR")
            if _S.error_msg:
                box2.label(text=f"  最終エラー: {_S.error_msg}", icon="ERROR")
            box2.operator("crb.stop_live_sync", text="停止", icon="PAUSE")
        else:
            box2.label(text="⚪ 停止中", icon="RADIOBUT_OFF")
            box2.operator("crb.start_live_sync", text="Live Sync 開始", icon="PLAY")
        
        layout.separator()
        
        # 操作ボタン
        col = layout.column(align=True)
        col.operator("crb.send_once",        text="1回送信",          icon="EXPORT")
        col.operator("crb.copy_json",         text="JSON コピー",      icon="COPYDOWN")
        col.operator("crb.export_glb",        text="GLB エクスポート", icon="FILE_3D")
        col.operator("crb.check_conformance", text="準拠チェック",     icon="CHECKMARK")
        
        # 選択オブジェクト情報
        if objs:
            layout.separator()
            b3 = layout.box()
            b3.label(text=f"選択中: {len(objs)} オブジェクト", icon="OBJECT_DATA")
            for i, obj in enumerate(objs[:5]):  # 最大5個まで表示
                t = object_to_cas_transform(obj)
                p, r2 = t["position"], t["rotation"]
                sub = b3.column(align=True)
                sub.label(text=f"[{i+1}] {obj.name}")
                sub.label(text=f"  Pos: ({p[0]:.2f}, {p[1]:.2f}, {p[2]:.2f})")
                sub.label(text=f"  Rot: [{r2[0]:.2f}, {r2[1]:.2f}, {r2[2]:.2f}, {r2[3]:.2f}]")
            if len(objs) > 5:
                b3.label(text=f"  ... 他 {len(objs)-5} 個
        col.operator("crb.export_glb",        text="GLB エクスポート", icon="FILE_3D")
        col.operator("crb.check_conformance", text="準拠チェック",     icon="CHECKMARK")
        if objs:
            t = object_to_cas_transform(objs[0])
            p, r2 = t["position"], t["rotation"]
            b3 = layout.box()
            b3.label(text=f"Pos: ({p[0]:.3f}, {p[1]:.3f}, {p[2]:.3f})")
            b3.label(text=f"Rot: ({r2[0]:.3f}, {r2[1]:.3f}, {r2[2]:.3f}, {r2[3]:.3f})")

# ----------------------------------------------------------------
classes = (
    CRBPreferences, CRB_OT_SendOnce, CRB_OT_StartLiveSync, CRB_OT_StopLiveSync,
    CRB_OT_CopyJSON, CRB_OT_ExportGLB, CRB_OT_Check, CRB_PT_Main,
)
def register():
    for c in classes: bpy.utils.register_class(c)
def unregister():
    _S.running = False
    for c in reversed(classes): bpy.utils.unregister_class(c)
if __name__ == "__main__":
    register()
