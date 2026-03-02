// metaverse-server/src/storage.rs
//! 永続ストレージモジュール (SQLite)

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use tracing::{debug, info};

use crate::world::{Entity, World};

pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// データベースを開く/作成
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        
        // テーブル作成
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS worlds (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS entities (
                entity_id TEXT PRIMARY KEY,
                world_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                avatar_color TEXT NOT NULL,
                position_x REAL NOT NULL,
                position_y REAL NOT NULL,
                position_z REAL NOT NULL,
                rotation_x REAL NOT NULL,
                rotation_y REAL NOT NULL,
                rotation_z REAL NOT NULL,
                rotation_w REAL NOT NULL,
                velocity_x REAL NOT NULL,
                velocity_y REAL NOT NULL,
                velocity_z REAL NOT NULL,
                last_update_ms INTEGER NOT NULL,
                FOREIGN KEY(world_id) REFERENCES worlds(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_entities_world ON entities(world_id);
            CREATE INDEX IF NOT EXISTS idx_entities_updated ON entities(last_update_ms);
            "#,
        )?;

        info!("Storage database initialized");
        Ok(Self { conn })
    }

    /// ワールドを保存
    pub fn save_world(&self, world: &World) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;

        self.conn.execute(
            "INSERT OR REPLACE INTO worlds (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![&world.id, &world.name, now, now],
        )?;

        debug!("World saved: {}", world.id);
        Ok(())
    }

    /// エンティティを保存
    pub fn save_entity(&self, world_id: &str, entity: &Entity) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO entities 
            (entity_id, world_id, display_name, avatar_color,
             position_x, position_y, position_z,
             rotation_x, rotation_y, rotation_z, rotation_w,
             velocity_x, velocity_y, velocity_z,
             last_update_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
            "#,
            params![
                &entity.id,
                world_id,
                &entity.display_name,
                &entity.avatar_color,
                entity.position[0],
                entity.position[1],
                entity.position[2],
                entity.rotation[0],
                entity.rotation[1],
                entity.rotation[2],
                entity.rotation[3],
                entity.velocity[0],
                entity.velocity[1],
                entity.velocity[2],
                entity.last_update_ms,
            ],
        )?;

        debug!("Entity saved: {} in world {}", entity.id, world_id);
        Ok(())
    }

    /// ワールドをロード
    pub fn load_world(&self, world_id: &str) -> Result<World> {
        let mut stmt = self.conn.prepare("SELECT id, name FROM worlds WHERE id = ?1")?;
        let (id, name): (String, String) = stmt.query_row(params![world_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let world = World::new(id, name);
        info!("World loaded: {}", world_id);
        Ok(world)
    }

    /// ワールド内の全エンティティをロード
    pub fn load_entities(&self, world_id: &str) -> Result<Vec<Entity>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT entity_id, display_name, avatar_color,
                   position_x, position_y, position_z,
                   rotation_x, rotation_y, rotation_z, rotation_w,
                   velocity_x, velocity_y, velocity_z,
                   last_update_ms
            FROM entities WHERE world_id = ?1
            "#,
        )?;

        let entities = stmt
            .query_map(params![world_id], |row| {
                Ok(Entity {
                    id: row.get(0)?,
                    display_name: row.get(1)?,
                    avatar_color: row.get(2)?,
                    position: [row.get(3)?, row.get(4)?, row.get(5)?],
                    rotation: [row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?],
                    velocity: [row.get(10)?, row.get(11)?, row.get(12)?],
                    last_update_ms: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        info!("Loaded {} entities from world {}", entities.len(), world_id);
        Ok(entities)
    }

    /// エンティティを削除
    pub fn delete_entity(&self, entity_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM entities WHERE entity_id = ?1", params![entity_id])?;
        debug!("Entity deleted: {}", entity_id);
        Ok(())
    }

    /// ワールドを削除 (cascadeでエンティティも削除)
    pub fn delete_world(&self, world_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM worlds WHERE id = ?1", params![world_id])?;
        info!("World deleted: {}", world_id);
        Ok(())
    }

    /// 全ワールドIDを取得
    pub fn list_worlds(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT id FROM worlds")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    /// 古いエンティティをクリーンアップ (TTL)
    pub fn cleanup_stale_entities(&self, ttl_ms: i64) -> Result<usize> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as i64;
        
        let cutoff = now - ttl_ms;
        
        let deleted = self.conn.execute(
            "DELETE FROM entities WHERE last_update_ms < ?1",
            params![cutoff],
        )?;

        if deleted > 0 {
            info!("Cleaned up {} stale entities", deleted);
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_workflow() {
        let storage = Storage::open(":memory:").unwrap();
        
        let world = World::new("test", "Test World");
        
        storage.save_world(&world).unwrap();
        
        let entity = Entity {
            id: "entity1".to_string(),
            display_name: "TestUser".to_string(),
            avatar_color: "#ff0000".to_string(),
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0, 0.0, 0.0],
            last_update_ms: 1000,
        };
        
        storage.save_entity("test", &entity).unwrap();
        
        let loaded_entities = storage.load_entities("test").unwrap();
        assert_eq!(loaded_entities.len(), 1);
        assert_eq!(loaded_entities[0].id, "entity1");
    }
}