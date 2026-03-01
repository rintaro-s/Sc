//! S2 Geometry 近似実装
//!
//! Google S2 Geometry ライブラリの概念を Rust でシンプルに実装。
//! 地球全体を階層的なセルで分割し、O(log N) で空間クエリを実行。
//!
//! 本実装は s2 クレートへの移行を前提とした簡易版。

use std::collections::HashMap;

/// S2 セル ID（64ビット、階層的位置エンコード）
///
/// 実際の S2 ライブラリでは Hilbert 曲線を使った64ビット整数だが、
/// ここでは経緯度グリッドの簡易実装を使う
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct S2CellApprox {
    /// レベル 0〜20（細かさ）
    pub level: u8,
    /// 緯度インデックス
    pub lat_idx: i32,
    /// 経度インデックス
    pub lon_idx: i32,
}

impl S2CellApprox {
    /// 緯度経度からセルを生成（level が大きいほど細かい）
    pub fn from_latlng(lat: f64, lng: f64, level: u8) -> Self {
        let grid = (1 << level) as f64;
        let lat_idx = ((lat + 90.0) / 180.0 * grid) as i32;
        let lon_idx = ((lng + 180.0) / 360.0 * grid) as i32;
        Self { level, lat_idx, lon_idx }
    }

    /// 3D 座標（メートル）からセルを生成（仮想空間向け）
    pub fn from_xyz(x: f32, y: f32, z: f32, level: u8) -> Self {
        let scale = 100.0f64; // 100m グリッド（level=0）
        let grid_size = scale / (1 << level) as f64;
        let lat_idx = (x as f64 / grid_size) as i32;
        let lon_idx = (z as f64 / grid_size) as i32;
        Self { level, lat_idx, lon_idx }
    }

    /// 親セル（一段粗いレベル）
    pub fn parent(&self) -> Option<Self> {
        if self.level == 0 {
            return None;
        }
        Some(Self {
            level: self.level - 1,
            lat_idx: self.lat_idx >> 1,
            lon_idx: self.lon_idx >> 1,
        })
    }

    /// 隣接セルを返す（上下左右斜め8方向）
    pub fn neighbors(&self) -> Vec<S2CellApprox> {
        let mut result = Vec::with_capacity(8);
        for dlat in -1i32..=1 {
            for dlon in -1i32..=1 {
                if dlat == 0 && dlon == 0 {
                    continue;
                }
                result.push(Self {
                    level: self.level,
                    lat_idx: self.lat_idx + dlat,
                    lon_idx: self.lon_idx + dlon,
                });
            }
        }
        result
    }

    /// 64ビット整数表現（マップキーとして使用）
    pub fn to_u64(&self) -> u64 {
        let level = self.level as u64;
        let lat = (self.lat_idx as i64 + 0x80000) as u64;
        let lon = (self.lon_idx as i64 + 0x80000) as u64;
        (level << 40) | (lat << 20) | lon
    }
}

/// 空間インデックス
///
/// エンティティを S2 セルで管理し、視錐台クエリを高速化する
pub struct SpatialIndex {
    /// セルID → エンティティIDリスト
    cell_to_entities: HashMap<u64, Vec<u64>>,
    /// エンティティID → セルID
    entity_to_cell: HashMap<u64, u64>,
    /// インデックスレベル（精度）
    level: u8,
}

impl SpatialIndex {
    pub fn new(level: u8) -> Self {
        Self {
            cell_to_entities: HashMap::new(),
            entity_to_cell: HashMap::new(),
            level,
        }
    }

    /// エンティティを位置とともに登録
    pub fn insert(&mut self, entity_id: u64, x: f32, y: f32, z: f32) {
        let cell = S2CellApprox::from_xyz(x, y, z, self.level);
        let cell_id = cell.to_u64();

        // 古いセルから削除
        if let Some(old_cell_id) = self.entity_to_cell.get(&entity_id) {
            if let Some(entities) = self.cell_to_entities.get_mut(old_cell_id) {
                entities.retain(|&id| id != entity_id);
            }
        }

        // 新しいセルに登録
        self.cell_to_entities
            .entry(cell_id)
            .or_default()
            .push(entity_id);
        self.entity_to_cell.insert(entity_id, cell_id);
    }

    /// エンティティを削除
    pub fn remove(&mut self, entity_id: u64) {
        if let Some(cell_id) = self.entity_to_cell.remove(&entity_id) {
            if let Some(entities) = self.cell_to_entities.get_mut(&cell_id) {
                entities.retain(|&id| id != entity_id);
            }
        }
    }

    /// 指定範囲（矩形）内のエンティティを返す（視錐台クエリの簡易版）
    pub fn query_range(
        &self,
        center_x: f32,
        center_z: f32,
        range: f32,
    ) -> Vec<u64> {
        let cell_center = S2CellApprox::from_xyz(center_x, 0.0, center_z, self.level);
        let range_cells = (range / 100.0 * (1 << self.level) as f32) as i32 + 1;

        let mut result = Vec::new();
        for dlat in -range_cells..=range_cells {
            for dlon in -range_cells..=range_cells {
                let query_cell = S2CellApprox {
                    level: self.level,
                    lat_idx: cell_center.lat_idx + dlat,
                    lon_idx: cell_center.lon_idx + dlon,
                };
                let cell_id = query_cell.to_u64();
                if let Some(entities) = self.cell_to_entities.get(&cell_id) {
                    result.extend_from_slice(entities);
                }
            }
        }
        result
    }

    /// インデックスに登録されているエンティティ総数
    pub fn len(&self) -> usize {
        self.entity_to_cell.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entity_to_cell.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_index_insert_query() {
        let mut idx = SpatialIndex::new(8);

        idx.insert(1, 0.0, 0.0, 0.0);
        idx.insert(2, 1.0, 0.0, 1.0);
        idx.insert(3, 100.0, 0.0, 100.0); // 遠い

        let near = idx.query_range(0.5, 0.5, 5.0);
        assert!(near.contains(&1), "エンティティ1が見つからない");
        assert!(near.contains(&2), "エンティティ2が見つからない");

        let far_check = idx.query_range(100.0, 100.0, 2.0);
        assert!(far_check.contains(&3), "エンティティ3が見つからない");
    }

    #[test]
    fn test_spatial_index_update() {
        let mut idx = SpatialIndex::new(8);
        idx.insert(1, 0.0, 0.0, 0.0);

        // 位置を更新
        idx.insert(1, 200.0, 0.0, 200.0);

        let old_area = idx.query_range(0.0, 0.0, 1.0);
        assert!(!old_area.contains(&1), "古い位置にいてはいけない");
    }
}
