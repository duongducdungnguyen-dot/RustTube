mod app;
mod cache;
mod theme;
mod ui;

use anyhow::Result;
use app::RustTubeApp;
use eframe::NativeOptions;
use rusttube_api::YouTubeApiClient;
use rusttube_auth::GoogleAuthManager;
use rusttube_core::AppConfig;
use rusttube_player::PlayerController;
use rusttube_storage::Database;
use rusttube_ytdlp::YtDlpWrapper;
use std::sync::Arc;
use tracing::info;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting RustTube native Windows desktop application...");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let _guard = rt.enter();

    let config = AppConfig::load_or_default("config.toml");
    let db = Database::new("cache/rusttube.db")?;

    let auth_manager = Arc::new(GoogleAuthManager::new(
        &config.auth.google_client_id,
        config.auth.redirect_port,
        &config.auth.keyring_service_name,
    ));

    let api_client = Arc::new(YouTubeApiClient::new());
    let ytdlp = Arc::new(YtDlpWrapper::new(
        &config.ytdlp.binary_path,
        &config.ytdlp.cookies_browser,
    ));

    let player = Arc::new(tokio::sync::Mutex::new(PlayerController::new(
        &config.player.hwdec,
    )));

    {
        let mut p = rt.block_on(player.lock());
        if let Err(e) = p.initialize() {
            tracing::warn!("libmpv player initialization notice: {}", e);
        }
    }

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RustTube")
            .with_inner_size([1380.0, 860.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RustTube",
        options,
        Box::new(move |cc| {
            Ok(Box::new(RustTubeApp::new(
                cc,
                db,
                auth_manager,
                api_client,
                ytdlp,
                player,
            )))
        }),
    )?;

    Ok(())
}
