use crate::cache::ThumbnailManager;
use crate::theme::THEME;
use egui::{Align, Color32, CornerRadius, Image, Rect, RichText, Sense, Stroke, Ui, Vec2};
use rusttube_core::models::VideoItem;

pub fn render_video_grid(
    ui: &mut Ui,
    videos: &[VideoItem],
    thumbnails: &ThumbnailManager,
    watch_later_ids: &[String],
    on_select_video: &mut Option<VideoItem>,
    on_toggle_watch_later: &mut Option<VideoItem>,
) {
    if videos.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                RichText::new("No videos found.")
                    .size(15.0)
                    .color(THEME.text_subdued),
            );
            ui.label(
                RichText::new("Use the search bar above or check your network connection.")
                    .size(12.0)
                    .color(THEME.text_muted),
            );
        });
        return;
    }

    let card_width = 280.0;
    let card_height = 230.0;
    let available_width = ui.available_width();
    let num_columns = ((available_width / (card_width + 12.0)).floor() as usize).max(1);

    egui::Grid::new("video_card_grid")
        .spacing(Vec2::new(12.0, 16.0))
        .max_col_width(card_width)
        .show(ui, |ui| {
            for (idx, video) in videos.iter().enumerate() {
                if idx > 0 && idx % num_columns == 0 {
                    ui.end_row();
                }

                let in_watch_later = watch_later_ids.contains(&video.id);

                ui.allocate_ui(Vec2::new(card_width, card_height), |ui| {
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(card_width, card_height), Sense::click());

                    let is_hovered = response.hovered();

                    // Card Background
                    let bg_color = if is_hovered {
                        THEME.card_hover
                    } else {
                        THEME.card_bg
                    };
                    let stroke = if is_hovered {
                        Stroke::new(1.0, THEME.border_active)
                    } else {
                        Stroke::new(1.0, THEME.border)
                    };

                    ui.painter().rect_filled(rect, CornerRadius::same(8), bg_color);
                    ui.painter().rect_stroke(rect, CornerRadius::same(8), stroke);

                    // Top Thumbnail Area
                    let thumb_height = 158.0;
                    let thumb_rect = Rect::from_min_size(rect.min, Vec2::new(card_width, thumb_height));

                    if let Some(tex) = thumbnails.get_or_load(&video.thumbnail_url) {
                        Image::new(&tex)
                            .corner_radius(CornerRadius {
                                nw: 8,
                                ne: 8,
                                sw: 0,
                                se: 0,
                            })
                            .paint_at(ui, thumb_rect);
                    } else {
                        ui.painter().rect_filled(
                            thumb_rect,
                            CornerRadius {
                                nw: 8,
                                ne: 8,
                                sw: 0,
                                se: 0,
                            },
                            Color32::from_rgb(18, 19, 26),
                        );
                        ui.painter().text(
                            thumb_rect.center(),
                            Align::Center,
                            "▶",
                            egui::FontId::proportional(20.0),
                            THEME.text_muted,
                        );
                    }

                    // Duration Badge (bottom-right of thumbnail)
                    if !video.duration_string.is_empty() {
                        let dur_text = &video.duration_string;
                        let dur_pos = thumb_rect.max - Vec2::new(8.0, 8.0);
                        let font_id = egui::FontId::monospace(10.0);
                        let galley = ui.painter().layout_no_wrap(
                            dur_text.clone(),
                            font_id.clone(),
                            Color32::WHITE,
                        );
                        let badge_rect = Rect::from_min_size(
                            dur_pos - Vec2::new(galley.size().x + 8.0, galley.size().y + 4.0),
                            galley.size() + Vec2::new(8.0, 4.0),
                        );
                        ui.painter().rect_filled(
                            badge_rect,
                            4.0,
                            Color32::from_black_alpha(210),
                        );
                        ui.painter().galley(
                            badge_rect.min + Vec2::new(4.0, 2.0),
                            galley,
                            Color32::WHITE,
                        );
                    }

                    // Watch Later Quick Action Button (top-right of thumbnail when hovered)
                    if is_hovered {
                        let wl_rect = Rect::from_min_size(
                            thumb_rect.max - Vec2::new(32.0, thumb_height - 8.0),
                            Vec2::new(24.0, 24.0),
                        );
                        ui.painter().rect_filled(
                            wl_rect,
                            4.0,
                            Color32::from_black_alpha(200),
                        );
                        let wl_icon = if in_watch_later { "🔖" } else { "📌" };
                        ui.painter().text(
                            wl_rect.center(),
                            Align::Center,
                            wl_icon,
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                    }

                    // Card Bottom Metadata
                    let text_rect = Rect::from_min_size(
                        rect.min + Vec2::new(10.0, thumb_height + 8.0),
                        Vec2::new(card_width - 20.0, card_height - thumb_height - 12.0),
                    );

                    // Title
                    let title_text = if video.title.len() > 55 {
                        format!("{}...", &video.title[..52])
                    } else {
                        video.title.clone()
                    };
                    ui.painter().text(
                        text_rect.min,
                        Align::LEFT,
                        &title_text,
                        egui::FontId::proportional(12.0),
                        if is_hovered { THEME.crimson } else { THEME.text_white },
                    );

                    // Channel name & Views
                    let meta_text = format!(
                        "{} • {}",
                        if video.channel_name.is_empty() {
                            "YouTube"
                        } else {
                            &video.channel_name
                        },
                        if video.view_count > 0 {
                            format_views(video.view_count)
                        } else {
                            String::new()
                        }
                    );
                    ui.painter().text(
                        text_rect.min + Vec2::new(0.0, 36.0),
                        Align::LEFT,
                        &meta_text,
                        egui::FontId::proportional(11.0),
                        THEME.text_subdued,
                    );

                    if response.clicked() {
                        *on_select_video = Some(video.clone());
                    }
                });
            }
        });
}

fn format_views(views: u64) -> String {
    if views >= 1_000_000 {
        format!("{:.1}M views", views as f64 / 1_000_000.0)
    } else if views >= 1_000 {
        format!("{:.1}K views", views as f64 / 1_000.0)
    } else if views > 0 {
        format!("{} views", views)
    } else {
        String::new()
    }
}
