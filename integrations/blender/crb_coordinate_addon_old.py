bl_info = {
    "name": "CR-Bridge Coordinate Adapter",
    "author": "CR-Bridge",
    "version": (0, 1, 0),
    "blender": (3, 6, 0),
    "location": "View3D > Sidebar > CR-Bridge",
    "description": "Blender座標をCAS(RH, Y-up, +Z forward, meter)へ変換",
    "category": "3D View",
}

import json
import socket
import time
import bpy
from mathutils import Quaternion

# Blender(Z-up RH) -> CAS(Y-up RH)
# p' = (x, z, -y)
# q' = qx(-90deg) * q_blender
QX_NEG_90 = Quaternion((0.70710678, -0.70710678, 0.0, 0.0))  # (w, x, y, z)


def blender_to_cas_pos(v):
    return {
        "x": float(v.x),
        "y": float(v.z),
        "z": float(-v.y),
    }


def blender_to_cas_quat(q_bl):
    q_cas = QX_NEG_90 @ q_bl
    return {
        "x": float(q_cas.x),
        "y": float(q_cas.y),
        "z": float(q_cas.z),
        "w": float(q_cas.w),
    }


def selected_to_cas_payload(context):
    obj = context.active_object
    if obj is None:
        raise RuntimeError("アクティブオブジェクトがありません")

    loc = blender_to_cas_pos(obj.location)
    rot = blender_to_cas_quat(obj.rotation_quaternion if obj.rotation_mode == 'QUATERNION' else obj.matrix_world.to_quaternion())

    return {
        "frame_id": int(time.time() * 1000),
        "timestamp_us": int(time.time() * 1_000_000),
        "transforms": [
            {
                "entity_id": int(obj.get("crb_entity_id", 1)),
                "position_m": loc,
                "rotation": rot,
                "scale": {
                    "x": float(obj.scale.x),
                    "y": float(obj.scale.y),
                    "z": float(obj.scale.z),
                },
                "source_frame": {
                    "source": "Blender",
                    "handedness": "RightHanded",
                    "up_axis": "ZPositive",
                    "forward_axis": "YNegative",
                    "unit_scale_to_meter": 1.0,
                },
            }
        ],
    }


class CRB_OT_copy_cas_json(bpy.types.Operator):
    bl_idname = "crb.copy_cas_json"
    bl_label = "CAS JSONをコピー"

    def execute(self, context):
        try:
            payload = selected_to_cas_payload(context)
            context.window_manager.clipboard = json.dumps(payload, ensure_ascii=False)
            self.report({'INFO'}, "CAS JSON をクリップボードへコピーしました")
            return {'FINISHED'}
        except Exception as e:
            self.report({'ERROR'}, str(e))
            return {'CANCELLED'}


class CRB_OT_send_udp(bpy.types.Operator):
    bl_idname = "crb.send_udp"
    bl_label = "UDP送信"

    def execute(self, context):
        scene = context.scene
        try:
            payload = selected_to_cas_payload(context)
            msg = json.dumps(payload, ensure_ascii=False).encode("utf-8")
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.sendto(msg, (scene.crb_udp_host, scene.crb_udp_port))
            sock.close()
            self.report({'INFO'}, f"UDP送信: {scene.crb_udp_host}:{scene.crb_udp_port}")
            return {'FINISHED'}
        except Exception as e:
            self.report({'ERROR'}, str(e))
            return {'CANCELLED'}


class CRB_PT_panel(bpy.types.Panel):
    bl_label = "CR-Bridge"
    bl_idname = "CRB_PT_panel"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = 'CR-Bridge'

    def draw(self, context):
        layout = self.layout
        scene = context.scene
        layout.prop(scene, "crb_udp_host")
        layout.prop(scene, "crb_udp_port")
        layout.operator("crb.copy_cas_json")
        layout.operator("crb.send_udp")


classes = (
    CRB_OT_copy_cas_json,
    CRB_OT_send_udp,
    CRB_PT_panel,
)


def register():
    for c in classes:
        bpy.utils.register_class(c)
    bpy.types.Scene.crb_udp_host = bpy.props.StringProperty(name="Host", default="127.0.0.1")
    bpy.types.Scene.crb_udp_port = bpy.props.IntProperty(name="Port", default=9101, min=1, max=65535)


def unregister():
    del bpy.types.Scene.crb_udp_host
    del bpy.types.Scene.crb_udp_port
    for c in reversed(classes):
        bpy.utils.unregister_class(c)


if __name__ == "__main__":
    register()
