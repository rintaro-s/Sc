//! Hermite 補間（C1連続）
//!
//! 次パケット到着後、EKF予測値と実測値の差（イノベーション）を
//! 滑らかに修正する。C1連続とは位置だけでなく速度も連続になること。
//!
//! ## 補間関数
//!
//! t ∈ [0, 1]: 前パケット時刻から次パケット時刻への正規化時間
//!
//! ```text
//! h00(t) = 2t³ - 3t² + 1
//! h10(t) = t³ - 2t² + t
//! h01(t) = -2t³ + 3t²
//! h11(t) = t³ - t²
//!
//! p(t) = h00*p0 + h10*Δt*v0 + h01*p1 + h11*Δt*v1
//! ```
//!
//! p0, v0: 前パケット時点の位置・速度（EKF推定値）
//! p1, v1: 新パケットの位置・速度（観測値）
//! → 接続点で位置・速度が連続 = 視覚的にテレポートしない

use crate::types::{Quaternionf, Vec3f};

/// Hermite 基底関数
#[inline(always)]
fn h00(t: f32) -> f32 {
    2.0 * t * t * t - 3.0 * t * t + 1.0
}

#[inline(always)]
fn h10(t: f32) -> f32 {
    t * t * t - 2.0 * t * t + t
}

#[inline(always)]
fn h01(t: f32) -> f32 {
    -2.0 * t * t * t + 3.0 * t * t
}

#[inline(always)]
fn h11(t: f32) -> f32 {
    t * t * t - t * t
}

/// C1連続な Hermite 補間
///
/// ## 引数
/// - `p0`: 開始位置（EKF予測値）
/// - `v0`: 開始速度
/// - `p1`: 終了位置（新パケット観測値）
/// - `v1`: 終了速度
/// - `delta_t`: 区間の経過時間（秒）
/// - `t`: 正規化時間 [0.0, 1.0]
pub fn hermite_interpolate(
    p0: Vec3f,
    v0: Vec3f,
    p1: Vec3f,
    v1: Vec3f,
    delta_t: f32,
    t: f32,
) -> Vec3f {
    let t = t.clamp(0.0, 1.0);
    let h00 = h00(t);
    let h10 = h10(t);
    let h01 = h01(t);
    let h11 = h11(t);

    Vec3f {
        x: h00 * p0.x + h10 * delta_t * v0.x + h01 * p1.x + h11 * delta_t * v1.x,
        y: h00 * p0.y + h10 * delta_t * v0.y + h01 * p1.y + h11 * delta_t * v1.y,
        z: h00 * p0.z + h10 * delta_t * v0.z + h01 * p1.z + h11 * delta_t * v1.z,
    }
}

/// Hermite 補間の速度（微分）
///
/// d/dt [h00*p0 + h10*dt*v0 + h01*p1 + h11*dt*v1] / delta_t
pub fn hermite_velocity(
    p0: Vec3f,
    v0: Vec3f,
    p1: Vec3f,
    v1: Vec3f,
    delta_t: f32,
    t: f32,
) -> Vec3f {
    let t = t.clamp(0.0, 1.0);
    // 基底関数の微分
    let dh00 = 6.0 * t * t - 6.0 * t;
    let dh10 = 3.0 * t * t - 4.0 * t + 1.0;
    let dh01 = -6.0 * t * t + 6.0 * t;
    let dh11 = 3.0 * t * t - 2.0 * t;

    // 速度 = d/dt / delta_t
    let inv_dt = if delta_t > 1e-10 { 1.0 / delta_t } else { 0.0 };

    Vec3f {
        x: (dh00 * p0.x + dh10 * delta_t * v0.x + dh01 * p1.x + dh11 * delta_t * v1.x) * inv_dt,
        y: (dh00 * p0.y + dh10 * delta_t * v0.y + dh01 * p1.y + dh11 * delta_t * v1.y) * inv_dt,
        z: (dh00 * p0.z + dh10 * delta_t * v0.z + dh01 * p1.z + dh11 * delta_t * v1.z) * inv_dt,
    }
}

/// テレポート補正エンジン
///
/// パケット到着後、EKF予測値と実測値のギャップを
/// 指定フレーム数かけて Hermite 補間で埋める。
pub struct TeleportCorrector {
    /// 補間開始位置
    start_pos: Vec3f,
    /// 補間開始速度
    start_vel: Vec3f,
    /// 補間目標位置
    target_pos: Vec3f,
    /// 補間目標速度
    target_vel: Vec3f,
    /// 補間区間の経過時間（秒）
    blend_duration: f32,
    /// 現在の補間時間（秒）
    elapsed: f32,
    /// アクティブ中か
    active: bool,
}

impl TeleportCorrector {
    pub fn new() -> Self {
        Self {
            start_pos: Vec3f::ZERO,
            start_vel: Vec3f::ZERO,
            target_pos: Vec3f::ZERO,
            target_vel: Vec3f::ZERO,
            blend_duration: 0.1, // デフォルト100ms
            elapsed: 0.0,
            active: false,
        }
    }

    /// 新しいパケットが到着したときに補間を開始
    ///
    /// ## 引数
    /// - `current_pos`: 現在のEKF推定位置
    /// - `current_vel`: 現在のEKF推定速度
    /// - `packet_pos`: 受信パケットの位置
    /// - `packet_vel`: 受信パケットの速度
    /// - `blend_ms`: 補間に使うミリ秒数
    pub fn start_blend(
        &mut self,
        current_pos: Vec3f,
        current_vel: Vec3f,
        packet_pos: Vec3f,
        packet_vel: Vec3f,
        blend_ms: f32,
    ) {
        // ギャップが小さい場合はブレンド不要
        let gap = current_pos.distance_to(&packet_pos);
        if gap < 0.01 {
            return;
        }

        self.start_pos = current_pos;
        self.start_vel = current_vel;
        self.target_pos = packet_pos;
        self.target_vel = packet_vel;
        self.blend_duration = blend_ms / 1000.0;
        self.elapsed = 0.0;
        self.active = true;
    }

    /// フレームごとに呼ぶ。補間後の位置を返す。
    ///
    /// 補間完了後は target_pos をそのまま返す。
    pub fn update(&mut self, dt_secs: f32) -> Option<(Vec3f, Vec3f)> {
        if !self.active {
            return None;
        }

        self.elapsed += dt_secs;
        let t = (self.elapsed / self.blend_duration).min(1.0);

        let pos = hermite_interpolate(
            self.start_pos,
            self.start_vel,
            self.target_pos,
            self.target_vel,
            self.blend_duration,
            t,
        );

        let vel = hermite_velocity(
            self.start_pos,
            self.start_vel,
            self.target_pos,
            self.target_vel,
            self.blend_duration,
            t,
        );

        if t >= 1.0 {
            self.active = false;
        }

        Some((pos, vel))
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Default for TeleportCorrector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hermite_endpoints() {
        let p0 = Vec3f::new(0.0, 0.0, 0.0);
        let v0 = Vec3f::new(1.0, 0.0, 0.0);
        let p1 = Vec3f::new(1.0, 0.0, 0.0);
        let v1 = Vec3f::new(1.0, 0.0, 0.0);

        // t=0 で p0 に一致
        let r0 = hermite_interpolate(p0, v0, p1, v1, 1.0, 0.0);
        assert!((r0.x - 0.0).abs() < 1e-5);

        // t=1 で p1 に一致
        let r1 = hermite_interpolate(p0, v0, p1, v1, 1.0, 1.0);
        assert!((r1.x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_hermite_c1_continuity() {
        let p0 = Vec3f::new(0.0, 0.0, 0.0);
        let v0 = Vec3f::new(2.0, 0.0, 0.0);
        let p1 = Vec3f::new(3.0, 0.0, 0.0);
        let v1 = Vec3f::new(1.0, 0.0, 0.0);
        let dt = 1.0;

        // t=0 の速度が v0 と一致するか確認
        let vel_at_0 = hermite_velocity(p0, v0, p1, v1, dt, 0.0);
        assert!((vel_at_0.x - 2.0).abs() < 1e-4, "t=0 の速度: {}", vel_at_0.x);

        // t=1 の速度が v1 と一致するか確認
        let vel_at_1 = hermite_velocity(p0, v0, p1, v1, dt, 1.0);
        assert!((vel_at_1.x - 1.0).abs() < 1e-4, "t=1 の速度: {}", vel_at_1.x);
    }

    #[test]
    fn test_teleport_corrector() {
        let mut corrector = TeleportCorrector::new();
        let start = Vec3f::new(0.0, 0.0, 0.0);
        let target = Vec3f::new(1.0, 0.0, 0.0);

        corrector.start_blend(
            start,
            Vec3f::ZERO,
            target,
            Vec3f::ZERO,
            100.0, // 100ms
        );

        assert!(corrector.is_active());

        // 50ms 後
        let result = corrector.update(0.05);
        assert!(result.is_some());
        let (pos, _vel) = result.unwrap();
        // 中間点のはず
        assert!(pos.x > 0.0 && pos.x < 1.0, "中間点: {}", pos.x);

        // 100ms 後（完了）
        let result = corrector.update(0.1);
        assert!(!corrector.is_active());
        let (pos, _) = result.unwrap();
        assert!((pos.x - 1.0).abs() < 1e-4, "終端: {}", pos.x);
    }
}
