use crate::ui::{video_grid::render_video_grid, ActivePage};
use egui::Ui;
use rusttube_core::{ChannelItem, PlaylistItem, UiCommand, VideoItem};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_channel_view(
    ui: &mut Ui,
    channel: Option<&ChannelItem>,
    videos: &[VideoItem],
    next_page_token: &Option<String>,
    playlists: &[PlaylistItem],
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    let Some(ch) = channel else {
        ui.label("Loading channel details...");
        return;
    };

    ui.group(|ui| {
        ui.heading(&ch.title);
        ui.horizontal(|ui| {
            if let Some(ref subs) = ch.subscriber_count {
                ui.label(format!("{} subscribers", subs));
                ui.label("·");
            }
            if let Some(ref uploads) = ch.uploads_playlist_id {
                ui.label(format!("Uploads Playlist: {}", uploads));
                ui.label("·");
            }
            let url = format!("https://www.youtube.com/channel/{}", ch.channel_id);
            ui.hyperlink_to("Channel URL", url);
        });
        if !ch.description.is_empty() {
            ui.add_space(4.0);
            ui.label(&ch.description);
        }
    });

    ui.add_space(10.0);
    ui.heading("Latest Uploaded Videos");
    ui.separator();

    render_video_grid(ui, videos, playlists, current_page, cmd_tx);

    if let Some(ref token) = next_page_token {
        ui.add_space(8.0);
        if ui.button("Load More Channel Uploads").clicked() {
            let _ = cmd_tx.send(UiCommand::OpenChannel {
                channel_id: ch.channel_id.clone(),
                page_token: Some(token.clone()),
            });
        }
    }
}
