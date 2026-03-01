//! デモメタバース シミュレーション
//!
//! パケットロス環境をシミュレートして、
//! 素朴実装 (テレポートあり) と CR-Bridge ATP (テレポートなし) を比較する。

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize as _, Serialize};
use tokio::time::interval;

use cr_bridge_core::atp::ekf::EKFConfig;
use cr_bridge_core::atp::engine::ATPEngine;
use cr_bridge_core::types::{ATPPacket, BridgeType, EntityType, Quaternionf, Vec3f};

/// アバターの軌道パターン
#[derive(Debug, Clone)]
pub enum MovementPattern {
    /// 直線移動
    Linear { direction: Vec3f, speed: f32 },
    /// 円軌道
    Circular { radius: f32, angular_speed: f32, phase: f32 },
    /// ランダムウォーク
    RandomWalk { step_size: f32 },
    /// 8の字軌道
    Figure8 { scale: f32, speed: f32 },
}

/// 仮想アバター（送信側）
pub struct VirtualAvatar {
    pub id: u64,
    pub name: String,
    pub true_position: Vec3f,  // 送信側の「真の位置」
    pub true_velocity: Vec3f,
    pub pattern: MovementPattern,
    pub time: f32,
    pub color: String,
}

impl VirtualAvatar {
    pub fn new(id: u64, name: String, start: Vec3f, pattern: MovementPattern, color: String) -> Self {
        Self {
            id,
            name,
            true_position: start,
            true_velocity: Vec3f::ZERO,
            pattern,
            time: 0.0,
            color,
        }
    }

    /// 次フレームの真の位置を計算（サーバー側の移動）
    pub fn step(&mut self, dt: f32) {
        self.time += dt;
        match &self.pattern.clone() {
            MovementPattern::Linear { direction, speed } => {
                self.true_velocity = *direction * *speed;
                self.true_position = self.true_position + self.true_velocity * dt;
                // 境界で折り返し
                if self.true_position.x > 400.0 || self.true_position.x < -400.0 {
                    if let MovementPattern::Linear { direction, .. } = &mut self.pattern {
                        direction.x = -direction.x;
                    }
                }
                if self.true_position.z > 300.0 || self.true_position.z < -300.0 {
                    if let MovementPattern::Linear { direction, .. } = &mut self.pattern {
                        direction.z = -direction.z;
                    }
                }
            }
            MovementPattern::Circular { radius, angular_speed, phase } => {
                let angle = self.time * angular_speed + phase;
                self.true_position.x = angle.cos() * radius;
                self.true_position.z = angle.sin() * radius;
                // y軸: 編圧振動（各アバターが異なる高さで浮いて見える）
                self.true_position.y = (self.time * 1.3 + phase).sin() * 18.0;
                self.true_velocity = Vec3f::new(
                    -angle.sin() * radius * angular_speed,
                    (self.time * 1.3 + phase).cos() * 18.0 * 1.3,
                    angle.cos() * radius * angular_speed,
                );
            }
            MovementPattern::RandomWalk { step_size } => {
                let mut rng = rand::thread_rng();
                self.true_position.x += rng.gen_range(-step_size..=*step_size);
                self.true_position.z += rng.gen_range(-step_size..=*step_size);
                self.true_position.x = self.true_position.x.clamp(-400.0, 400.0);
                self.true_position.z = self.true_position.z.clamp(-300.0, 300.0);
            }
            MovementPattern::Figure8 { scale, speed } => {
                let t = self.time * speed;
                self.true_position.x = scale * (2.0 * t).sin();
                self.true_position.z = scale * t.sin() * t.cos();
                self.true_position.y = scale * 0.15 * (4.0 * t).sin();
                self.true_velocity = Vec3f::new(
                    scale * 2.0 * speed * (2.0 * t).cos(),
                    scale * 0.15 * 4.0 * speed * (4.0 * t).cos(),
                    scale * speed * ((t).cos().powi(2) - (t).sin().powi(2)),
                );
            }
        }
    }

    /// ATPパケットを生成
    pub fn make_packet(&self, seq: u32) -> ATPPacket {
        ATPPacket {
            entity_id: self.id,
            timestamp_us: ATPPacket::now_us(),
            sequence: seq,
            position: self.true_position,
            velocity: self.true_velocity,
            acceleration: Vec3f::ZERO,
            orientation: Quaternionf::IDENTITY,
            angular_velocity: Vec3f::ZERO,
            imu_confidence: 0.9,
            entity_type: EntityType::VRAvatar,
            source_bridge: BridgeType::VRChatOSC,
        }
    }
}

/// アバターの受信側状態
#[derive(Debug, Clone, Serialize)]
pub struct AvatarRxState {
    pub id: u64,
    pub name: String,
    pub color: String,
    /// 素朴実装: 最後に受信した座標をそのまま使う
    pub naive_position: Vec3f,
    /// CR-Bridge ATP: EKF 補間済み座標
    pub crbridge_position: Vec3f,
    /// 真の位置（デバッグ/評価用）
    pub true_position: Vec3f,
    /// パケットロス中かどうか
    pub in_packet_loss: bool,
    /// 最後のパケット受信からのフレーム数
    pub frames_since_packet: u32,
    /// 累積テレポート回数（素朴実装）
    pub naive_teleport_count: u32,
    /// 素朴実装とCR-Bridgeの誤差比較
    pub naive_error: f32,
    pub crbridge_error: f32,
}

/// デモフレームデータ（WebSocketで送信）
#[derive(Debug, Serialize)]
pub struct DemoFrame {
    pub avatars: Vec<AvatarRxState>,
    pub sns_posts: Vec<SNSPostState>,
    pub packet_loss_rate: f32,
    pub stats: DemoStats,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SNSPostState {
    pub id: u64,
    pub text: String,
    pub platform: String,
    pub position: Vec3f,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DemoStats {
    pub total_naive_teleports: u32,
    pub total_ekf_prevented: u32,
    pub avg_naive_error: f32,
    pub avg_crbridge_error: f32,
    pub packet_loss_pct: f32,
}

/// メタバースシミュレーション
pub struct MetaverseSimulation {
    /// ATP エンジン (CR-Bridge 実装)
    engine: Arc<ATPEngine>,
    /// 受信側アバター状態
    rx_states: RwLock<Vec<AvatarRxState>>,
    /// 送信側アバター（移動シミュレーション）
    avatars: RwLock<Vec<VirtualAvatar>>,
    /// シーケンス番号
    sequence: parking_lot::Mutex<u32>,
    /// パケットロス率 [0.0 - 1.0]
    packet_loss_rate: RwLock<f32>,
    /// SNS 投稿
    sns_posts: RwLock<Vec<SNSPostState>>,
    /// 統計
    stats: RwLock<DemoStats>,
}

impl MetaverseSimulation {
    pub fn new() -> Self {
        let ekf_config = EKFConfig {
            position_process_noise: 0.01,
            velocity_process_noise: 0.5,
            acceleration_process_noise: 2.0,
            quaternion_process_noise: 0.001,
            position_obs_noise: 0.01,
            quaternion_obs_noise: 0.01,
            initial_covariance: 10.0,
        };

        let engine = Arc::new(ATPEngine::new(ekf_config));

        // 初期アバターを作成
        let avatars = vec![
            VirtualAvatar::new(
                1, "Alice".to_string(),
                Vec3f::new(-200.0, 0.0, 0.0),
                MovementPattern::Circular { radius: 150.0, angular_speed: 0.8, phase: 0.0 },
                "#FF6B6B".to_string(),
            ),
            VirtualAvatar::new(
                2, "Bob".to_string(),
                Vec3f::new(100.0, 0.0, 100.0),
                MovementPattern::Figure8 { scale: 120.0, speed: 0.6 },
                "#4ECDC4".to_string(),
            ),
            VirtualAvatar::new(
                3, "Charlie".to_string(),
                Vec3f::new(0.0, 0.0, -150.0),
                MovementPattern::Linear {
                    direction: Vec3f::new(1.0, 0.0, 0.5).normalize(),
                    speed: 60.0,
                },
                "#45B7D1".to_string(),
            ),
            VirtualAvatar::new(
                4, "Diana".to_string(),
                Vec3f::new(-100.0, 0.0, 100.0),
                MovementPattern::RandomWalk { step_size: 8.0 },
                "#96CEB4".to_string(),
            ),
            VirtualAvatar::new(
                5, "Eve".to_string(),
                Vec3f::new(200.0, 0.0, -100.0),
                MovementPattern::Circular { radius: 80.0, angular_speed: 1.2, phase: std::f32::consts::PI },
                "#FFEAA7".to_string(),
            ),
        ];

        // 初期受信状態
        let rx_states: Vec<AvatarRxState> = avatars.iter().map(|a| AvatarRxState {
            id: a.id,
            name: a.name.clone(),
            color: a.color.clone(),
            naive_position: a.true_position,
            crbridge_position: a.true_position,
            true_position: a.true_position,
            in_packet_loss: false,
            frames_since_packet: 0,
            naive_teleport_count: 0,
            naive_error: 0.0,
            crbridge_error: 0.0,
        }).collect();

        // SNS 投稿（デモ）
        let sns_posts = vec![
            SNSPostState {
                id: 9001,
                text: "🗼 東京タワーから夕焼け最高！".to_string(),
                platform: "Twitter".to_string(),
                position: Vec3f::new(80.0, 0.0, -80.0),
            },
            SNSPostState {
                id: 9002,
                text: "🎮 VRChatで友達と会ってる".to_string(),
                platform: "Twitter".to_string(),
                position: Vec3f::new(-120.0, 0.0, 50.0),
            },
            SNSPostState {
                id: 9003,
                text: "🌸 今日の上野公園".to_string(),
                platform: "Instagram".to_string(),
                position: Vec3f::new(50.0, 0.0, 120.0),
            },
            SNSPostState {
                id: 9004,
                text: "🎵 新宿のライブ最高だった".to_string(),
                platform: "Twitter".to_string(),
                position: Vec3f::new(-200.0, 0.0, -100.0),
            },
        ];

        Self {
            engine,
            rx_states: RwLock::new(rx_states),
            avatars: RwLock::new(avatars),
            sequence: parking_lot::Mutex::new(0),
            packet_loss_rate: RwLock::new(0.20), // デフォルト20%ロス
            sns_posts: RwLock::new(sns_posts),
            stats: RwLock::new(DemoStats::default()),
        }
    }

    /// バックグラウンドでシミュレーションを実行する
    pub async fn run_simulation(&self) {
        let dt = 1.0 / 60.0; // 60fps
        let mut ticker = interval(Duration::from_millis(16));

        loop {
            ticker.tick().await;
            self.step(dt);
        }
    }

    /// 1フレーム進める（main.rsから直接呼び出す）
    pub fn step(&self, dt: f32) {
        let loss_rate = *self.packet_loss_rate.read();
        let mut rng = rand::thread_rng();

        // ATP エンジンの tick（EKF predict）
        self.engine.tick((dt * 1_000_000.0) as u64);

        // アバターを移動させてパケットを送信（一部はドロップ）
        let packets: Vec<(u64, ATPPacket, Vec3f, bool)> = {
            let mut avatars = self.avatars.write();
            let mut seq = self.sequence.lock();

            avatars.iter_mut().map(|avatar| {
                avatar.step(dt);
                *seq = seq.wrapping_add(1);
                let packet = avatar.make_packet(*seq);
                let true_pos = avatar.true_position;

                // パケットロスをシミュレート
                let dropped = rng.gen::<f32>() < loss_rate;
                (avatar.id, packet, true_pos, dropped)
            }).collect()
        };

        // 受信側の状態を更新（明示ロックスコープ）
        {
            let mut rx_states = self.rx_states.write();
            for (id, packet, true_pos, dropped) in packets {
                if let Some(rx) = rx_states.iter_mut().find(|r| r.id == id) {
                    rx.true_position = true_pos;
                    rx.in_packet_loss = dropped;

                    if !dropped {
                        // === 素朴実装: 座標を直接上書き ===
                        let old_naive = rx.naive_position;
                        let jump = old_naive.distance_to(&packet.position);
                        if jump > 20.0 && rx.frames_since_packet > 3 {
                            rx.naive_teleport_count += 1;
                        }
                        rx.naive_position = packet.position;

                        // === CR-Bridge ATP: EKF に渡す ===
                        self.engine.receive_packet(packet);
                        rx.frames_since_packet = 0;
                    } else {
                        rx.frames_since_packet += 1;
                    }

                    if let Some(state) = self.engine.get_entity_state(id) {
                        rx.crbridge_position = state.position;
                    }

                    rx.naive_error = rx.naive_position.distance_to(&true_pos);
                    rx.crbridge_error = rx.crbridge_position.distance_to(&true_pos);
                }
            }
        } // rx_states write lock 解放

        // VRChat OSC エンティティの同期（entity_id >= 90000）
        {
            let engine_states = self.engine.get_all_states();
            let mut rx_states = self.rx_states.write();
            for state in engine_states {
                if state.entity_id >= 90000 {
                    if let Some(rx) = rx_states.iter_mut().find(|r| r.id == state.entity_id) {
                        rx.crbridge_position = state.position;
                        rx.naive_position = state.position;
                        rx.true_position = state.position;
                        rx.in_packet_loss = false;
                    } else {
                        rx_states.push(AvatarRxState {
                            id: state.entity_id,
                            name: format!("VRC{:02}", state.entity_id % 100),
                            color: "#FF99FF".to_string(),
                            naive_position: state.position,
                            crbridge_position: state.position,
                            true_position: state.position,
                            in_packet_loss: false,
                            frames_since_packet: 0,
                            naive_teleport_count: 0,
                            naive_error: 0.0,
                            crbridge_error: 0.0,
                        });
                    }
                }
            }
        } // rx_states write lock 解放

        // 統計更新
        {
            let rx_states = self.rx_states.read();
            let mut stats = self.stats.write();
            stats.total_naive_teleports = rx_states.iter().map(|r| r.naive_teleport_count).sum();
            stats.avg_naive_error = if rx_states.is_empty() { 0.0 } else {
                rx_states.iter().map(|r| r.naive_error).sum::<f32>() / rx_states.len() as f32
            };
            stats.avg_crbridge_error = if rx_states.is_empty() { 0.0 } else {
                rx_states.iter().map(|r| r.crbridge_error).sum::<f32>() / rx_states.len() as f32
            };
            stats.packet_loss_pct = loss_rate * 100.0;
            let engine_stats = self.engine.get_stats();
            stats.total_ekf_prevented = engine_stats.ekf_prevented_teleport_count as u32;
        }
    }

    /// 現在のフレームデータを取得
    pub fn get_frame(&self) -> DemoFrame {
        let avatars = self.rx_states.read().clone();
        let sns_posts = self.sns_posts.read().clone();
        let stats = self.stats.read().clone();
        let packet_loss_rate = *self.packet_loss_rate.read();

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        DemoFrame {
            avatars,
            sns_posts,
            packet_loss_rate,
            stats,
            timestamp_ms: now_ms,
        }
    }

    /// ATPEngineへの Arc を返す（VRChat OSC パケットの直接投入に使用）
    pub fn get_engine(&self) -> Arc<ATPEngine> {
        Arc::clone(&self.engine)
    }

    /// パケットロス率を設定
    pub fn set_packet_loss_rate(&self, rate: f32) {
        *self.packet_loss_rate.write() = rate.clamp(0.0, 1.0);
    }

    /// シミュレーションをリセット
    pub fn reset(&self) {
        let mut rx = self.rx_states.write();
        for r in rx.iter_mut() {
            r.naive_teleport_count = 0;
            r.frames_since_packet = 0;
        }
        let mut stats = self.stats.write();
        *stats = DemoStats::default();
    }

    /// ランダムなアバターを追加
    pub fn add_random_avatar(&self) {
        let mut rng = rand::thread_rng();
        let id = rng.gen_range(100..999) as u64;
        let names = ["Faye", "Grace", "Henry", "Iris", "Jack", "Kira", "Leo", "Mia"];
        let name = names[rng.gen_range(0..names.len())].to_string();
        let colors = ["#FF9FF3", "#54A0FF", "#5F27CD", "#01CBC6", "#FF9F43", "#EE5A24"];
        let color = colors[rng.gen_range(0..colors.len())].to_string();

        let avatar = VirtualAvatar::new(
            id, name.clone(),
            Vec3f::new(
                rng.gen_range(-300.0..300.0f32),
                0.0,
                rng.gen_range(-200.0..200.0f32),
            ),
            MovementPattern::Circular {
                radius: rng.gen_range(50.0..200.0),
                angular_speed: rng.gen_range(0.3..1.5),
                phase: rng.gen_range(0.0..std::f32::consts::TAU),
            },
            color.clone(),
        );

        let rx = AvatarRxState {
            id,
            name,
            color,
            naive_position: avatar.true_position,
            crbridge_position: avatar.true_position,
            true_position: avatar.true_position,
            in_packet_loss: false,
            frames_since_packet: 0,
            naive_teleport_count: 0,
            naive_error: 0.0,
            crbridge_error: 0.0,
        };

        self.avatars.write().push(avatar);
        self.rx_states.write().push(rx);
    }
}

// Vec3f に normalize メソッドが必要
trait Normalize {
    fn normalize(self) -> Self;
}

impl Normalize for Vec3f {
    fn normalize(self) -> Self {
        let len = self.length();
        if len < 1e-10 {
            return Vec3f::ZERO;
        }
        Vec3f::new(self.x / len, self.y / len, self.z / len)
    }
}
