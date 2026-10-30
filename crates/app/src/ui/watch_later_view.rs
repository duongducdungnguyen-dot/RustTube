use crate::ui::ActivePage;
use egui::Ui;
use rusttube_core::{UiCommand, WatchLaterEntry};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_watch_later_view(
    ui: &mut Ui,
    local_entries: &[WatchLaterEntry],
    experimental_entries: &[WatchLaterEntry],
    experimental_enabled: bool,
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    ui.heading(format!("A. RustTube Local Watch Later ({})", local_entries.len()));
    ui.label("Stored persistently in your local SQLite database. Never overwritten by remote changes.");
    ui.separator();

    if local_entries.is_empty() {
        ui.label("No videos saved in your local RustTube Watch Later queue.");
    } else {
        for entry in local_entries {
            ui.horizontal(|ui| {
                if ui.button(format!("▶ {}", entry.video.title)).clicked() {
                    *current_page = ActivePage::Watch;
                    let _ = cmd_tx.send(UiCommand::PlayVideo {
                        video: entry.video.clone(),
                        playlist_queue: None,
                        resume_from_seconds: None,
                    });
                }
                ui.label(&entry.video.channel_name);
                if ui.small_button("Remove").clicked() {
                    let _ = cmd_tx.send(UiCommand::RemoveWatchLater {
                        video_id: entry.video.id.clone(),
                    });
                }
            });
        }
    }

    ui.add_space(20.0);
    ui.heading("B. YouTube Watch Later (Unofficial / Cookie-Based)");
    ui.label("Note: YouTube's official Data API v3 restricts access to the Watch Later (WL) playlist. Optional extraction uses yt-dlp with your local browser cookies and never overwrites your RustTube local queue.");
    if experimental_enabled && ui.button("Fetch via Browser Cookies (Experimental)").clicked() {
        let _ = cmd_tx.send(UiCommand::ImportExperimentalYouTubeWatchLater);
    }

    for entry in experimental_entries {
        ui.horizontal(|ui| {
            if ui.button(format!("▶ {}", entry.video.title)).clicked() {
                *current_page = ActivePage::Watch;
                let _ = cmd_tx.send(UiCommand::PlayVideo {
                    video: entry.video.clone(),
                    playlist_queue: None,
                    resume_from_seconds: None,
                });
            }
            ui.label(&entry.video.channel_name);
        });
    }
}
