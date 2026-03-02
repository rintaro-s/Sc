//! SNS Spatial Bridge
//!
//! SNS 投稿に「座標」を後付けして SMSL の S2 インデックスに登録する。
//! 「SNS の投稿は、ずっと時系列で流れてきた。CR-Bridge はそれを空間に変換する」
//!
//! ## サポート予定
//! - Twitter/X: Filtered Stream API（位置情報 or 地名抽出）
//! - Instagram: Graph API（Media with Location）
//!
//! ## 現在の実装
//! - 地名 → 座標変換（簡易版）
//! - SNS 投稿を ATPPacket として SMSL に登録

use std::sync::Arc;

use crate::atp::engine::ATPEngine;
use crate::types::{ATPPacket, BridgeType, EntityType, Quaternionf, Vec3f};

/// SNS 投稿エンティティ（座標付き）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SNSPost {
    /// 投稿ID（SNS プラットフォームのID）
    pub post_id: String,
    /// プラットフォーム名
    pub platform: SNSPlatform,
    /// テキスト内容（100文字まで）
    pub text: String,
    /// 緯度（現実座標）
    pub lat: f64,
    /// 経度（現実座標）
    pub lng: f64,
    /// 3D座標（仮想空間マッピング）
    pub position: Vec3f,
    /// 投稿時刻 (Unix timestamp ms)
    pub created_at_ms: u64,
    /// いいね数
    pub likes: u32,
}

/// SNS プラットフォーム種別
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum SNSPlatform {
    Twitter,
    Instagram,
    Bluesky,
}

/// SNS Spatial Bridge
pub struct SNSSpatialBridge {
    engine: Arc<ATPEngine>,
    /// 登録済み投稿のカウンタ（entity_id 生成用）
    next_id: std::sync::atomic::AtomicU64,
}

impl SNSSpatialBridge {
    pub fn new(engine: Arc<ATPEngine>) -> Self {
        Self {
            engine,
            next_id: std::sync::atomic::AtomicU64::new(0x8000_0000_0000_0000),
        }
    }

    /// SNS 投稿を SMSL に空間エンティティとして登録
    pub fn register_post(&self, post: &SNSPost) -> u64 {
        let entity_id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let packet = ATPPacket {
            entity_id,
            timestamp_us: post.created_at_ms * 1000,
            sequence: 0,
            position: post.position,
            velocity: Vec3f::ZERO,      // SNS投稿は移動しない
            acceleration: Vec3f::ZERO,
            orientation: Quaternionf::IDENTITY,
            angular_velocity: Vec3f::ZERO,
            imu_confidence: 0.0, // SNSは IMU なし
            entity_type: EntityType::SNSPost,
            source_bridge: match post.platform {
                SNSPlatform::Twitter => BridgeType::TwitterStream,
                SNSPlatform::Instagram => BridgeType::InstagramGraph,
                _ => BridgeType::Unknown,
            },
        };

        self.engine.receive_packet(packet);
        entity_id
    }

}
