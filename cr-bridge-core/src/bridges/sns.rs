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
//! - シミュレーション/サンプル投稿の生成
//! - 地名 → 座標変換（簡易版）
//! - SNS 投稿を ATPPacket として SMSL に登録

use std::sync::Arc;

use tracing::info;

use crate::atp::engine::ATPEngine;
use crate::smsl::ledger::{LedgerEntry, SpatialLedger};
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
    Simulated,
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

    /// デモ用：架空のSNS投稿をバッチ生成して登録する
    pub fn populate_demo_posts(&self, count: usize) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let templates = [
            ("東京タワーから夕焼けを見ている🗼", 35.6586, 139.7454, SNSPlatform::Twitter),
            ("渋谷のスクランブル交差点！人が多すぎ😂", 35.6595, 139.7004, SNSPlatform::Instagram),
            ("秋葉原でガンプラを買ってきた🤖", 35.7021, 139.7753, SNSPlatform::Twitter),
            ("浅草寺で参拝してきた🙏", 35.7148, 139.7967, SNSPlatform::Instagram),
            ("新宿の高層ビル群、いつ見てもすごい", 35.6896, 139.6917, SNSPlatform::Twitter),
            ("上野公園でお花見🌸", 35.7147, 139.7743, SNSPlatform::Instagram),
            ("VRChatの東京駅ワールドにいる人ー？", 35.6812, 139.7671, SNSPlatform::Twitter),
            ("六本木ヒルズからの夜景が最高すぎる✨", 35.6602, 139.7293, SNSPlatform::Instagram),
        ];

        info!("デモ SNS 投稿を {} 件生成中...", count);

        for i in 0..count {
            let tmpl = &templates[i % templates.len()];
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            // 緯度経度を仮想空間の3D座標にマッピング
            // 東京駅(35.68, 139.77)を原点とした座標系
            let origin_lat = 35.6812f64;
            let origin_lng = 139.7671f64;
            let scale = 10000.0f64; // 1度 = 10000m（簡易）

            let offset_x = (tmpl.2 - origin_lng) * scale * 111_000.0 * origin_lat.to_radians().cos();
            let offset_z = (tmpl.1 - origin_lat) * scale * 111_000.0;

            // ランダムな揺らぎを追加
            let jitter_x = rng.gen_range(-50.0..50.0f64);
            let jitter_z = rng.gen_range(-50.0..50.0f64);

            let post = SNSPost {
                post_id: format!("{}-{}", tmpl.0.len(), i),
                platform: tmpl.3.clone(),
                text: tmpl.0.to_string(),
                lat: tmpl.1,
                lng: tmpl.2,
                position: Vec3f::new(
                    (offset_x + jitter_x) as f32,
                    0.0,
                    (offset_z + jitter_z) as f32,
                ),
                created_at_ms: now_ms - rng.gen_range(0..3_600_000u64),
                likes: rng.gen_range(0..1000),
            };

            self.register_post(&post);
        }

        info!("SNS 投稿の登録完了: {} 件", count);
    }
}
