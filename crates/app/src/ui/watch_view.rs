use crate::ui::ActivePage;
use egui::Ui;
use rusttube_core::{PlaylistItem, UiCommand, VideoItem, VideoRating};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_watch_view(
    ui: &mut Ui,
    selected_video: Option<&VideoItem>,
    is_liked: bool,
    in_watch_later: bool,
    playlists: &[PlaylistItem],
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    let Some(video) = selected_video else {
        ui.label("Select a video to begin playback.");
        return;
    };

    ui.heading(&video.title);
    ui.horizontal(|ui| {
        if !video.channel_id.is_empty() {
            if ui.button(&video.channel_name).clicked() {
                *current_page = ActivePage::Channel;
                let _ = cmd_tx.send(UiCommand::OpenChannel {
                    channel_id: video.channel_id.clone(),
                    page_token: None,
                });
            }
        } else {
            ui.label(&video.channel_name);
        }

        ui.separator();

        let like_label = if is_liked { "👍 Liked" } else { "👍 Like" };
        if ui.button(like_label).clicked() {
            let _ = cmd_tx.send(UiCommand::RateVideo {
                video: video.clone(),
                rating: if is_liked {
                    VideoRating::None
                } else {
                    VideoRating::Like
                },
            });
        }

        let wl_label = if in_watch_later {
            "✓ In Watch Later"
        } else {
            "+ Watch Later"
        };
        if ui.button(wl_label).clicked() {
            let _ = cmd_tx.send(UiCommand::ToggleLocalWatchLater {
                video: video.clone(),
            });
        }

        if !playlists.is_empty() {
            ui.menu_button("+ Save to Playlist", |ui| {
                for pl in playlists {
                    if ui.button(&pl.title).clicked() {
                        let _ = cmd_tx.send(UiCommand::AddVideoToPlaylist {
                            playlist_id: pl.playlist_id.clone(),
                            video: video.clone(),
                        });
                        ui.close_menu();
                    }
                }
            });
        }
    });

    ui.separator();
    ui.label(&video.description);
}
