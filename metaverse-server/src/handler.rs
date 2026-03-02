//! WebSocket ハンドラ・HTTP API
//!
//! ## WebSocket フロー
//! 1. 接続 → welcome 送信
//! 2. join_world → world_state 送信 + entity_joined ブロードキャスト
//! 3. update_pose → interest 範囲内へ entity_pose ブロードキャスト
//! 4. chat → world 全体へ chat_message ブロードキャスト
//! 5. 切断 → entity_left ブロードキャスト

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    Json,
    extract::{Path, State, WebSocketUpgrade},
    extract::ws::{Message, WebSocket},
    response::{IntoResponse, Response},
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{info, warn, debug};
use uuid::Uuid;

use crate::protocol::{ClientMessage, EntitySnapshot, ServerMessage};
use crate::session::Session;
use crate::state::AppState;
use crate::world::Entity;

// ─────────────────────────────────────────────────────────────
// ユーティリティ
// ─────────────────────────────────────────────────────────────

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn random_color() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let hue: u32 = rng.gen_range(0..360);
    format!("hsl({}, 70%, 60%)", hue)
}

// ─────────────────────────────────────────────────────────────
// WebSocket エントリポイント
// ─────────────────────────────────────────────────────────────

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let session_id = Uuid::new_v4().to_string();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // セッション登録
    {
        let mut sessions = state.sessions.write();
        sessions.add(Session {
            id: session_id.clone(),
            user_id: String::new(),
            display_name: String::new(),
            world_id: None,
            tx: tx.clone(),
        });
    }

    info!("WS接続: session={:.8}", &session_id);

    // Welcome 送信
    {
        let msg = ServerMessage::Welcome {
            session_id: session_id.clone(),
            server_time_ms: now_ms(),
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = tx.send(json);
        }
    }

    let (mut ws_tx, mut ws_rx) = socket.split();

    // 書き込みタスク: rx からメッセージを受け取り WS へ送出
    let write_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_tx.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
    });

    // 読み取りループ
    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(Message::Text(text)) => {
                debug!("受信: {:.80}", &text);
                if let Err(e) = process_message(&text, &session_id, &state) {
                    warn!("メッセージ処理エラー [{}]: {}", &session_id[..8], e);
                }
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    // 切断処理
    cleanup_session(&session_id, &state);
    write_task.abort();
    info!("WS切断: session={:.8}", &session_id);
}

// ─────────────────────────────────────────────────────────────
// メッセージ処理
// ─────────────────────────────────────────────────────────────

fn process_message(
    text: &str,
    session_id: &str,
    state: &Arc<AppState>,
) -> Result<(), String> {
    let msg: ClientMessage = serde_json::from_str(text).map_err(|e| e.to_string())?;

    match msg {
        // ── join_world ──────────────────────────────────────
        ClientMessage::JoinWorld { world_id, user_id, display_name, avatar_color } => {
            let world_exists = {
                let worlds = state.worlds.read();
                worlds.get_world(&world_id).is_some()
            };
            if !world_exists {
                let sessions = state.sessions.read();
                sessions.send(
                    session_id,
                    &ServerMessage::Error {
                        code: "WORLD_NOT_FOUND".into(),
                        message: format!("World '{}' not found", world_id),
                    },
                );
                return Ok(());
            }

            let color = avatar_color.unwrap_or_else(random_color);
            let ts = now_ms();

            // エンティティをワールドへ追加
            let all_entities = {
                let mut worlds = state.worlds.write();
                let world = worlds.get_world_mut(&world_id).unwrap();
                // 同じ user_id が既存なら上書き
                world.remove_entity(&user_id);
                world.add_entity(Entity {
                    id: user_id.clone(),
                    display_name: display_name.clone(),
                    avatar_color: color.clone(),
                    position: [0.0, 0.9, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    velocity: [0.0, 0.0, 0.0],
                    last_update_ms: ts,
                });
                world.get_all_snapshots()
            };

            // セッション情報更新
            {
                let mut sessions = state.sessions.write();
                if let Some(s) = sessions.get_mut(session_id) {
                    s.user_id = user_id.clone();
                    s.display_name = display_name.clone();
                    s.world_id = Some(world_id.clone());
                }
            }

            // 入室者へ現在のワールド状態を送信
            let world_name = {
                let worlds = state.worlds.read();
                worlds
                    .get_world(&world_id)
                    .map(|w| w.name.clone())
                    .unwrap_or_default()
            };
            {
                let sessions = state.sessions.read();
                sessions.send(
                    session_id,
                    &ServerMessage::WorldState {
                        world_id: world_id.clone(),
                        world_name,
                        entities: all_entities,
                    },
                );
            }

            // 既存ユーザーへ新入室通知
            let new_snapshot = EntitySnapshot {
                entity_id: user_id.clone(),
                display_name: display_name.clone(),
                avatar_color: color,
                position: [0.0, 0.9, 0.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                velocity: [0.0, 0.0, 0.0],
                last_update_ms: ts,
            };
            {
                let sessions = state.sessions.read();
                sessions.broadcast_world(
                    &world_id,
                    &ServerMessage::EntityJoined { entity: new_snapshot },
                    Some(session_id),
                );
            }

            info!("入室: {} → world={} (total={})",
                display_name, world_id,
                state.sessions.read().sessions_in_world(&world_id));
        }

        // ── leave_world ─────────────────────────────────────
        ClientMessage::LeaveWorld {} => {
            cleanup_session(session_id, state);
        }

        // ── update_pose ─────────────────────────────────────
        ClientMessage::UpdatePose { position, rotation, velocity, timestamp_ms } => {
            let (user_id, world_id) = {
                let sessions = state.sessions.read();
                match sessions.get_user_world(session_id) {
                    Some(pair) => pair,
                    None => return Ok(()),
                }
            };

            let updated = {
                let mut worlds = state.worlds.write();
                if let Some(world) = worlds.get_world_mut(&world_id) {
                    world.update_pose(&user_id, position, rotation, velocity, timestamp_ms)
                } else {
                    false
                }
            };

            if updated {
                // Interest Management: 近くのエンティティにのみ送信
                let interested = {
                    let worlds = state.worlds.read();
                    worlds.get_world(&world_id).map(|w| {
                        w.interested_entity_ids(position, &user_id)
                    }).unwrap_or_default()
                };

                let pose_msg = ServerMessage::EntityPose {
                    entity_id: user_id.clone(),
                    position,
                    rotation,
                    velocity,
                    timestamp_ms,
                };

                let sessions = state.sessions.read();
                if interested.is_empty() {
                    // 誰もいなければブロードキャスト（フォールバック）
                    sessions.broadcast_world(&world_id, &pose_msg, Some(session_id));
                } else {
                    sessions.send_to_entity_ids(&world_id, &interested, &pose_msg);
                }
            }
        }

        // ── chat ────────────────────────────────────────────
        ClientMessage::Chat { text } => {
            let (from_id, from_name, world_id) = {
                let sessions = state.sessions.read();
                match sessions.get_display_and_world(session_id) {
                    Some(triple) => triple,
                    None => return Ok(()),
                }
            };

            // 500 字まで
            let text: String = text.chars().take(500).collect();
            if text.trim().is_empty() {
                return Ok(());
            }

            let msg = ServerMessage::ChatMessage {
                from_id,
                from_name,
                text,
                timestamp_ms: now_ms(),
            };
            let sessions = state.sessions.read();
            sessions.broadcast_world(&world_id, &msg, None);
        }

        // ── ping ────────────────────────────────────────────
        ClientMessage::Ping { client_timestamp_ms } => {
            let sessions = state.sessions.read();
            sessions.send(
                session_id,
                &ServerMessage::Pong {
                    client_timestamp_ms,
                    server_timestamp_ms: now_ms(),
                },
            );
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────
// クリーンアップ
// ─────────────────────────────────────────────────────────────

pub fn cleanup_session(session_id: &str, state: &Arc<AppState>) {
    let session = {
        let mut sessions = state.sessions.write();
        sessions.remove(session_id)
    };

    if let Some(session) = session {
        if let Some(world_id) = &session.world_id {
            let user_id = session.user_id.clone();

            // ワールドからエンティティ削除
            {
                let mut worlds = state.worlds.write();
                if let Some(world) = worlds.get_world_mut(world_id) {
                    world.remove_entity(&user_id);
                }
            }

            // 残留セッションへ退室通知
            {
                let sessions = state.sessions.read();
                sessions.broadcast_world(
                    world_id,
                    &ServerMessage::EntityLeft { entity_id: user_id.clone() },
                    None,
                );
            }

            info!("退室: {} ← world={}", session.display_name, world_id);
        }
    }
}

// ─────────────────────────────────────────────────────────────
// REST API ハンドラ
// ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ServerInfo {
    pub version: &'static str,
    pub uptime_secs: u64,
    pub total_sessions: usize,
    pub total_entities: usize,
    pub worlds: Vec<WorldInfo>,
}

#[derive(Serialize)]
pub struct WorldInfo {
    pub id: String,
    pub name: String,
    pub entity_count: usize,
}

pub async fn api_info(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let worlds_lock = state.worlds.read();
    let sessions_lock = state.sessions.read();

    let worlds: Vec<WorldInfo> = worlds_lock
        .world_ids()
        .into_iter()
        .filter_map(|id| {
            worlds_lock.get_world(&id).map(|w| WorldInfo {
                id: w.id.clone(),
                name: w.name.clone(),
                entity_count: w.entity_count(),
            })
        })
        .collect();

    Json(ServerInfo {
        version: env!("CARGO_PKG_VERSION"),
        uptime_secs: worlds_lock.uptime_secs(),
        total_sessions: sessions_lock.session_count(),
        total_entities: worlds_lock.total_entity_count(),
        worlds,
    })
}

#[derive(Serialize)]
pub struct WorldEntities {
    pub world_id: String,
    pub entities: Vec<EntitySnapshot>,
}

pub async fn api_world_entities(
    Path(world_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let worlds = state.worlds.read();
    if let Some(world) = worlds.get_world(&world_id) {
        Json(WorldEntities {
            world_id: world_id.clone(),
            entities: world.get_all_snapshots(),
        })
        .into_response()
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            format!("World '{}' not found", world_id),
        )
            .into_response()
    }
}

#[derive(Serialize, Deserialize)]
pub struct CreateWorldRequest {
    pub id: String,
    pub name: String,
}

pub async fn api_create_world(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateWorldRequest>,
) -> impl IntoResponse {
    let mut worlds = state.worlds.write();
    if worlds.create_world(&req.id, &req.name) {
        (axum::http::StatusCode::CREATED, format!("World '{}' created", req.id)).into_response()
    } else {
        (
            axum::http::StatusCode::CONFLICT,
            format!("World '{}' already exists", req.id),
        )
            .into_response()
    }
}
