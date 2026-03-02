//! WebSocket プロトコル定義
//!
//! CR-Bridge Metaverse の通信プロトコル。
//! JSON over WebSocket。将来 MessagePack に移行可能。

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────
// Client → Server
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// ワールドへの入室
    JoinWorld {
        world_id: String,
        user_id: String,
        display_name: String,
        avatar_color: Option<String>,
    },
    /// ワールドからの退室
    LeaveWorld {},
    /// 位置・姿勢更新（20〜60Hz）
    UpdatePose {
        position: [f32; 3],
        rotation: [f32; 4], // CAS quaternion [x, y, z, w]
        velocity: [f32; 3],
        timestamp_ms: u64,
    },
    /// チャットメッセージ送信
    Chat { text: String },
    /// 死活確認
    Ping { client_timestamp_ms: u64 },
}

// ─────────────────────────────────────────────────────────────
// Server → Client
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// 接続受理
    Welcome {
        session_id: String,
        server_time_ms: u64,
    },
    /// 入室時に送る現在のワールドスナップショット
    WorldState {
        world_id: String,
        world_name: String,
        entities: Vec<EntitySnapshot>,
    },
    /// 他ユーザーが入室
    EntityJoined { entity: EntitySnapshot },
    /// 他ユーザーが退室
    EntityLeft { entity_id: String },
    /// 位置・姿勢更新ブロードキャスト
    EntityPose {
        entity_id: String,
        position: [f32; 3],
        rotation: [f32; 4],
        velocity: [f32; 3],
        timestamp_ms: u64,
    },
    /// チャットメッセージ受信
    ChatMessage {
        from_id: String,
        from_name: String,
        text: String,
        timestamp_ms: u64,
    },
    /// Ping 応答
    Pong {
        client_timestamp_ms: u64,
        server_timestamp_ms: u64,
    },
    /// サーバーエラー通知
    Error { code: String, message: String },
    /// サーバー統計（任意送信）
    Stats {
        total_sessions: usize,
        sessions_in_world: usize,
        uptime_secs: u64,
    },
}

/// エンティティのスナップショット（入室時・一覧取得時）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity_id: String,
    pub display_name: String,
    pub avatar_color: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    pub last_update_ms: u64,
}
