use crate::cache::ThumbnailManager;
use crate::theme::THEME;
use egui::{RichText, Ui};
use rusttube_core::models::{ChannelDetail, VideoItem};

pub fn render_channel_view(
    ui: &mut Ui,
    channel_detail: &Option<ChannelDetail>,
    channel_videos: &[VideoItem],
    thumbnails: &ThumbnailManager,
    watch_later_ids: &[String],
    is_subscribed: bool,
    is_logged_in: bool,
    on_back: &mut bool,
    on_toggle_subscribe: &mut bool,
    on_select_video: &mut Option<VideoItem>,
    on_toggle_watch_later: &mut Option<VideoItem>,
) {
    ui.vertical(|ui| {
        // Top Back Button
        ui.horizontal(|ui| {
            if ui
                .button(RichText::new("← Back").size(12.0).strong())
                .clicked()
            {
                *on_back = true;
            }
        });

        ui.add_space(10.0);

        if let Some(detail) = channel_detail {
            // Channel Header Box
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&detail.item.title)
                                .size(20.0)
                                .strong()
                                .color(THEME.text_white),
                        );
                        ui.label(
                            RichText::new(&detail.subscriber_text)
                                .size(12.0)
                                .color(THEME.text_subdued),
                        );
                        if !detail.item.description.is_empty() {
                            ui.add_space(4.0);
                            let desc_snippet = if detail.item.description.len() > 180 {
                                format!("{}...", &detail.item.description[..175])
                            } else {
                                detail.item.description.clone()
                            };
                            ui.label(
                                RichText::new(&desc_snippet)
                                    .size(11.5)
                                    .color(THEME.text_muted),
                            );
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if is_logged_in {
                            let sub_text = if is_subscribed { "Subscribed ✓" } else { "Subscribe" };
                            let sub_color = if is_subscribed { THEME.text_subdued } else { THEME.text_white };
                            if ui
                                .button(RichText::new(sub_text).size(12.0).strong().color(sub_color))
                                .clicked()
                            {
                                *on_toggle_subscribe = true;
                            }
                        }
                    });
                });
            });

            ui.add_space(16.0);

            ui.label(
                RichText::new("Uploads & Videos")
                    .size(16.0)
                    .strong()
                    .color(THEME.text_white),
            );

            ui.add_space(10.0);

            if channel_videos.is_empty() {
                ui.label(
                    RichText::new("Loading channel videos...")
                        .size(12.0)
                        .color(THEME.text_subdued),
                );
            } else {
                crate::ui::video_grid::render_video_grid(
                    ui,
                    channel_videos,
                    thumbnails,
                    watch_later_ids,
                    on_select_video,
                    on_toggle_watch_later,
                );
            }
        } else {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("Loading channel information...")
                        .size(13.0)
                        .color(THEME.text_subdued),
                );
            });
        }
    });
}
