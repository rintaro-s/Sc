//! アプリケーション共有状態

use std::sync::Arc;
use parking_lot::RwLock;
use crate::world::WorldManager;
use crate::session::SessionManager;

pub struct AppState {
    pub worlds: RwLock<WorldManager>,
    pub sessions: RwLock<SessionManager>,
}

impl AppState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            worlds: RwLock::new(WorldManager::new()),
            sessions: RwLock::new(SessionManager::new()),
        })
    }
}
