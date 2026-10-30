use crate::ui::ActivePage;
use egui::Ui;
use rusttube_core::{PlaylistItem, UiCommand, VideoItem};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_video_grid(
    ui: &mut Ui,
    videos: &[VideoItem],
    playlists: &[PlaylistItem],
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    if videos.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label("No videos found in this view.");
        });
        return;
    }

    egui::Grid::new("rusttube_video_grid")
        .num_columns(3)
        .spacing([16.0, 16.0])
        .show(ui, |ui| {
            for (idx, video) in videos.iter().enumerate() {
                ui.group(|ui| {
                    ui.set_width(300.0);
                    ui.vertical(|ui| {
                        if ui.button(format!("▶ {}", video.title)).clicked() {
                            *current_page = ActivePage::Watch;
                            let _ = cmd_tx.send(UiCommand::PlayVideo {
                                video: video.clone(),
                                playlist_queue: None,
                                resume_from_seconds: video.resume_position_seconds,
                            });
                        }

                        ui.horizontal(|ui| {
                            if !video.channel_id.is_empty() {
                                if ui.small_button(&video.channel_name).clicked() {
                                    *current_page = ActivePage::Channel;
                                    let _ = cmd_tx.send(UiCommand::OpenChannel {
                                        channel_id: video.channel_id.clone(),
                                        page_token: None,
                                    });
                                }
                            } else {
                                ui.label(&video.channel_name);
                            }
                            ui.label("·");
                            ui.label(&video.duration_string);
                        });

                        ui.horizontal(|ui| {
                            if ui.small_button("Watch Later").clicked() {
                                let _ = cmd_tx.send(UiCommand::ToggleLocalWatchLater {
                                    video: video.clone(),
                                });
                            }
                            if !playlists.is_empty() {
                                ui.menu_button("+ Playlist", |ui| {
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
                    });
                });

                if (idx + 1) % 3 == 0 {
                    ui.end_row();
                }
            }
        });
}
