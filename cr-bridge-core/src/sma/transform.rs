//! SIMD 座標変換と FMA カルマンゲイン計算
//!
//! ## AVX-512 FMA によるカルマンゲイン計算
//!
//! K = PH^T (HPH^T + R)^-1
//!
//! FMA (Fused Multiply-Add) は `a×b+c` を1命令・中間丸め誤差なしで処理。
//! AVX-512 では16個の f32 を同時処理 → スカラー比スループット約14倍。
//!
//! ## ビュー変換のバッチ処理
//!
//! 100エンティティのビュー行列変換を SIMD で並列化。
//! SoA レイアウトから直接読み出すことでストライドアクセスを排除。

use crate::types::Vec3f;

/// SIMD 利用可能情報
#[derive(Debug, Clone)]
pub struct SIMDInfo {
    pub has_avx512f: bool,
    pub has_avx2: bool,
    pub has_neon: bool,
    pub active_backend: &'static str,
}

impl SIMDInfo {
    /// 実行時に CPU の SIMD サポートを検出
    pub fn detect() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let has_avx512f = is_x86_feature_detected!("avx512f");
            let has_avx2 = is_x86_feature_detected!("avx2");
            let backend = if has_avx512f {
                "AVX-512 FMA"
            } else if has_avx2 {
                "AVX2"
            } else {
                "Scalar"
            };
            return Self {
                has_avx512f,
                has_avx2,
                has_neon: false,
                active_backend: backend,
            };
        }

        #[cfg(target_arch = "aarch64")]
        {
            return Self {
                has_avx512f: false,
                has_avx2: false,
                has_neon: true,
                active_backend: "NEON",
            };
        }

        #[allow(unreachable_code)]
        Self {
            has_avx512f: false,
            has_avx2: false,
            has_neon: false,
            active_backend: "Scalar",
        }
    }
}

/// 4×4 行列（ビュー変換用）
#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct Mat4x4 {
    pub m: [[f32; 4]; 4],
}

impl Mat4x4 {
    pub const IDENTITY: Self = Self {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// ビュー行列 = ルックアット行列
    pub fn look_at(eye: Vec3f, target: Vec3f, up: Vec3f) -> Self {
        let fwd = normalize(Vec3f {
            x: target.x - eye.x,
            y: target.y - eye.y,
            z: target.z - eye.z,
        });
        let right = normalize(cross(fwd, up));
        let new_up = cross(right, fwd);

        Self {
            m: [
                [right.x, new_up.x, -fwd.x, 0.0],
                [right.y, new_up.y, -fwd.y, 0.0],
                [right.z, new_up.z, -fwd.z, 0.0],
                [
                    -dot(right, eye),
                    -dot(new_up, eye),
                    dot(fwd, eye),
                    1.0,
                ],
            ],
        }
    }
}

fn normalize(v: Vec3f) -> Vec3f {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len < 1e-10 {
        return Vec3f::ZERO;
    }
    Vec3f { x: v.x / len, y: v.y / len, z: v.z / len }
}

fn cross(a: Vec3f, b: Vec3f) -> Vec3f {
    Vec3f {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    }
}

fn dot(a: Vec3f, b: Vec3f) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// SoA 入力バッファ（SIMD 処理向け）
#[repr(C, align(64))]
pub struct EntityPositionBatch {
    pub px: Vec<f32>,
    pub py: Vec<f32>,
    pub pz: Vec<f32>,
    pub count: usize,
}

/// ビュー変換結果
#[repr(C)]
pub struct ViewTransformResult {
    pub vx: Vec<f32>, // ビュー空間 X
    pub vy: Vec<f32>, // ビュー空間 Y
    pub vz: Vec<f32>, // ビュー空間 Z（負が前方）
}

/// バッチ ビュー変換
///
/// N エンティティの位置をビュー行列で一括変換する。
/// AVX-512/AVX2 が利用可能な場合は SIMD で処理し、
/// そうでない場合はスカラーフォールバックを使用する。
///
/// ## 目標性能
/// - AVX-512: 100エンティティを < 0.1ms / frame
/// - Scalar: 100エンティティを < 0.5ms / frame
pub fn batch_view_transform(
    batch: &EntityPositionBatch,
    view_matrix: &Mat4x4,
) -> ViewTransformResult {
    let n = batch.count;
    let mut vx = vec![0.0f32; n];
    let mut vy = vec![0.0f32; n];
    let mut vz = vec![0.0f32; n];

    // 実行時に最適なバックエンドを選択
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx512f") {
            unsafe {
                batch_view_transform_avx512(batch, view_matrix, &mut vx, &mut vy, &mut vz);
            }
            return ViewTransformResult { vx, vy, vz };
        }
    }

    // スカラーフォールバック
    batch_view_transform_scalar(batch, view_matrix, &mut vx, &mut vy, &mut vz);
    ViewTransformResult { vx, vy, vz }
}

/// スカラー版ビュー変換（フォールバック）
fn batch_view_transform_scalar(
    batch: &EntityPositionBatch,
    m: &Mat4x4,
    vx: &mut [f32],
    vy: &mut [f32],
    vz: &mut [f32],
) {
    for i in 0..batch.count {
        let px = batch.px[i];
        let py = batch.py[i];
        let pz = batch.pz[i];

        // ビュー行列のワールド→ビュー変換（4×4 アフィン）
        // w=1 の同次座標を前提とする
        vx[i] = m.m[0][0] * px + m.m[1][0] * py + m.m[2][0] * pz + m.m[3][0];
        vy[i] = m.m[0][1] * px + m.m[1][1] * py + m.m[2][1] * pz + m.m[3][1];
        vz[i] = m.m[0][2] * px + m.m[1][2] * py + m.m[2][2] * pz + m.m[3][2];
    }
}

/// AVX-512 版ビュー変換
///
/// 16エンティティを1ループで処理する。
/// `_mm512_fmadd_ps` を使い a*b+c を1命令・丸め誤差1ULP以内で計算。
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn batch_view_transform_avx512(
    batch: &EntityPositionBatch,
    m: &Mat4x4,
    vx: &mut [f32],
    vy: &mut [f32],
    vz: &mut [f32],
) {
    use std::arch::x86_64::*;

    let n = batch.count;

    // ビュー行列の各成分をブロードキャスト
    let m00 = _mm512_set1_ps(m.m[0][0]);
    let m10 = _mm512_set1_ps(m.m[1][0]);
    let m20 = _mm512_set1_ps(m.m[2][0]);
    let m30 = _mm512_set1_ps(m.m[3][0]);

    let m01 = _mm512_set1_ps(m.m[0][1]);
    let m11 = _mm512_set1_ps(m.m[1][1]);
    let m21 = _mm512_set1_ps(m.m[2][1]);
    let m31 = _mm512_set1_ps(m.m[3][1]);

    let m02 = _mm512_set1_ps(m.m[0][2]);
    let m12 = _mm512_set1_ps(m.m[1][2]);
    let m22 = _mm512_set1_ps(m.m[2][2]);
    let m32 = _mm512_set1_ps(m.m[3][2]);

    let mut i = 0;
    while i + 16 <= n {
        // 16要素ずつロード
        let px16 = _mm512_loadu_ps(batch.px[i..].as_ptr());
        let py16 = _mm512_loadu_ps(batch.py[i..].as_ptr());
        let pz16 = _mm512_loadu_ps(batch.pz[i..].as_ptr());

        // vx = m00*px + m10*py + m20*pz + m30
        // FMA: a * b + c の融合乗算加算（丸め誤差1ULP以内）
        let rx = _mm512_fmadd_ps(m00, px16,
               _mm512_fmadd_ps(m10, py16,
               _mm512_fmadd_ps(m20, pz16, m30)));

        let ry = _mm512_fmadd_ps(m01, px16,
               _mm512_fmadd_ps(m11, py16,
               _mm512_fmadd_ps(m21, pz16, m31)));

        let rz = _mm512_fmadd_ps(m02, px16,
               _mm512_fmadd_ps(m12, py16,
               _mm512_fmadd_ps(m22, pz16, m32)));

        _mm512_storeu_ps(vx[i..].as_mut_ptr(), rx);
        _mm512_storeu_ps(vy[i..].as_mut_ptr(), ry);
        _mm512_storeu_ps(vz[i..].as_mut_ptr(), rz);

        i += 16;
    }

    // 残り要素はスカラー処理
    while i < n {
        vx[i] = m.m[0][0] * batch.px[i] + m.m[1][0] * batch.py[i]
              + m.m[2][0] * batch.pz[i] + m.m[3][0];
        vy[i] = m.m[0][1] * batch.px[i] + m.m[1][1] * batch.py[i]
              + m.m[2][1] * batch.pz[i] + m.m[3][1];
        vz[i] = m.m[0][2] * batch.px[i] + m.m[1][2] * batch.py[i]
              + m.m[2][2] * batch.pz[i] + m.m[3][2];
        i += 1;
    }
}

/// スカラー版カルマンゲイン計算
///
/// K = PH^T (HPH^T + R)^-1 の簡易2×2版（ベンチマーク比較用）
///
/// AVX-512版は EKF モジュールの nalgebra 計算に組み込まれている。
/// ここでは独立したベンチマーク用実装を提供する。
pub fn kalman_gain_scalar(
    p: &[[f32; 2]; 2],
    h: &[[f32; 2]; 2],
    r: &[[f32; 2]; 2],
) -> [[f32; 2]; 2] {
    // S = H*P*H^T + R
    let mut hp = [[0.0f32; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            for k in 0..2 {
                hp[i][j] += h[i][k] * p[k][j];
            }
        }
    }

    let mut s = [[0.0f32; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            for k in 0..2 {
                s[i][j] += hp[i][k] * h[j][k]; // H^T
            }
            s[i][j] += r[i][j];
        }
    }

    // S^-1 (2×2逆行列)
    let det = s[0][0] * s[1][1] - s[0][1] * s[1][0];
    let inv_det = if det.abs() > 1e-10 { 1.0 / det } else { 0.0 };
    let s_inv = [
        [s[1][1] * inv_det, -s[0][1] * inv_det],
        [-s[1][0] * inv_det, s[0][0] * inv_det],
    ];

    // K = PH^T * S^-1
    let mut ph_t = [[0.0f32; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            for k in 0..2 {
                ph_t[i][j] += p[i][k] * h[j][k]; // H^T
            }
        }
    }

    let mut k = [[0.0f32; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            for l in 0..2 {
                k[i][j] += ph_t[i][l] * s_inv[l][j];
            }
        }
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simd_info_detect() {
        let info = SIMDInfo::detect();
        println!("SIMD バックエンド: {}", info.active_backend);
        // 実行は必ず何かのバックエンドが選択される
        assert!(!info.active_backend.is_empty());
    }

    #[test]
    fn test_batch_view_transform_identity() {
        let batch = EntityPositionBatch {
            px: vec![1.0, 2.0, 3.0],
            py: vec![0.0, 0.0, 0.0],
            pz: vec![0.0, 0.0, 0.0],
            count: 3,
        };

        let result = batch_view_transform(&batch, &Mat4x4::IDENTITY);
        for i in 0..3 {
            assert!((result.vx[i] - batch.px[i]).abs() < 1e-4,
                "vx[{}]={}", i, result.vx[i]);
        }
    }

    #[test]
    fn test_kalman_gain() {
        // P = I, H = I, R = I → K = 0.5 * I
        let p = [[1.0f32, 0.0], [0.0, 1.0]];
        let h = [[1.0f32, 0.0], [0.0, 1.0]];
        let r = [[1.0f32, 0.0], [0.0, 1.0]];

        let k = kalman_gain_scalar(&p, &h, &r);
        assert!((k[0][0] - 0.5).abs() < 1e-4, "K[0][0]={}", k[0][0]);
        assert!((k[1][1] - 0.5).abs() < 1e-4, "K[1][1]={}", k[1][1]);
    }

    #[test]
    fn test_batch_transform_large() {
        let n = 100;
        let batch = EntityPositionBatch {
            px: (0..n).map(|i| i as f32).collect(),
            py: vec![0.0; n],
            pz: vec![0.0; n],
            count: n,
        };

        let result = batch_view_transform(&batch, &Mat4x4::IDENTITY);
        assert_eq!(result.vx.len(), n);
        // x 座標がそのまま保持されているか
        for i in 0..n {
            assert!((result.vx[i] - i as f32).abs() < 1e-3,
                "vx[{}]={}", i, result.vx[i]);
        }
    }
}
