//! エンティティストア - SoA (Structure of Arrays) レイアウト
//!
//! SIMD処理時のストライドアクセスを排除するため、
//! AoS (Array of Structures) ではなく SoA レイアウトを採用。
//!
//! ```text
//! SoA:
//!   [entity_id_0, entity_id_1, ...entity_id_N]
//!   [pos_x_0, pos_x_1, ...pos_x_N]
//!   [pos_y_0, pos_y_1, ...pos_y_N]
//!   [pos_z_0, pos_z_1, ...pos_z_N]
//!   ...
//! ```
//!
//! これにより AVX-512 で16エンティティ分の x 座標を1命令で処理できる。

use std::collections::HashMap;

/// SoA レイアウトのエンティティストア
///
/// 最大 MAX_ENTITIES エンティティを管理する。
pub struct EntityStore {
    /// entity_id → インデックスのマッピング
    id_to_idx: HashMap<u64, usize>,
    /// インデックス → entity_id
    idx_to_id: Vec<u64>,

    // SoA レイアウト（SIMD読み出し最適化）
    /// 位置 X 配列
    pub pos_x: Vec<f32>,
    /// 位置 Y 配列
    pub pos_y: Vec<f32>,
    /// 位置 Z 配列
    pub pos_z: Vec<f32>,
    /// 速度 X 配列
    pub vel_x: Vec<f32>,
    /// 速度 Y 配列
    pub vel_y: Vec<f32>,
    /// 速度 Z 配列
    pub vel_z: Vec<f32>,
    /// クォータニオン W
    pub quat_w: Vec<f32>,
    /// クォータニオン X
    pub quat_x: Vec<f32>,
    /// クォータニオン Y
    pub quat_y: Vec<f32>,
    /// クォータニオン Z
    pub quat_z: Vec<f32>,
    /// タイムスタンプ（us）
    pub timestamps: Vec<u64>,

    capacity: usize,
    count: usize,
}

impl EntityStore {
    pub fn new(capacity: usize) -> Self {
        let zero_f = vec![0.0f32; capacity];
        let zero_u = vec![0u64; capacity];
        let zero_id = vec![0u64; capacity];

        Self {
            id_to_idx: HashMap::with_capacity(capacity),
            idx_to_id: zero_id,
            pos_x: zero_f.clone(),
            pos_y: zero_f.clone(),
            pos_z: zero_f.clone(),
            vel_x: zero_f.clone(),
            vel_y: zero_f.clone(),
            vel_z: zero_f.clone(),
            quat_w: vec![1.0f32; capacity],
            quat_x: zero_f.clone(),
            quat_y: zero_f.clone(),
            quat_z: zero_f.clone(),
            timestamps: zero_u,
            capacity,
            count: 0,
        }
    }

    /// エンティティをストアに追加または更新
    pub fn upsert(
        &mut self,
        entity_id: u64,
        px: f32, py: f32, pz: f32,
        vx: f32, vy: f32, vz: f32,
        qw: f32, qx: f32, qy: f32, qz: f32,
        timestamp_us: u64,
    ) -> bool {
        let idx = if let Some(&existing_idx) = self.id_to_idx.get(&entity_id) {
            existing_idx
        } else {
            if self.count >= self.capacity {
                return false; // 容量超過
            }
            let new_idx = self.count;
            self.id_to_idx.insert(entity_id, new_idx);
            self.idx_to_id[new_idx] = entity_id;
            self.count += 1;
            new_idx
        };

        self.pos_x[idx] = px;
        self.pos_y[idx] = py;
        self.pos_z[idx] = pz;
        self.vel_x[idx] = vx;
        self.vel_y[idx] = vy;
        self.vel_z[idx] = vz;
        self.quat_w[idx] = qw;
        self.quat_x[idx] = qx;
        self.quat_y[idx] = qy;
        self.quat_z[idx] = qz;
        self.timestamps[idx] = timestamp_us;

        true
    }

    /// エンティティの座標を取得
    pub fn get_position(&self, entity_id: u64) -> Option<(f32, f32, f32)> {
        let &idx = self.id_to_idx.get(&entity_id)?;
        Some((self.pos_x[idx], self.pos_y[idx], self.pos_z[idx]))
    }

    /// 有効なエンティティ数
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// SoA スライスを直接返す（SIMD処理向け）
    pub fn position_slices(&self) -> (&[f32], &[f32], &[f32]) {
        (
            &self.pos_x[..self.count],
            &self.pos_y[..self.count],
            &self.pos_z[..self.count],
        )
    }

    /// 全エンティティIDを返す
    pub fn entity_ids(&self) -> &[u64] {
        &self.idx_to_id[..self.count]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_store_upsert_get() {
        let mut store = EntityStore::new(100);
        assert!(store.upsert(1, 1.0, 2.0, 3.0, 0.1, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0));
        let pos = store.get_position(1).unwrap();
        assert!((pos.0 - 1.0).abs() < 1e-5);
        assert!((pos.1 - 2.0).abs() < 1e-5);
        assert!((pos.2 - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_entity_store_update() {
        let mut store = EntityStore::new(100);
        store.upsert(1, 1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0);
        store.upsert(1, 4.0, 5.0, 6.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0);
        assert_eq!(store.len(), 1); // 更新なので増えない
        let pos = store.get_position(1).unwrap();
        assert!((pos.0 - 4.0).abs() < 1e-5);
    }

    #[test]
    fn test_entity_store_soa_slices() {
        let mut store = EntityStore::new(100);
        for i in 0..5u64 {
            store.upsert(i, i as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0);
        }
        let (px, _, _) = store.position_slices();
        assert_eq!(px.len(), 5);
        // x座標が 0,1,2,3,4 の順に格納されている
        for i in 0..5 {
            assert!((px[i] - i as f32).abs() < 1e-5, "px[{}]={}", i, px[i]);
        }
    }
}
