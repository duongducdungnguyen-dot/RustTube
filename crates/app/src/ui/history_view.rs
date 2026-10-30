use crate::cache::ThumbnailManager;
use crate::theme::THEME;
use egui::{RichText, Ui};
use rusttube_core::models::VideoItem;

pub fn render_history_view(
    ui: &mut Ui,
    history_items: &[VideoItem],
    thumbnails: &ThumbnailManager,
    watch_later_ids: &[String],
    on_clear_history: &mut bool,
    on_select_video: &mut Option<VideoItem>,
    on_toggle_watch_later: &mut Option<VideoItem>,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Watch History")
                    .size(18.0)
                    .strong()
                    .color(THEME.text_white),
            );

            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("({} videos)", history_items.len()))
                    .size(12.0)
                    .color(THEME.text_muted),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !history_items.is_empty() {
                    if ui
                        .button(RichText::new("🗑 Clear History").size(11.0).color(THEME.crimson))
                        .clicked()
                    {
                        *on_clear_history = true;
                    }
                }
            });
        });

        ui.add_space(16.0);

        if history_items.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("No watch history recorded yet.")
                        .size(13.0)
                        .color(THEME.text_subdued),
                );
                ui.label(
                    RichText::new("Videos played inside RustTube will automatically appear here.")
                        .size(11.0)
                        .color(THEME.text_muted),
                );
            });
            return;
        }

        crate::ui::video_grid::render_video_grid(
            ui,
            history_items,
            thumbnails,
            watch_later_ids,
            on_select_video,
            on_toggle_watch_later,
        );
    });
}
