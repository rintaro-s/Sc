//! CR-Bridge 共通型定義
//!
//! FlatBuffers スキーマに対応するRustの型。
//! ゼロコピーシリアライズを考慮した設計。

use serde::{Deserialize, Serialize};

/// 3次元ベクトル
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(C)]
pub struct Vec3f {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3f {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn distance_to(&self, other: &Vec3f) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn lerp(&self, other: &Vec3f, t: f32) -> Vec3f {
        Vec3f {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            z: self.z + (other.z - self.z) * t,
        }
    }
}

impl std::ops::Add for Vec3f {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { x: self.x + rhs.x, y: self.y + rhs.y, z: self.z + rhs.z }
    }
}

impl std::ops::Sub for Vec3f {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { x: self.x - rhs.x, y: self.y - rhs.y, z: self.z - rhs.z }
    }
}

impl std::ops::Mul<f32> for Vec3f {
    type Output = Self;
    fn mul(self, scalar: f32) -> Self {
        Self { x: self.x * scalar, y: self.y * scalar, z: self.z * scalar }
    }
}

/// クォータニオン（単位クォータニオン = 姿勢）
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(C)]
pub struct Quaternionf {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Quaternionf {
    pub const IDENTITY: Self = Self { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

    pub fn new(w: f32, x: f32, y: f32, z: f32) -> Self {
        let mut q = Self { w, x, y, z };
        q.normalize();
        q
    }

    pub fn normalize(&mut self) {
        let norm = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if norm > 1e-10 {
            self.w /= norm;
            self.x /= norm;
            self.y /= norm;
            self.z /= norm;
        }
    }

    pub fn dot(&self, other: &Quaternionf) -> f32 {
        self.w * other.w + self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// 球面線形補間 (SLERP)
    pub fn slerp(&self, other: &Quaternionf, t: f32) -> Quaternionf {
        let mut dot = self.dot(other);
        let other_adjusted = if dot < 0.0 {
            dot = -dot;
            Quaternionf { w: -other.w, x: -other.x, y: -other.y, z: -other.z }
        } else {
            *other
        };

        if dot > 0.9995 {
            // 非常に近い場合は線形補間
            let mut result = Quaternionf {
                w: self.w + (other_adjusted.w - self.w) * t,
                x: self.x + (other_adjusted.x - self.x) * t,
                y: self.y + (other_adjusted.y - self.y) * t,
                z: self.z + (other_adjusted.z - self.z) * t,
            };
            result.normalize();
            result
        } else {
            let theta0 = dot.acos();
            let theta = theta0 * t;
            let sin_theta = theta.sin();
            let sin_theta0 = theta0.sin();

            let s1 = (theta0 - theta).sin() / sin_theta0;
            let s2 = sin_theta / sin_theta0;

            Quaternionf {
                w: self.w * s1 + other_adjusted.w * s2,
                x: self.x * s1 + other_adjusted.x * s2,
                y: self.y * s1 + other_adjusted.y * s2,
                z: self.z * s1 + other_adjusted.z * s2,
            }
        }
    }

    /// クォータニオンから角速度ベクトルを積分してクォータニオンを更新
    pub fn integrate_angular_velocity(&self, omega: &Vec3f, dt: f32) -> Quaternionf {
        let angle = (omega.x * omega.x + omega.y * omega.y + omega.z * omega.z).sqrt();
        if angle < 1e-10 {
            return *self;
        }
        let half_angle = angle * dt * 0.5;
        let s = half_angle.sin() / angle;
        let dq = Quaternionf {
            w: half_angle.cos(),
            x: omega.x * s,
            y: omega.y * s,
            z: omega.z * s,
        };
        // q_new = dq * q_old
        let mut result = Quaternionf {
            w: dq.w * self.w - dq.x * self.x - dq.y * self.y - dq.z * self.z,
            x: dq.w * self.x + dq.x * self.w + dq.y * self.z - dq.z * self.y,
            y: dq.w * self.y - dq.x * self.z + dq.y * self.w + dq.z * self.x,
            z: dq.w * self.z + dq.x * self.y - dq.y * self.x + dq.z * self.w,
        };
        result.normalize();
        result
    }
}

/// ATP パケット：Anti-Teleport Protocol の基本伝送単位
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ATPPacket {
    /// エンティティ一意識別子
    pub entity_id: u64,
    /// 送信側マイクロ秒タイムスタンプ（補間基点）
    pub timestamp_us: u64,
    /// シーケンス番号（欠落検知用）
    pub sequence: u32,
    /// 現在座標
    pub position: Vec3f,
    /// 速度ベクトル (m/s)
    pub velocity: Vec3f,
    /// 加速度 (m/s²)
    pub acceleration: Vec3f,
    /// 姿勢クォータニオン
    pub orientation: Quaternionf,
    /// 角速度 (rad/s) - IMUデータ
    pub angular_velocity: Vec3f,
    /// IMUデータ信頼度 [0.0-1.0]
    pub imu_confidence: f32,
    /// エンティティ種別
    pub entity_type: EntityType,
    /// データソース
    pub source_bridge: BridgeType,
}

/// エンティティ種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    Unknown,
    VRAvatar,
    SNSPost,
    ARObject,
    RealLandmark,
}

/// ブリッジ種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeType {
    Unknown,
    VRChatOSC,
    TwitterStream,
    InstagramGraph,
    ARVPS,
    Direct,
}

impl ATPPacket {
    /// 現在時刻のマイクロ秒タイムスタンプを生成
    pub fn now_us() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }
}

/// エンティティの完全状態（EKF推定後）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityState {
    pub entity_id: u64,
    /// EKF推定位置
    pub position: Vec3f,
    /// EKF推定速度
    pub velocity: Vec3f,
    /// EKF推定加速度
    pub acceleration: Vec3f,
    /// EKF推定姿勢
    pub orientation: Quaternionf,
    /// 最終更新タイムスタンプ (us)
    pub timestamp_us: u64,
    /// 最後にパケットを受信してからの経過時間 (us)
    pub since_last_packet_us: u64,
    /// このフレームでパケットが届いたか
    pub packet_received: bool,
}
