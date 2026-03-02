//! CR-Bridge Metaverse Server
//!
//! ## 提供するエンドポイント
//! - `GET  /ws`                       WebSocket (メタバース)
//! - `GET  /api/info`                 サーバー情報 (JSON)
//! - `GET  /api/worlds/:id/entities`  ワールド内エンティティ一覧 (JSON)
//! - `POST /api/worlds`               ワールド作成 (JSON)
//! - `GET  /`                         Three.js メタバースクライアント (静的ファイル)
//!
//! ## 環境変数
//! - `PORT`        待受ポート (デフォルト: 8080)
//! - `STATIC_DIR`  静的ファイルディレクトリ (デフォルト: ./static)
//! - `JWT_SECRET`  JWT署名キー (デフォルト: デバッグ用キー)

use axum::{
    Router,
    routing::{get, post},
};
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

mod auth;
mod handler;
mod protocol;
mod session;
mod state;
mod storage;
mod world;

use handler::{api_create_world, api_info, api_world_entities, ws_handler};
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    print_banner();

    let state = AppState::new();

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let static_dir = std::env::var("STATIC_DIR")
        .unwrap_or_else(|_| "metaverse-server/static".to_string());

    let app = Router::new()
        // WebSocket
        .route("/ws", get(ws_handler))
        // REST API
        .route("/api/info", get(api_info))
        .route("/api/worlds/:id/entities", get(api_world_entities))
        .route("/api/worlds", post(api_create_world))
        // 静的ファイル (Three.js クライアント)
        .fallback_service(ServeDir::new(&static_dir))
        // ミドルウェア
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("🌐 メタバースサーバー起動: http://localhost:{}", port);
    info!("🔌 WebSocket: ws://localhost:{}/ws", port);
    info!("📁 静的ファイル: {}", static_dir);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn print_banner() {
    println!();
    println!("  ╔══════════════════════════════════════════════════════╗");
    println!("  ║    CR-Bridge Metaverse Server  v0.1.0               ║");
    println!("  ║                                                      ║");
    println!("  ║    World Service + Presence + State Replication      ║");
    println!("  ║    Interest Management + ATP Integration             ║");
    println!("  ╚══════════════════════════════════════════════════════╝");
    println!();
}
