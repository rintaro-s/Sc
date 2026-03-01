//! CR-Bridge デモメタバース — サーバー本体
//!
//! ## アーキテクチャ
//! - tokio::sync::broadcast で全クライアントに同一フレームデータを送信
//! - VRChat OSC (UDP :9001) を直接受信して ATPEngine に投入
//! - WebSocket (/ws) でブラウザクライアントに 60fps 配信
//! - HTTP (/) で Three.js フロントエンド HTML を配信

use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use rosc::{OscPacket, OscType};
use tokio::{net::UdpSocket, sync::broadcast, time::interval};
use tower_http::cors::CorsLayer;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

mod simulation;
use simulation::MetaverseSimulation;

use cr_bridge_core::atp::engine::ATPEngine;
use cr_bridge_core::types::{ATPPacket, BridgeType, EntityType, Quaternionf, Vec3f};

#[derive(Clone)]
struct AppState {
    sim: Arc<MetaverseSimulation>,
    tx: Arc<broadcast::Sender<Arc<String>>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let sim = Arc::new(MetaverseSimulation::new());
    let (tx, _) = broadcast::channel::<Arc<String>>(128);
    let tx = Arc::new(tx);

    // タスク1: 物理 + ブロードキャストループ (60fps)
    {
        let sim = Arc::clone(&sim);
        let tx = Arc::clone(&tx);
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_millis(16));
            let dt = 1.0f32 / 60.0;
            loop {
                ticker.tick().await;
                sim.step(dt);
                let frame = sim.get_frame();
                if let Ok(json) = serde_json::to_string(&frame) {
                    let _ = tx.send(Arc::new(json));
                }
            }
        });
    }

    // タスク2: VRChat OSC 受信 (UDP :9001)
    {
        let engine = sim.get_engine();
        tokio::spawn(async move {
            osc_listener(engine).await;
        });
    }

    let state = AppState { sim, tx };
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/ws", get(ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("ポート 3000 でバインドできませんでした");

    banner();
    axum::serve(listener, app).await.unwrap();
}

fn banner() {
    println!();
    println!("  \x1b[36m╔══════════════════════════════════════════════════════════╗\x1b[0m");
    println!("  \x1b[36m║\x1b[0m  \x1b[1mCR-Bridge Demo Metaverse\x1b[0m  —  ATP v1 リアルタイムデモ   \x1b[36m║\x1b[0m");
    println!("  \x1b[36m╚══════════════════════════════════════════════════════════╝\x1b[0m");
    println!();
    println!("  ブラウザで開く:  \x1b[33mhttp://localhost:3000\x1b[0m");
    println!();
    println!("  [Left]  \x1b[31m素朴実装\x1b[0m    — パケットロス時にテレポート発生");
    println!("  [Right] \x1b[32mCR-Bridge ATP\x1b[0m — EKF で慣性予測、テレポートなし");
    println!();
    println!("  VRChat OSC: \x1b[90mUDP :9001 で待受中\x1b[0m");
    println!("  複数ブラウザタブを開くとマルチクライアント動作確認できます");
    println!();
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(ws: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = ws.split();
    let mut rx = state.tx.subscribe();
    let sim = Arc::clone(&state.sim);

    info!("WebSocket クライアント接続");

    let send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if sender.send(Message::Text((*msg).clone())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("クライアントが {} フレームをスキップ", n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                    handle_command(&cmd, &sim);
                }
            }
        }
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }
    info!("WebSocket クライアント切断");
}

fn handle_command(cmd: &serde_json::Value, sim: &MetaverseSimulation) {
    match cmd.get("action").and_then(|v| v.as_str()) {
        Some("set_packet_loss") => {
            if let Some(r) = cmd.get("rate").and_then(|v| v.as_f64()) {
                sim.set_packet_loss_rate(r as f32);
                info!("パケットロス率 → {:.0}%", r * 100.0);
            }
        }
        Some("reset") => sim.reset(),
        Some("add_avatar") => sim.add_random_avatar(),
        _ => {}
    }
}

async fn osc_listener(engine: Arc<ATPEngine>) {
    match UdpSocket::bind("0.0.0.0:9001").await {
        Ok(sock) => {
            info!("VRChat OSC: UDP :9001 待受開始");
            let mut buf = vec![0u8; 4096];
            loop {
                let Ok((n, _addr)) = sock.recv_from(&mut buf).await else { continue };
                let Ok((_, pkt)) = rosc::decoder::decode_udp(&buf[..n]) else { continue };
                dispatch_osc(pkt, &engine);
            }
        }
        Err(e) => {
            warn!("OSC :9001 バインド失敗 (VRChat 連携なし): {}", e);
        }
    }
}

fn dispatch_osc(pkt: OscPacket, engine: &ATPEngine) {
    match pkt {
        OscPacket::Message(msg) => {
            if let Some(atp) = parse_vrc_osc(&msg) {
                engine.receive_packet(atp);
            }
        }
        OscPacket::Bundle(b) => {
            for inner in b.content {
                dispatch_osc(inner, engine);
            }
        }
    }
}

fn parse_vrc_osc(msg: &rosc::OscMessage) -> Option<ATPPacket> {
    let parts: Vec<&str> = msg.addr.trim_start_matches('/').split('/').collect();
    let scale = 100.0f32; // VRChat メートル → デモワールド単位

    let entity_id: u64 = if parts.len() >= 4
        && parts[0] == "tracking"
        && parts[1] == "trackers"
        && parts[3] == "position"
    {
        90000 + parts[2].parse::<u64>().ok()?
    } else if parts.len() >= 3
        && parts[0] == "tracking"
        && parts[1] == "head"
        && parts[2] == "position"
    {
        90100
    } else {
        return None;
    };

    let x = osc_f32(&msg.args, 0)?;
    let y = osc_f32(&msg.args, 1)?;
    let z = osc_f32(&msg.args, 2)?;

    Some(ATPPacket {
        entity_id,
        timestamp_us: ATPPacket::now_us(),
        sequence: 0,
        position: Vec3f::new(x * scale, y * scale, z * scale),
        velocity: Vec3f::ZERO,
        acceleration: Vec3f::ZERO,
        orientation: Quaternionf::IDENTITY,
        angular_velocity: Vec3f::ZERO,
        imu_confidence: 1.0,
        entity_type: EntityType::VRAvatar,
        source_bridge: BridgeType::VRChatOSC,
    })
}

fn osc_f32(args: &[OscType], i: usize) -> Option<f32> {
    match args.get(i)? {
        OscType::Float(f) => Some(*f),
        OscType::Double(d) => Some(*d as f32),
        OscType::Int(n) => Some(*n as f32),
        _ => None,
    }
}
