use crate::ui::ActivePage;
use egui::Ui;
use rusttube_core::{UiCommand, WatchHistoryEntry};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_history_view(
    ui: &mut Ui,
    entries: &[WatchHistoryEntry],
    search_query: &mut String,
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    ui.horizontal(|ui| {
        ui.heading(format!("Local Watch History ({})", entries.len()));
        if ui
            .add(
                egui::TextEdit::singleline(search_query)
                    .hint_text("Search local history...")
                    .desired_width(220.0),
            )
            .changed()
        {
            let q = search_query.trim().to_string();
            let _ = cmd_tx.send(UiCommand::LoadHistory {
                search_query: if q.is_empty() { None } else { Some(q) },
            });
        }

        if !entries.is_empty() && ui.button("Clear All History").clicked() {
            let _ = cmd_tx.send(UiCommand::ClearHistory);
        }
    });
    ui.label("Local playback history is stored privately in SQLite on your device (YouTube's official Data API v3 does not expose personal watch history).");
    ui.separator();

    for entry in entries {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                if ui.button(format!("▶ {}", entry.video.title)).clicked() {
                    *current_page = ActivePage::Watch;
                    let _ = cmd_tx.send(UiCommand::PlayVideo {
                        video: entry.video.clone(),
                        playlist_queue: None,
                        resume_from_seconds: if entry.last_position_seconds > 0 {
                            Some(entry.last_position_seconds)
                        } else {
                            None
                        },
                    });
                }

                ui.label(&entry.video.channel_name);
                if entry.last_position_seconds > 0 {
                    ui.label(format!(
                        "(Resume at {}:{:02})",
                        entry.last_position_seconds / 60,
                        entry.last_position_seconds % 60
                    ));
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Remove Entry").clicked() {
                        let _ = cmd_tx.send(UiCommand::RemoveHistoryEntry {
                            video_id: entry.video.id.clone(),
                        });
                    }
                });
            });
        });
    }
}
