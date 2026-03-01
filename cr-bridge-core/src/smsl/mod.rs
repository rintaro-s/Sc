//! Shared Memory Spatial Ledger (SMSL) モジュール
//!
//! ネットワーク受信から AR 描画エンジンまで、データが一切コピーされない
//! 経路を実現するインメモリ空間データベース。
//!
//! ## 設計原則
//! - SeqLock による単一ライター・複数リーダー（mutex なし）
//! - SoA（Structure of Arrays）レイアウトで SIMD 処理を最適化
//! - S2 Geometry セルによる空間インデックス（視錐台クエリ O(log N)）

pub mod entity_store;
pub mod ledger;
pub mod seqlock;
pub mod spatial_index;

pub use ledger::SpatialLedger;
pub use seqlock::SeqLock;
pub use spatial_index::{S2CellApprox, SpatialIndex};
