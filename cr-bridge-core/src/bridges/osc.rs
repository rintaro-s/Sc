//! VRChat OSC Bridge
//!
//! VRChat の OSC (Open Sound Control) プロトコルを受信し、
//! ATP パケットに変換して ATPEngine に渡す。
//!
//! ## VRChat OSC エンドポイント
//! - ポート: 9001 (受信), 9000 (送信)
//! - アバターパラメータ: `/avatar/parameters/*`
//! - トラッキングデータ: `/tracking/trackers/*`
//!
//! ## 変換処理
//! OSC → FlatBuffers (ATPPacket) → SMSLへ書き込み

use std::net::SocketAddr;
use std::sync::Arc;

use rosc::{OscMessage, OscPacket, OscType};
use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

use crate::atp::engine::ATPEngine;
use crate::error::{CrBridgeError, Result};
use crate::types::{ATPPacket, BridgeType, EntityType, Quaternionf, Vec3f};

/// VRChat OSC ブリッジ設定
#[derive(Debug, Clone)]
pub struct OSCBridgeConfig {
    /// OSC 受信ポート（VRChatが送信してくるポート）
    pub listen_port: u16,
    /// OSC 送信ポート（VRChatが受信するポート）
    pub send_port: u16,
    /// VRChat ホスト
    pub vrchat_host: String,
    /// デバッグログを詳細に出力するか
    pub verbose: bool,
}

impl Default for OSCBridgeConfig {
    fn default() -> Self {
        Self {
            listen_port: 9001,
            send_port: 9000,
            vrchat_host: "127.0.0.1".to_string(),
            verbose: false,
        }
    }
}

/// VRChat アバターのOSC状態バッファ
///
/// バラバラに届く OSC メッセージを組み合わせて ATPPacket を構築する
#[derive(Debug, Default, Clone)]
struct AvatarOSCState {
    entity_id: u64,
    position: Option<Vec3f>,
    velocity: Option<Vec3f>,
    orientation: Option<Quaternionf>,
    sequence: u32,
}

impl AvatarOSCState {
    /// 完全な ATPPacket を組み立てられるか
    fn can_build_packet(&self) -> bool {
        self.position.is_some()
    }

    fn build_packet(&mut self) -> ATPPacket {
        self.sequence = self.sequence.wrapping_add(1);
        ATPPacket {
            entity_id: self.entity_id,
            timestamp_us: ATPPacket::now_us(),
            sequence: self.sequence,
            position: self.position.unwrap_or(Vec3f::ZERO),
            velocity: self.velocity.unwrap_or(Vec3f::ZERO),
            acceleration: Vec3f::ZERO,
            orientation: self.orientation.unwrap_or(Quaternionf::IDENTITY),
            angular_velocity: Vec3f::ZERO,
            imu_confidence: 0.8,
            entity_type: EntityType::VRAvatar,
            source_bridge: BridgeType::VRChatOSC,
        }
    }
}

/// VRChat OSC Bridge
///
/// UDPソケットで OSC パケットを受信し、ATPEngineに渡す。
pub struct VRChatOSCBridge {
    config: OSCBridgeConfig,
    engine: Arc<ATPEngine>,
}

impl VRChatOSCBridge {
    pub fn new(config: OSCBridgeConfig, engine: Arc<ATPEngine>) -> Self {
        Self { config, engine }
    }

    /// OSC 受信ループを起動（tokioタスクとして動作）
    pub async fn run(&self) -> Result<()> {
        let addr = format!("0.0.0.0:{}", self.config.listen_port);
        let socket = UdpSocket::bind(&addr).await.map_err(|e| {
            CrBridgeError::Bridge {
                bridge: "VRChat OSC".into(),
                message: format!("UDPバインド失敗: {}", e),
            }
        })?;

        info!("VRChat OSC Bridge: {} で受信待機中", addr);

        let mut buf = vec![0u8; 65536];
        let mut avatar_states: std::collections::HashMap<u64, AvatarOSCState> =
            std::collections::HashMap::new();

        loop {
            let (len, peer) = socket.recv_from(&mut buf).await.map_err(|e| {
                CrBridgeError::Io(e)
            })?;

            if let Err(e) = self.handle_osc_packet(
                &buf[..len],
                peer,
                &mut avatar_states,
            ) {
                if self.config.verbose {
                    warn!("OSC パケット処理エラー: {}", e);
                }
            }
        }
    }

    /// OSC パケットを処理して ATPEngine に渡す
    fn handle_osc_packet(
        &self,
        data: &[u8],
        peer: SocketAddr,
        states: &mut std::collections::HashMap<u64, AvatarOSCState>,
    ) -> Result<()> {
        let packet = rosc::decoder::decode_udp(data).map_err(|e| {
            CrBridgeError::Osc(format!("OSC デコード失敗: {:?}", e))
        })?;

        match packet.1 {
            OscPacket::Message(msg) => {
                self.handle_osc_message(msg, peer, states)?;
            }
            OscPacket::Bundle(bundle) => {
                for content in bundle.content {
                    if let OscPacket::Message(msg) = content {
                        let _ = self.handle_osc_message(msg, peer, states);
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_osc_message(
        &self,
        msg: OscMessage,
        peer: SocketAddr,
        states: &mut std::collections::HashMap<u64, AvatarOSCState>,
    ) -> Result<()> {
        // ピアアドレスからエンティティIDを生成（簡易実装）
        let entity_id = peer_to_entity_id(&peer);
        let state = states
            .entry(entity_id)
            .or_insert_with(|| AvatarOSCState {
                entity_id,
                ..Default::default()
            });

        if self.config.verbose {
            debug!("OSC: {} {:?}", msg.addr, msg.args);
        }

        // パスに応じてフィールドを更新
        if msg.addr.starts_with("/avatar/parameters/") {
            parse_avatar_parameter(&msg, state);
        } else if msg.addr.starts_with("/tracking/trackers/") {
            parse_tracking_data(&msg, state);
        } else if msg.addr == "/chatbox/input" {
            // チャット入力は無視
        } else if self.config.verbose {
            debug!("未知の OSC アドレス: {}", msg.addr);
        }

        // 位置情報が揃ったらパケットを送信
        if state.can_build_packet() {
            let packet = state.build_packet();
            self.engine.receive_packet(packet);
        }

        Ok(())
    }
}

/// アバターパラメータの解析
///
/// `/avatar/parameters/Position_x` などの形式を処理
fn parse_avatar_parameter(msg: &OscMessage, state: &mut AvatarOSCState) {
    let param = msg.addr.trim_start_matches("/avatar/parameters/");

    match param {
        "VelocityX" => {
            if let Some(OscType::Float(v)) = msg.args.first() {
                let vel = state.velocity.get_or_insert(Vec3f::ZERO);
                vel.x = *v;
            }
        }
        "VelocityY" => {
            if let Some(OscType::Float(v)) = msg.args.first() {
                let vel = state.velocity.get_or_insert(Vec3f::ZERO);
                vel.y = *v;
            }
        }
        "VelocityZ" => {
            if let Some(OscType::Float(v)) = msg.args.first() {
                let vel = state.velocity.get_or_insert(Vec3f::ZERO);
                vel.z = *v;
            }
        }
        "AngularY" => {
            // 角速度（Y軸回転）
            // ここでは無視（EKFのプロセスノイズで対応）
        }
        _ => {}
    }
}

/// トラッキングデータの解析
///
/// `/tracking/trackers/1/position` などの形式を処理
fn parse_tracking_data(msg: &OscMessage, state: &mut AvatarOSCState) {
    let parts: Vec<&str> = msg.addr.trim_start_matches("/tracking/trackers/").split('/').collect();
    if parts.len() < 2 {
        return;
    }

    match parts[1] {
        "position" => {
            if msg.args.len() >= 3 {
                if let (
                    Some(OscType::Float(x)),
                    Some(OscType::Float(y)),
                    Some(OscType::Float(z)),
                ) = (msg.args.get(0), msg.args.get(1), msg.args.get(2))
                {
                    state.position = Some(Vec3f::new(*x, *y, *z));
                }
            }
        }
        "rotation" => {
            if msg.args.len() >= 4 {
                if let (
                    Some(OscType::Float(x)),
                    Some(OscType::Float(y)),
                    Some(OscType::Float(z)),
                    Some(OscType::Float(w)),
                ) = (
                    msg.args.get(0),
                    msg.args.get(1),
                    msg.args.get(2),
                    msg.args.get(3),
                ) {
                    state.orientation = Some(Quaternionf::new(*w, *x, *y, *z));
                }
            }
        }
        _ => {}
    }
}

/// ピアアドレスからエンティティIDを生成（FNV-1aハッシュ風）
fn peer_to_entity_id(peer: &SocketAddr) -> u64 {
    let ip: u64 = match peer.ip() {
        std::net::IpAddr::V4(v4) => {
            let octets = v4.octets();
            u64::from_be_bytes([octets[0], octets[1], octets[2], octets[3], 0, 0, 0, 0])
        }
        std::net::IpAddr::V6(v6) => {
            let octets = v6.octets();
            u64::from_be_bytes([
                octets[0], octets[1], octets[2], octets[3],
                octets[4], octets[5], octets[6], octets[7],
            ])
        }
    };
    ip ^ (peer.port() as u64 * 0x9e3779b97f4a7c15)
}
