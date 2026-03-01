//! ATP エンジン - Anti-Teleport Protocol のメインエンジン
//!
//! 全エンティティの EKF・デッドレコニング・Hermite補間を統合管理する。
//! tokio のタスクとして動作し、パケット受信イベントに応答する。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::dead_reckoning::{dead_reckon_from_packet, JitterBuffer};
use super::ekf::{EKFConfig, EKF};
use super::hermite::TeleportCorrector;
use super::packet::ATPSession;
use crate::types::{ATPPacket, EntityState};

/// エンティティごとの状態管理
struct EntityContext {
    ekf: EKF,
    session: ATPSession,
    corrector: TeleportCorrector,
    jitter: JitterBuffer,
    /// テレポート検知カウンタ（EKFなし時の素朴実装）
    naive_teleport_count: u64,
    /// 最後に素朴実装で記録した位置（テレポート検知用）
    naive_last_pos: Option<crate::types::Vec3f>,
}

impl EntityContext {
    fn new(entity_id: u64, config: EKFConfig) -> Self {
        Self {
            ekf: EKF::new(config),
            session: ATPSession::new(entity_id),
            corrector: TeleportCorrector::new(),
            jitter: JitterBuffer::new(0.1),
            naive_teleport_count: 0,
            naive_last_pos: None,
        }
    }
}

/// ATPエンジンの統計情報
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ATPStats {
    pub entity_count: usize,
    pub total_packets_received: u64,
    pub total_packets_lost: u64,
    pub avg_packet_loss_rate: f32,
    /// テレポート発生数（素朴実装）
    pub naive_teleport_count: u64,
    /// EKF補間で回避したテレポート数
    pub ekf_prevented_teleport_count: u64,
    pub avg_ekf_latency_us: f64,
}

/// ATPエンジン
///
/// 全エンティティの状態を管理し、毎フレーム補間済み座標を提供する。
pub struct ATPEngine {
    entities: Arc<RwLock<HashMap<u64, EntityContext>>>,
    ekf_config: EKFConfig,
    stats: Arc<RwLock<ATPStats>>,
    /// テレポート判定閾値 (m) — これ以上のジャンプをテレポートとみなす
    teleport_threshold: f32,
}

impl ATPEngine {
    pub fn new(ekf_config: EKFConfig) -> Self {
        Self {
            entities: Arc::new(RwLock::new(HashMap::new())),
            ekf_config,
            stats: Arc::new(RwLock::new(ATPStats::default())),
            teleport_threshold: 0.5, // 0.5m 以上のジャンプ = テレポート
        }
    }

    /// パケットを受信して EKF を更新する
    pub fn receive_packet(&self, packet: ATPPacket) {
        let entity_id = packet.entity_id;
        let mut entities = self.entities.write();

        let ctx = entities
            .entry(entity_id)
            .or_insert_with(|| EntityContext::new(entity_id, self.ekf_config.clone()));

        let (_, lost_count) = ctx.session.receive_packet(packet.clone());

        if lost_count > 0 {
            debug!(
                entity_id,
                lost_count, "パケット欠落を検知 — EKF 予測で補完済み"
            );
        }

        // EKF の更新ステップ
        let before_pos = if ctx.ekf.is_initialized() {
            let s = ctx.ekf.get_state(entity_id, 0);
            Some(s.position)
        } else {
            None
        };

        if let Err(e) = ctx.ekf.update(&packet) {
            warn!(entity_id, error = %e, "EKF 更新エラー");
        }

        // テレポート検知（素朴実装との比較用）
        let pos = packet.position;
        if let Some(naive_last) = ctx.naive_last_pos {
            let jump = naive_last.distance_to(&pos);
            if jump > self.teleport_threshold {
                ctx.naive_teleport_count += 1;
            }
        }
        ctx.naive_last_pos = Some(pos);

        // EKF補間で回避できたテレポートを統計に反映
        if let Some(before) = before_pos {
            let ekf_state = ctx.ekf.get_state(entity_id, 0);
            let ekf_jump = before.distance_to(&ekf_state.position);
            let naive_jump = before.distance_to(&packet.position);
            if naive_jump > self.teleport_threshold && ekf_jump < self.teleport_threshold {
                let mut stats = self.stats.write();
                stats.ekf_prevented_teleport_count += 1;
            }
        }

        // Hermite補間の開始（大きなギャップがある場合）
        if ctx.ekf.is_initialized() {
            let ekf_state = ctx.ekf.get_state(entity_id, 0);
            ctx.corrector.start_blend(
                ekf_state.position,
                ekf_state.velocity,
                packet.position,
                packet.velocity,
                80.0, // 80ms でブレンド
            );
        }
    }

    /// 毎フレーム呼ぶ — EKF predict + 統計更新
    pub fn tick(&self, dt_us: u64) {
        let mut entities = self.entities.write();
        for ctx in entities.values_mut() {
            ctx.ekf.predict(dt_us);
        }
    }

    /// エンティティの現在の推定状態を取得（EKF補間済み）
    pub fn get_entity_state(&self, entity_id: u64) -> Option<EntityState> {
        let entities = self.entities.read();
        let ctx = entities.get(&entity_id)?;
        let since_last = ctx.session.last_received_us;
        Some(ctx.ekf.get_state(entity_id, since_last))
    }

    /// 全エンティティの推定状態を取得
    pub fn get_all_states(&self) -> Vec<EntityState> {
        let entities = self.entities.read();
        entities
            .iter()
            .map(|(id, ctx)| ctx.ekf.get_state(*id, ctx.session.last_received_us))
            .collect()
    }

    /// エンティティ数を返す
    pub fn entity_count(&self) -> usize {
        self.entities.read().len()
    }

    /// 統計情報を取得
    pub fn get_stats(&self) -> ATPStats {
        let entities = self.entities.read();
        let mut stats = self.stats.read().clone();

        stats.entity_count = entities.len();
        let mut total_received = 0u64;
        let mut total_lost = 0u64;
        let mut total_naive_teleports = 0u64;

        for ctx in entities.values() {
            total_received += ctx.session.total_received;
            total_lost += ctx.session.total_lost;
            total_naive_teleports += ctx.naive_teleport_count;
        }

        stats.total_packets_received = total_received;
        stats.total_packets_lost = total_lost;
        stats.naive_teleport_count = total_naive_teleports;

        let total = total_received + total_lost;
        stats.avg_packet_loss_rate = if total > 0 {
            total_lost as f32 / total as f32
        } else {
            0.0
        };

        stats
    }

    /// テレポート閾値を設定
    pub fn set_teleport_threshold(&mut self, meters: f32) {
        self.teleport_threshold = meters;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    #[test]
    fn test_engine_receive_and_get() {
        let engine = ATPEngine::new(EKFConfig::default());

        let packet = ATPPacket {
            entity_id: 42,
            timestamp_us: ATPPacket::now_us(),
            sequence: 1,
            position: Vec3f::new(1.0, 2.0, 3.0),
            velocity: Vec3f::new(0.5, 0.0, 0.0),
            acceleration: Vec3f::ZERO,
            orientation: Quaternionf::IDENTITY,
            angular_velocity: Vec3f::ZERO,
            imu_confidence: 0.9,
            entity_type: EntityType::VRAvatar,
            source_bridge: BridgeType::VRChatOSC,
        };

        engine.receive_packet(packet);

        let state = engine.get_entity_state(42);
        assert!(state.is_some());
        let s = state.unwrap();
        assert!((s.position.x - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_engine_tick_predicts() {
        let engine = ATPEngine::new(EKFConfig::default());

        let packet = ATPPacket {
            entity_id: 1,
            timestamp_us: ATPPacket::now_us(),
            sequence: 1,
            position: Vec3f::new(0.0, 0.0, 0.0),
            velocity: Vec3f::new(1.0, 0.0, 0.0),
            acceleration: Vec3f::ZERO,
            orientation: Quaternionf::IDENTITY,
            angular_velocity: Vec3f::ZERO,
            imu_confidence: 1.0,
            entity_type: EntityType::VRAvatar,
            source_bridge: BridgeType::VRChatOSC,
        };

        engine.receive_packet(packet);

        // 500ms tick（パケット欠落をシミュレート）
        engine.tick(500_000);

        let state = engine.get_entity_state(1).unwrap();
        // 1 m/s × 0.5s ≈ 0.5m
        assert!(state.position.x > 0.3, "EKF predict が機能していない: {}", state.position.x);
    }
}
