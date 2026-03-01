//! Anti-Teleport Protocol (ATP) モジュール
//!
//! パケットロス時のテレポートをアーキテクチャレベルで禁止する予測同期エンジン。
//!
//! ## 処理パイプライン
//!
//! 1. パケット受信 → [`engine::ATPEngine`] が管理
//! 2. パケット欠落検知 → [`dead_reckoning`] で物理予測を実行
//! 3. 拡張カルマンフィルタ [`ekf::EKF`] が状態推定を更新
//! 4. 次パケット到着時 → [`hermite::HermiteInterpolator`] で C1連続補間

pub mod dead_reckoning;
pub mod ekf;
pub mod engine;
pub mod hermite;
pub mod packet;

pub use engine::ATPEngine;
pub use ekf::{EKFConfig, EKF};
pub use packet::ATPSession;
