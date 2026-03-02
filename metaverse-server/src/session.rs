//! セッション管理
//!
//! 各 WebSocket 接続 = 1 Session。

use std::collections::HashMap;
use tokio::sync::mpsc;
use crate::protocol::ServerMessage;

pub struct Session {
    pub id: String,
    pub user_id: String,
    pub display_name: String,
    pub world_id: Option<String>,
    /// JSON 文字列を書き込むチャネル
    pub tx: mpsc::UnboundedSender<String>,
}

pub struct SessionManager {
    sessions: HashMap<String, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self { sessions: HashMap::new() }
    }

    pub fn add(&mut self, session: Session) {
        self.sessions.insert(session.id.clone(), session);
    }

    pub fn remove(&mut self, session_id: &str) -> Option<Session> {
        self.sessions.remove(session_id)
    }

    pub fn get_mut(&mut self, session_id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(session_id)
    }

    // ── 読み取りヘルパー ──────────────────────────────────────

    /// (user_id, world_id) を返す
    pub fn get_user_world(&self, session_id: &str) -> Option<(String, String)> {
        self.sessions.get(session_id).and_then(|s| {
            s.world_id.as_ref().map(|w| (s.user_id.clone(), w.clone()))
        })
    }

    /// (user_id, display_name, world_id) を返す
    pub fn get_display_and_world(&self, session_id: &str) -> Option<(String, String, String)> {
        self.sessions.get(session_id).and_then(|s| {
            s.world_id.as_ref().map(|w| {
                (s.user_id.clone(), s.display_name.clone(), w.clone())
            })
        })
    }

    // ── 送信ヘルパー ──────────────────────────────────────────

    /// 指定セッションへ送信
    pub fn send(&self, session_id: &str, msg: &ServerMessage) {
        if let Some(session) = self.sessions.get(session_id) {
            if let Ok(json) = serde_json::to_string(msg) {
                let _ = session.tx.send(json);
            }
        }
    }

    /// world 内の全セッションへブロードキャスト
    pub fn broadcast_world(&self, world_id: &str, msg: &ServerMessage, exclude: Option<&str>) {
        if let Ok(json) = serde_json::to_string(msg) {
            for session in self.sessions.values() {
                if session.world_id.as_deref() == Some(world_id) {
                    if exclude.map_or(true, |ex| ex != session.id) {
                        let _ = session.tx.send(json.clone());
                    }
                }
            }
        }
    }

    /// world 内の指定 entity_id リストのセッションへのみ送信
    pub fn send_to_entity_ids(&self, world_id: &str, entity_ids: &[String], msg: &ServerMessage) {
        if let Ok(json) = serde_json::to_string(msg) {
            for session in self.sessions.values() {
                if session.world_id.as_deref() == Some(world_id)
                    && entity_ids.contains(&session.user_id)
                {
                    let _ = session.tx.send(json.clone());
                }
            }
        }
    }

    // ── 統計 ─────────────────────────────────────────────────

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn sessions_in_world(&self, world_id: &str) -> usize {
        self.sessions
            .values()
            .filter(|s| s.world_id.as_deref() == Some(world_id))
            .count()
    }
}
