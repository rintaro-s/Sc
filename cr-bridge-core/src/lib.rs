//! CR-Bridge Core Library
//!
//! クロスリアリティ低レイヤー基盤ミドルウェアのコアライブラリ。
//!
//! ## アーキテクチャ
//!
//! - [`atp`] - Anti-Teleport Protocol: EKF + デッドレコニング + Hermite補間
//! - [`smsl`] - Shared Memory Spatial Ledger: ゼロコピー空間データベース
//! - [`sma`] - Spatial Math Accelerator: SIMD/FMA 高速演算
//! - [`bridges`] - Protocol Bridges: VRChat OSC / SNS アダプター群

pub mod atp;
pub mod bridges;
pub mod error;
pub mod sma;
pub mod smsl;
pub mod types;

pub use error::{CrBridgeError, Result};
pub use types::*;
