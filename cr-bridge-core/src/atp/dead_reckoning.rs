//! デッドレコニング（慣性航法予測）
//!
//! パケット欠落中、速度・加速度・角速度を使って
//! エンティティの位置・姿勢を物理的に再計算する。
//!
//! ATPエンジンが EKF predict の前に呼ぶ軽量予測。

use crate::types::{ATPPacket, EntityState, Quaternionf, Vec3f};

/// デッドレコニングによる次状態の予測
///
/// ## 物理モデル
/// - 位置: p(t+dt) = p(t) + v(t)*dt + 0.5*a(t)*dt²
/// - 速度: v(t+dt) = v(t) + a(t)*dt
/// - 姿勢: q(t+dt) = integrate_angular_velocity(q(t), ω, dt)
pub fn predict_next_state(state: &EntityState, dt_secs: f32) -> EntityState {
    let dt = dt_secs;
    let dt2 = 0.5 * dt * dt;

    // 位置更新（等加速度モデル）
    let new_position = Vec3f {
        x: state.position.x + state.velocity.x * dt + state.acceleration.x * dt2,
        y: state.position.y + state.velocity.y * dt + state.acceleration.y * dt2,
        z: state.position.z + state.velocity.z * dt + state.acceleration.z * dt2,
    };

    // 速度更新
    let new_velocity = Vec3f {
        x: state.velocity.x + state.acceleration.x * dt,
        y: state.velocity.y + state.acceleration.y * dt,
        z: state.velocity.z + state.acceleration.z * dt,
    };

    EntityState {
        entity_id: state.entity_id,
        position: new_position,
        velocity: new_velocity,
        acceleration: state.acceleration, // 加速度は定数モデル
        orientation: state.orientation,   // 姿勢は別途 EKF で更新
        timestamp_us: state.timestamp_us + (dt_secs * 1_000_000.0) as u64,
        since_last_packet_us: state.since_last_packet_us + (dt_secs * 1_000_000.0) as u64,
        packet_received: false,
    }
}

/// パケットデータからデッドレコニング予測を実行
///
/// 最後に受信したパケットから dt_us マイクロ秒後の状態を計算する
pub fn dead_reckon_from_packet(packet: &ATPPacket, dt_us: u64) -> EntityState {
    let dt = dt_us as f32 / 1_000_000.0;
    let dt2 = 0.5 * dt * dt;

    let pos = Vec3f {
        x: packet.position.x + packet.velocity.x * dt + packet.acceleration.x * dt2,
        y: packet.position.y + packet.velocity.y * dt + packet.acceleration.y * dt2,
        z: packet.position.z + packet.velocity.z * dt + packet.acceleration.z * dt2,
    };

    let vel = Vec3f {
        x: packet.velocity.x + packet.acceleration.x * dt,
        y: packet.velocity.y + packet.acceleration.y * dt,
        z: packet.velocity.z + packet.acceleration.z * dt,
    };

    // 角速度を積分して姿勢を更新
    let ori = packet
        .orientation
        .integrate_angular_velocity(&packet.angular_velocity, dt);

    EntityState {
        entity_id: packet.entity_id,
        position: pos,
        velocity: vel,
        acceleration: packet.acceleration,
        orientation: ori,
        timestamp_us: packet.timestamp_us + dt_us,
        since_last_packet_us: dt_us,
        packet_received: false,
    }
}

/// 動的ジッターバッファ管理
///
/// EWMA でネットワーク遅延分散 σ_t を追跡し、
/// バッファ量を [σ_t × 1.5] ms に自動調整する
pub struct JitterBuffer {
    /// 指数移動平均の遅延
    ewma_delay_ms: f32,
    /// 分散の指数移動平均
    ewma_variance: f32,
    /// EWMAの平滑化係数 (0-1)
    alpha: f32,
}

impl Default for JitterBuffer {
    fn default() -> Self {
        Self {
            ewma_delay_ms: 20.0,
            ewma_variance: 4.0,
            alpha: 0.1,
        }
    }
}

impl JitterBuffer {
    pub fn new(alpha: f32) -> Self {
        Self {
            alpha,
            ..Default::default()
        }
    }

    /// 新しい遅延サンプルで EWMA を更新
    pub fn update(&mut self, delay_ms: f32) {
        let error = delay_ms - self.ewma_delay_ms;
        self.ewma_delay_ms += self.alpha * error;
        self.ewma_variance = (1.0 - self.alpha) * self.ewma_variance + self.alpha * error * error;
    }

    /// 推奨バッファ量 (ms) を返す
    ///
    /// σ_t × 1.5 ms（仕様通り）
    pub fn recommended_buffer_ms(&self) -> f32 {
        self.ewma_variance.sqrt() * 1.5
    }

    /// 現在の EWMA 遅延 (ms)
    pub fn current_delay_ms(&self) -> f32 {
        self.ewma_delay_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BridgeType, EntityType, Quaternionf, Vec3f};

    #[test]
    fn test_dead_reckoning_constant_velocity() {
        let packet = ATPPacket {
            entity_id: 1,
            timestamp_us: 0,
            sequence: 0,
            position: Vec3f::new(0.0, 0.0, 0.0),
            velocity: Vec3f::new(1.0, 0.0, 0.0), // 1 m/s in x
            acceleration: Vec3f::ZERO,
            orientation: Quaternionf::IDENTITY,
            angular_velocity: Vec3f::ZERO,
            imu_confidence: 1.0,
            entity_type: EntityType::VRAvatar,
            source_bridge: BridgeType::Direct,
        };

        // 1秒後の予測
        let state = dead_reckon_from_packet(&packet, 1_000_000);
        assert!((state.position.x - 1.0).abs() < 1e-4, "x={}", state.position.x);
        assert!(state.position.y.abs() < 1e-4);
    }

    #[test]
    fn test_dead_reckoning_with_acceleration() {
        let packet = ATPPacket {
            entity_id: 1,
            timestamp_us: 0,
            sequence: 0,
            position: Vec3f::new(0.0, 0.0, 0.0),
            velocity: Vec3f::new(0.0, 0.0, 0.0),
            acceleration: Vec3f::new(2.0, 0.0, 0.0), // 2 m/s²
            orientation: Quaternionf::IDENTITY,
            angular_velocity: Vec3f::ZERO,
            imu_confidence: 1.0,
            entity_type: EntityType::VRAvatar,
            source_bridge: BridgeType::Direct,
        };

        // 1秒後: x = 0.5 * 2 * 1² = 1.0
        let state = dead_reckon_from_packet(&packet, 1_000_000);
        assert!((state.position.x - 1.0).abs() < 1e-4, "x={}", state.position.x);
    }

    #[test]
    fn test_jitter_buffer() {
        let mut jb = JitterBuffer::new(0.2);
        for _ in 0..100 {
            jb.update(20.0);
        }
        assert!((jb.current_delay_ms() - 20.0).abs() < 1.0);
        // 分散が小さいので推奨バッファも小さい
        assert!(jb.recommended_buffer_ms() < 5.0);
    }
}
