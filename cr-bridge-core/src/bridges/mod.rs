//! Protocol Bridges モジュール
//!
//! 既存サービスを CR-Bridge に接続するアダプター群。
//!
//! - [`osc`] - VRChat OSC Bridge
//! - [`sns`] - SNS Spatial Bridge（Twitter/X, Instagram）
//! - [`crs`] - Blender/Unity/Three.js の絶対座標(CAS) Bridge

pub mod crs;
pub mod osc;
pub mod sns;

pub use crs::CRSBridge;
pub use osc::VRChatOSCBridge;
pub use sns::SNSSpatialBridge;
