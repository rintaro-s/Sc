//! CR-Bridge エラー型

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrBridgeError {
    #[error("IO エラー: {0}")]
    Io(#[from] std::io::Error),

    #[error("EKF 行列計算エラー: {0}")]
    EkfMatrix(String),

    #[error("パケット解析エラー: {0}")]
    PacketParse(String),

    #[error("OSC エラー: {0}")]
    Osc(String),

    #[error("SMSL 初期化エラー: {0}")]
    SmslInit(String),

    #[error("ブリッジエラー: {bridge} - {message}")]
    Bridge { bridge: String, message: String },

    #[error("タイムアウト: {0}")]
    Timeout(String),

    #[error("設定エラー: {0}")]
    Config(String),

    #[error("JSON エラー: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CrBridgeError>;
