#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cache;
mod theme;
mod ui;

use anyhow::Result;
use app::RustTubeApp;
use rusttube_core::AppConfig;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info,rusttube=debug")
        .init();

    let config = AppConfig::load().unwrap_or_default();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()?;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RustTube")
            .with_inner_size([1360.0, 840.0])
            .with_min_inner_size([960.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RustTube",
        native_options,
        Box::new(move |cc| {
            theme::apply_obsidian_theme(&cc.egui_ctx);
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(RustTubeApp::new(config, rt)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}
