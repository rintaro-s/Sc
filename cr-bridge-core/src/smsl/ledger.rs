//! Shared Memory Spatial Ledger (SMSL)
//!
//! 全プロセスが同一データを参照するゼロコピー空間データベース。
//! SeqLock + SoA + S2インデックスで構成される。

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::types::{ATPPacket, EntityState, EntityType, Vec3f, Quaternionf};
use super::entity_store::EntityStore;
use super::seqlock::{SeqLock, SeqLockedPose};
use super::spatial_index::SpatialIndex;

/// SMSL エントリ（エンティティ1件分）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub entity_id: u64,
    pub entity_type: EntityType,
    /// S2セルID（空間インデックス）
    pub s2_cell_id: u64,
    /// キャッシュ有効期限 (ms)
    pub ttl_ms: u32,
    /// 最終更新時刻 (us)
    pub last_updated_us: u64,
    /// EKF予測済み位置
    pub predicted_position: Vec3f,
    /// EKF予測済み姿勢
    pub predicted_orientation: Quaternionf,
    /// 元のパケット
    pub packet: Option<ATPPacket>,
}

impl LedgerEntry {
    pub fn is_expired(&self, now_us: u64, ttl_ms: u32) -> bool {
        let ttl_us = ttl_ms as u64 * 1000;
        now_us.saturating_sub(self.last_updated_us) > ttl_us
    }
}

/// SMSL 設定
#[derive(Debug, Clone)]
pub struct LedgerConfig {
    /// 最大エンティティ数
    pub max_entities: usize,
    /// デフォルトTTL (ms)
    pub default_ttl_ms: u32,
    /// 空間インデックスレベル（細かさ）
    pub spatial_level: u8,
}

impl Default for LedgerConfig {
    fn default() -> Self {
        Self {
            max_entities: 10_000,
            default_ttl_ms: 5_000,
            spatial_level: 10,
        }
    }
}

/// Shared Memory Spatial Ledger
///
/// ネットワーク受信バッファ → AR 描画エンジンの経路でコピーゼロを実現。
pub struct SpatialLedger {
    config: LedgerConfig,
    /// エンティティID → SeqLock<SeqLockedPose>
    pose_locks: Arc<RwLock<HashMap<u64, SeqLock<SeqLockedPose>>>>,
    /// SoA エンティティストア（SIMD処理向け）
    entity_store: Arc<RwLock<EntityStore>>,
    /// 空間インデックス
    spatial_index: Arc<RwLock<SpatialIndex>>,
    /// エントリ詳細情報
    entries: Arc<RwLock<HashMap<u64, LedgerEntry>>>,
}

impl SpatialLedger {
    pub fn new(config: LedgerConfig) -> Self {
        Self {
            entity_store: Arc::new(RwLock::new(EntityStore::new(config.max_entities))),
            spatial_index: Arc::new(RwLock::new(SpatialIndex::new(config.spatial_level))),
            pose_locks: Arc::new(RwLock::new(HashMap::new())),
            entries: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// EKF 推定済みエンティティ状態を SMSL に書き込む
    ///
    /// SeqLock で書き込みを保護し、SoA と空間インデックスを更新する。
    pub fn write_entity(&self, state: &EntityState, packet: Option<&ATPPacket>) {
        let entity_id = state.entity_id;
        let now_us = ATPPacket::now_us();

        // SeqLock に位置・姿勢を書き込み
        let pose = SeqLockedPose {
            position: [state.position.x, state.position.y, state.position.z],
            velocity: [state.velocity.x, state.velocity.y, state.velocity.z],
            orientation: [
                state.orientation.w,
                state.orientation.x,
                state.orientation.y,
                state.orientation.z,
            ],
            timestamp_us: now_us,
            _pad: [0; 4],
        };

        // pose_locks に書き込み
        {
            let locks = self.pose_locks.read();
            if let Some(lock) = locks.get(&entity_id) {
                lock.write(pose);
            } else {
                drop(locks);
                let mut locks = self.pose_locks.write();
                locks
                    .entry(entity_id)
                    .or_insert_with(|| SeqLock::new(SeqLockedPose::default()))
                    .write(pose);
            }
        }

        // SoA エンティティストアを更新
        {
            let mut store = self.entity_store.write();
            store.upsert(
                entity_id,
                state.position.x, state.position.y, state.position.z,
                state.velocity.x, state.velocity.y, state.velocity.z,
                state.orientation.w, state.orientation.x, state.orientation.y, state.orientation.z,
                now_us,
            );
        }

        // 空間インデックスを更新
        {
            let mut idx = self.spatial_index.write();
            idx.insert(entity_id, state.position.x, state.position.y, state.position.z);
        }

        // エントリを更新
        {
            let mut entries = self.entries.write();
            let entry = entries.entry(entity_id).or_insert_with(|| LedgerEntry {
                entity_id,
                entity_type: packet
                    .map(|p| p.entity_type)
                    .unwrap_or(EntityType::Unknown),
                s2_cell_id: 0,
                ttl_ms: self.config.default_ttl_ms,
                last_updated_us: now_us,
                predicted_position: state.position,
                predicted_orientation: state.orientation,
                packet: None,
            });
            entry.last_updated_us = now_us;
            entry.predicted_position = state.position;
            entry.predicted_orientation = state.orientation;
            if let Some(p) = packet {
                entry.packet = Some(p.clone());
            }
        }
    }

    /// エンティティの位置をゼロコピーで読み取る（SeqLock経由）
    pub fn read_pose(&self, entity_id: u64) -> Option<SeqLockedPose> {
        let locks = self.pose_locks.read();
        Some(locks.get(&entity_id)?.read())
    }

    /// 指定範囲内のエンティティIDを返す（視錐台クエリ）
    pub fn query_range(&self, cx: f32, cz: f32, range: f32) -> Vec<u64> {
        self.spatial_index.read().query_range(cx, cz, range)
    }

    /// 全エンティティの状態をスナップショット
    pub fn snapshot_all(&self) -> Vec<LedgerEntry> {
        let entries = self.entries.read();
        entries.values().cloned().collect()
    }

    /// 期限切れエンティティを削除（GC）
    pub fn gc(&self) {
        let now_us = ATPPacket::now_us();
        let expired: Vec<u64> = {
            let entries = self.entries.read();
            entries
                .values()
                .filter(|e| e.is_expired(now_us, self.config.default_ttl_ms))
                .map(|e| e.entity_id)
                .collect()
        };

        if expired.is_empty() {
            return;
        }

        let mut entries = self.entries.write();
        let mut idx = self.spatial_index.write();
        let mut locks = self.pose_locks.write();

        for id in expired {
            entries.remove(&id);
            idx.remove(id);
            locks.remove(&id);
        }
    }

    /// 登録エンティティ数
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }
}
