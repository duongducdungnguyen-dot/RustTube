use crate::ui::{video_grid::render_video_grid, ActivePage};
use egui::Ui;
use rusttube_core::{PlaylistItem, UiCommand, VideoItem};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_liked_view(
    ui: &mut Ui,
    liked_videos: &[VideoItem],
    next_page_token: &Option<String>,
    playlists: &[PlaylistItem],
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    ui.horizontal(|ui| {
        ui.heading(format!("Liked Videos ({})", liked_videos.len()));
        if ui.button("Refresh Liked Videos").clicked() {
            let _ = cmd_tx.send(UiCommand::LoadLikedVideos {
                page_token: None,
                force_refresh: true,
            });
        }
    });
    ui.separator();

    render_video_grid(ui, liked_videos, playlists, current_page, cmd_tx);

    if let Some(ref token) = next_page_token {
        if ui.button("Load More Liked Videos").clicked() {
            let _ = cmd_tx.send(UiCommand::LoadLikedVideos {
                page_token: Some(token.clone()),
                force_refresh: false,
            });
        }
    }
}
