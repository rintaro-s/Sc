//! CR-Bridge System Daemon
//!
//! カーネル〜ユーザーランド一気通貫の
//! クロスリアリティ低レイヤー基盤ミドルウェアデーモン。
//!
//! ## 起動するサービス
//! 1. ATP Engine（EKF + デッドレコニング）
//! 2. VRChat OSC Bridge（ポート9001）
//! 3. SMSL（Shared Memory Spatial Ledger）
//! 4. 統計ログ（1秒ごと）

use std::sync::Arc;
use std::time::Duration;
use tokio::{net::UdpSocket, time::interval};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use cr_bridge_core::atp::ekf::EKFConfig;
use cr_bridge_core::atp::engine::ATPEngine;
use cr_bridge_core::bridges::CRSBridge;
use cr_bridge_core::bridges::osc::{OSCBridgeConfig, VRChatOSCBridge};
use cr_bridge_core::sma::SIMDInfo;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ロガー初期化
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    print_banner();

    // SIMD 情報を表示
    let simd = SIMDInfo::detect();
    info!("SIMD バックエンド: {}", simd.active_backend);
    info!("  AVX-512: {}", simd.has_avx512f);
    info!("  AVX2:    {}", simd.has_avx2);
    info!("  NEON:    {}", simd.has_neon);

    // ATP エンジンを初期化
    let ekf_config = EKFConfig::default();
    let engine = Arc::new(ATPEngine::new(ekf_config));

    info!("ATPエンジンを起動しました");

    // 統計ログタスク
    let engine_stats = Arc::clone(&engine);
    let stats_task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(5));
        loop {
            ticker.tick().await;
            let stats = engine_stats.get_stats();
            info!(
                "📊 ATPStats: エンティティ={}, 受信={}, 欠落={}, ロス率={:.1}%, テレポート回避={}",
                stats.entity_count,
                stats.total_packets_received,
                stats.total_packets_lost,
                stats.avg_packet_loss_rate * 100.0,
                stats.ekf_prevented_teleport_count,
            );
        }
    });

    // ATPエンジンの tick タスク（毎16ms = 60fps）
    let engine_tick = Arc::clone(&engine);
    let tick_task = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(16));
        loop {
            ticker.tick().await;
            engine_tick.tick(16_000); // 16ms in us
        }
    });

    // VRChat OSC Bridge タスク
    let osc_config = OSCBridgeConfig::default();
    let osc_port = osc_config.listen_port;
    let bridge = VRChatOSCBridge::new(osc_config, Arc::clone(&engine));

    info!("VRChat OSC Bridge を起動します (ポート {})", osc_port);
    info!("VRChat の OSC 送信先を 127.0.0.1:{} に設定してください", osc_port);

    // OSC Bridge タスク
    let osc_task = tokio::spawn(async move {
        if let Err(e) = bridge.run().await {
            warn!("OSC Bridge エラー: {}", e);
        }
    });

    // CAS Bridge タスク（UDP :9101）
    let cas_engine = Arc::clone(&engine);
    let cas_task = tokio::spawn(async move {
        cas_listener(cas_engine).await;
    });

    // シグナルハンドリング
    tokio::signal::ctrl_c().await?;
    info!("シャットダウンシグナルを受信しました。終了します...");

    stats_task.abort();
    tick_task.abort();
    osc_task.abort();
    cas_task.abort();

    info!("CR-Bridge Daemon を終了しました");
    Ok(())
}

async fn cas_listener(engine: Arc<ATPEngine>) {
    match UdpSocket::bind("0.0.0.0:9101").await {
        Ok(sock) => {
            info!("CAS Bridge を起動します (UDP :9101)");
            let mut buf = vec![0u8; 65535];
            loop {
                let Ok((n, _)) = sock.recv_from(&mut buf).await else { continue };
                let Ok(text) = std::str::from_utf8(&buf[..n]) else { continue };
                match CRSBridge::parse_json_batch(text) {
                    Ok(packets) => {
                        for packet in packets {
                            engine.receive_packet(packet);
                        }
                    }
                    Err(e) => warn!("CAS parse error: {}", e),
                }
            }
        }
        Err(e) => warn!("CAS Bridge を起動できません: {}", e),
    }
}

fn print_banner() {
    println!();
    println!("  ╔═══════════════════════════════════════════════╗");
    println!("  ║           CR-Bridge Daemon v0.1.0             ║");
    println!("  ║   クロスリアリティ低レイヤー基盤ミドルウェア        ║");
    println!("  ║                                               ║");
    println!("  ║   Anti-Teleport Protocol Engine               ║");
    println!("  ║   EKF + DeadReckoning + Hermite Interpolation ║");
    println!("  ╚═══════════════════════════════════════════════╝");
    println!();
}
