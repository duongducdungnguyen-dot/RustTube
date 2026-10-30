use crate::cache::ThumbnailManager;
use crate::theme::THEME;
use egui::{RichText, Ui};
use rusttube_core::models::VideoItem;

pub fn render_watch_later_view(
    ui: &mut Ui,
    items: &[VideoItem],
    thumbnails: &ThumbnailManager,
    watch_later_ids: &[String],
    on_select_video: &mut Option<VideoItem>,
    on_toggle_watch_later: &mut Option<VideoItem>,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Watch Later")
                    .size(18.0)
                    .bold()
                    .color(THEME.text_white),
            );

            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("({} videos)", items.len()))
                    .size(12.0)
                    .color(THEME.text_muted),
            );
        });

        ui.add_space(16.0);

        if items.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("No videos saved in Watch Later.")
                        .size(13.0)
                        .color(THEME.text_subdued),
                );
                ui.label(
                    RichText::new("Click the bookmark icon on any video card to save it for later.")
                        .size(11.0)
                        .color(THEME.text_muted),
                );
            });
            return;
        }

        crate::ui::video_grid::render_video_grid(
            ui,
            items,
            thumbnails,
            watch_later_ids,
            on_select_video,
            on_toggle_watch_later,
        );
    });
}
