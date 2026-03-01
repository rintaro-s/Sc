//! Spatial Math Accelerator (SMA) モジュール
//!
//! SIMD/FMA/NPU による高速空間演算ライブラリ。
//!
//! ## 実装優先順位
//! 1. AVX-512 FMA (x86_64, CPUが対応している場合)
//! 2. NEON (ARM/Android)
//! 3. スカラーフォールバック（全環境で動作保証）

mod transform;

pub use transform::{
    batch_view_transform, kalman_gain_scalar, SIMDInfo,
};
