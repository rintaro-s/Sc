//! Protocol Bridges モジュール
//!
//! 既存サービスを CR-Bridge に接続するアダプター群。
//!
//! - [`osc`] - VRChat OSC Bridge
//! - [`sns`] - SNS Spatial Bridge（Twitter/X, Instagram）

pub mod osc;
pub mod sns;

pub use osc::VRChatOSCBridge;
pub use sns::SNSSpatialBridge;
