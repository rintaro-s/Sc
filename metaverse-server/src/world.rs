//! ワールド管理
//!
//! World / Entity の CRUD と Interest Management を担当。

use std::collections::HashMap;
use crate::protocol::EntitySnapshot;

/// ワールド内のエンティティ（接続ユーザー）
pub struct Entity {
    pub id: String,
    pub display_name: String,
    pub avatar_color: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub velocity: [f32; 3],
    pub last_update_ms: u64,
}

impl Entity {
    pub fn snapshot(&self) -> EntitySnapshot {
        EntitySnapshot {
            entity_id: self.id.clone(),
            display_name: self.display_name.clone(),
            avatar_color: self.avatar_color.clone(),
            position: self.position,
            rotation: self.rotation,
            velocity: self.velocity,
            last_update_ms: self.last_update_ms,
        }
    }
}

/// 1つのワールド（ルーム相当）
pub struct World {
    pub id: String,
    pub name: String,
    /// Interest Management 半径（この距離内のエンティティ更新だけ受け取る）
    pub interest_radius: f32,
    entities: HashMap<String, Entity>,
}

impl World {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            interest_radius: 200.0, // 200 m
            entities: HashMap::new(),
        }
    }

    pub fn add_entity(&mut self, entity: Entity) {
        self.entities.insert(entity.id.clone(), entity);
    }

    pub fn remove_entity(&mut self, entity_id: &str) -> bool {
        self.entities.remove(entity_id).is_some()
    }

    pub fn update_pose(
        &mut self,
        entity_id: &str,
        position: [f32; 3],
        rotation: [f32; 4],
        velocity: [f32; 3],
        timestamp_ms: u64,
    ) -> bool {
        if let Some(e) = self.entities.get_mut(entity_id) {
            e.position = position;
            e.rotation = rotation;
            e.velocity = velocity;
            e.last_update_ms = timestamp_ms;
            true
        } else {
            false
        }
    }

    pub fn get_all_snapshots(&self) -> Vec<EntitySnapshot> {
        self.entities.values().map(|e| e.snapshot()).collect()
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Interest Management: sender 以外で半径内のエンティティ ID 一覧
    pub fn interested_entity_ids(&self, from_pos: [f32; 3], exclude_id: &str) -> Vec<String> {
        let r2 = self.interest_radius * self.interest_radius;
        self.entities
            .iter()
            .filter(|(id, e)| {
                if id.as_str() == exclude_id {
                    return false;
                }
                let dx = e.position[0] - from_pos[0];
                let dy = e.position[1] - from_pos[1];
                let dz = e.position[2] - from_pos[2];
                dx * dx + dy * dy + dz * dz <= r2
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn has_entity(&self, entity_id: &str) -> bool {
        self.entities.contains_key(entity_id)
    }
}

// ─────────────────────────────────────────────────────────────
// WorldManager
// ─────────────────────────────────────────────────────────────

pub struct WorldManager {
    worlds: HashMap<String, World>,
    created_at: std::time::Instant,
}

impl WorldManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            worlds: HashMap::new(),
            created_at: std::time::Instant::now(),
        };
        mgr.create_world("default", "CR-World");
        mgr.create_world("arena", "CR-Arena");
        mgr
    }

    pub fn create_world(&mut self, id: &str, name: &str) -> bool {
        if self.worlds.contains_key(id) {
            return false;
        }
        self.worlds.insert(id.to_string(), World::new(id, name));
        true
    }

    pub fn get_world(&self, id: &str) -> Option<&World> {
        self.worlds.get(id)
    }

    pub fn get_world_mut(&mut self, id: &str) -> Option<&mut World> {
        self.worlds.get_mut(id)
    }

    pub fn world_ids(&self) -> Vec<String> {
        self.worlds.keys().cloned().collect()
    }

    pub fn uptime_secs(&self) -> u64 {
        self.created_at.elapsed().as_secs()
    }

    pub fn total_entity_count(&self) -> usize {
        self.worlds.values().map(|w| w.entity_count()).sum()
    }
}
