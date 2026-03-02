#!/usr/bin/env python3
"""
CR-Bridge Conformance Test
==========================
Blender / Unity / Three.js の座標変換が一致するか検証
"""

import json
import math
import subprocess
import sys
from pathlib import Path

# CASテスト用座標セット（多様な値）
# 注: Quaternions は全て (x, y, z, w) フォーマット [CAS標準]
TEST_CASES = [
    {
        "name": "Identity",
        "pos": (0, 0, 0),
        "rot": (0, 0, 0, 1),  # (x,y,z,w) format
        "scale": (1, 1, 1),
    },
    {
        "name": "Simple Translation",
        "pos": (1, 2, 3),
        "rot": (0, 0, 0, 1),
        "scale": (1, 1, 1),
    },
    {
        "name": "Rotation X90",
        "pos": (0, 0, 0),
        "rot": (0.7071, 0, 0, 0.7071),  # 90 deg around X
        "scale": (1, 1, 1),
    },
    {
        "name": "Simple Scale",
        "pos": (0, 0, 0),
        "rot": (0, 0, 0, 1),
        "scale": (2, 2, 2),
    },
]

def blender_quat_from_cas(quat_cas):
    """Convert CAS quaternion (x,y,z,w) to Blender (w,x,y,z) format"""
    x, y, z, w = quat_cas
    return (w, x, y, z)  # Blender format

def blender_to_cas(pos, rot_cas, scale):
    """Blender Z-up RH → CAS Y-up RH
    Note: rot_cas is in (x,y,z,w) format, needs conversion to Blender (w,x,y,z)
    """
    # Position: (x, y, z) Blender → (x, z, -y) CAS
    cas_pos = (pos[0], pos[2], -pos[1])
    
    # Rotation: Convert to Blender format and apply
    w, x, y, z = blender_quat_from_cas(rot_cas)
    # In Blender: qx_neg90 = Quaternion((w,x,y,z)) = (0.707, 0.707, -0.707, 0.0)
    # Multiplication: qx_neg90 @ q_blender
    qx_neg90 = (0.70710678, 0.70710678, -0.70710678, 0.0)  # (w,x,y,z)
    result = quat_mult_wxyz(qx_neg90, (w, x, y, z))
    # Convert back to CAS format (x,y,z,w)
    w2, x2, y2, z2 = result
    cas_rot = normalize_quat((x2, y2, z2, w2))
    
    # Scale: (x, y, z) → (x, z, y)
    cas_scale = (scale[0], scale[2], scale[1])
    
    return cas_pos, cas_rot, cas_scale

def unity_to_cas(pos, rot, scale):
    """Unity LH [-X前] → CAS RH [+Z前]
    注: Unity座標系はX右、Y上、Z奥手
    CAS座標系はX右、Y上、Z手前（RH）
    
    変換: Unity LH → CAS RH
    Position: (-x, y, z)
    Rotation: (-qx, -qy, qz, qw)
    """
    cas_pos = (-pos[0], pos[1], pos[2])
    
    qx, qy, qz, qw = rot
    cas_rot = (-qx, -qy, qz, qw)
    cas_rot = normalize_quat(cas_rot)
    
    cas_scale = scale
    
    return cas_pos, cas_rot, cas_scale

def threejs_to_cas(pos, rot, scale):
    """Three.js: Already RH Y-up, Z-forward
    Three.jsはデフォルトでCAS準拠（正規化のみ）
    """
    return pos, normalize_quat(rot), scale

def euclidean_distance(p1, p2):
    """3D Euclidean distance"""
    return math.sqrt(sum((a - b)**2 for a, b in zip(p1, p2)))

def quat_distance(q1, q2):
    """Quaternion angular distance (in radians)"""
    # Dot product
    dot = sum(a*b for a, b in zip(q1, q2))
    dot = max(-1, min(1, dot))  # Clamp to [-1, 1]
    return 2 * math.acos(abs(dot))  # Angular distance

def test_case(case):
    """Test a single conformance case"""
    name = case["name"]
    pos = case["pos"]
    rot = case["rot"]
    scale = case["scale"]
    
    # Convert to CAS
    cas_blender = blender_to_cas(pos, rot, scale)
    cas_unity = unity_to_cas(pos, rot, scale)
    cas_threejs = threejs_to_cas(pos, rot, scale)
    
    # Extract components
    pos_b, rot_b, scale_b = cas_blender
    pos_u, rot_u, scale_u = cas_unity
    pos_t, rot_t, scale_t = cas_threejs
    
    # Calculate distances
    pos_dist_bu = euclidean_distance(pos_b, pos_u)
    pos_dist_bt = euclidean_distance(pos_b, pos_t)
    pos_dist_ut = euclidean_distance(pos_u, pos_t)
    
    rot_dist_bu = quat_distance(rot_b, rot_u)
    rot_dist_bt = quat_distance(rot_b, rot_t)
    rot_dist_ut = quat_distance(rot_u, rot_t)
    
    scale_dist_bu = euclidean_distance(scale_b, scale_u)
    scale_dist_bt = euclidean_distance(scale_b, scale_t)
    scale_dist_ut = euclidean_distance(scale_u, scale_t)
    
    # Tolerances
    POS_TOL = 0.001  # 1mm
    ROT_TOL = math.radians(0.1)  # 0.1 degree
    SCALE_TOL = 0.001
    
    pos_ok = all(d < POS_TOL for d in [pos_dist_bu, pos_dist_bt, pos_dist_ut])
    rot_ok = all(d < ROT_TOL for d in [rot_dist_bu, rot_dist_bt, rot_dist_ut])
    scale_ok = all(d < SCALE_TOL for d in [scale_dist_bu, scale_dist_bt, scale_dist_ut])
    
    return {
        "name": name,
        "input": {"pos": pos, "rot": rot, "scale": scale},
        "blender_cas": {"pos": pos_b, "rot": rot_b, "scale": scale_b},
        "unity_cas": {"pos": pos_u, "rot": rot_u, "scale": scale_u},
        "threejs_cas": {"pos": pos_t, "rot": rot_t, "scale": scale_t},
        "distances": {
            "pos_blender_unity": pos_dist_bu,
            "pos_blender_threejs": pos_dist_bt,
            "pos_unity_threejs": pos_dist_ut,
            "rot_blender_unity": math.degrees(rot_dist_bu),
            "rot_blender_threejs": math.degrees(rot_dist_bt),
            "rot_unity_threejs": math.degrees(rot_dist_ut),
            "scale_blender_unity": scale_dist_bu,
            "scale_blender_threejs": scale_dist_bt,
            "scale_unity_threejs": scale_dist_ut,
        },
        "conformance": {
            "position": pos_ok,
            "rotation": rot_ok,
            "scale": scale_ok,
            "all_pass": pos_ok and rot_ok and scale_ok,
        }
    }

def main():
    print("=" * 70)
    print("CR-Bridge Conformance Test: Blender / Unity / Three.js")
    print("=" * 70)
    print()
    
    results = []
    passed = 0
    failed = 0
    
    for case in TEST_CASES:
        result = test_case(case)
        results.append(result)
        
        status = "✅ PASS" if result["conformance"]["all_pass"] else "❌ FAIL"
        print(f"{status}: {result['name']}")
        
        if result["conformance"]["all_pass"]:
            passed += 1
        else:
            failed += 1
            print(f"  Position:  {result['distances']['pos_blender_unity']:.6f}m (B-U), "
                  f"{result['distances']['pos_blender_threejs']:.6f}m (B-T)")
            print(f"  Rotation:  {result['distances']['rot_blender_unity']:.6f}° (B-U), "
                  f"{result['distances']['rot_blender_threejs']:.6f}° (B-T)")
            print(f"  Scale:     {result['distances']['scale_blender_unity']:.6f} (B-U), "
                  f"{result['distances']['scale_blender_threejs']:.6f} (B-T)")
    
    print()
    print("=" * 70)
    print(f"Results: {passed} passed, {failed} failed out of {len(TEST_CASES)} tests")
    print("=" * 70)
    
    # Output JSON
    output_file = Path(__file__).parent / "test_conformance_results.json"
    with open(output_file, "w") as f:
        json.dump({
            "summary": {
                "total": len(TEST_CASES),
                "passed": passed,
                "failed": failed,
            },
            "results": results
        }, f, indent=2)
    
    print(f"\nDetailed results: {output_file}")
    
    return 0 if failed == 0 else 1

if __name__ == "__main__":
    sys.exit(main())
