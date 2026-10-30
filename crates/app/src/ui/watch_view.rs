use crate::cache::ThumbnailManager;
use crate::theme::THEME;
use egui::{Color32, CornerRadius, RichText, Slider, Stroke, Ui, Vec2};
use rusttube_core::models::{PlaybackStatus, VideoItem};

pub fn render_watch_view(
    ui: &mut Ui,
    selected_video: &VideoItem,
    video_description: &str,
    status: &PlaybackStatus,
    related_videos: &[VideoItem],
    thumbnails: &ThumbnailManager,
    in_watch_later: bool,
    is_liked: bool,
    on_back: &mut bool,
    on_toggle_play: &mut bool,
    on_seek: &mut Option<f64>,
    on_volume_change: &mut Option<u32>,
    on_speed_change: &mut Option<f32>,
    on_quality_change: &mut Option<String>,
    on_toggle_watch_later: &mut bool,
    on_toggle_like: &mut bool,
    on_select_related: &mut Option<VideoItem>,
) {
    ui.vertical(|ui| {
        // Top Navigation Bar
        ui.horizontal(|ui| {
            if ui
                .button(RichText::new("← Back to Browse").size(12.0).bold())
                .clicked()
            {
                *on_back = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("Quality: {}", status.quality))
                        .size(11.0)
                        .color(THEME.text_muted),
                );
            });
        });

        ui.add_space(10.0);

        // Native Player Canvas / Placeholder Surface
        let player_height = 420.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), player_height), egui::Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::same(8), Color32::BLACK);
        ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0, THEME.border));

        if !status.is_playing {
            ui.painter().text(
                rect.center(),
                egui::Align::Center,
                "▶ Playing in hardware accelerated libmpv engine",
                egui::FontId::proportional(15.0),
                THEME.text_subdued,
            );
        }

        ui.add_space(8.0);

        // Player Controls Bar
        ui.group(|ui| {
            ui.horizontal(|ui| {
                // Play / Pause Button
                let play_btn_text = if status.is_playing { "⏸ Pause" } else { "▶ Play" };
                if ui.button(RichText::new(play_btn_text).size(12.0).bold()).clicked() {
                    *on_toggle_play = true;
                }

                ui.add_space(10.0);

                // Scrubber Slider
                let mut pos = status.position_seconds;
                let dur = status.duration_seconds.max(1.0);
                ui.style_mut().spacing.slider_width = 300.0;
                let slider = Slider::new(&mut pos, 0.0..=dur).show_value(false);
                if ui.add(slider).changed() {
                    *on_seek = Some(pos);
                }

                // Time Display Text
                let cur_m = (status.position_seconds as u32) / 60;
                let cur_s = (status.position_seconds as u32) % 60;
                let dur_m = (status.duration_seconds as u32) / 60;
                let dur_s = (status.duration_seconds as u32) % 60;
                ui.label(
                    RichText::new(format!("{:02}:{:02} / {:02}:{:02}", cur_m, cur_s, dur_m, dur_s))
                        .size(11.0)
                        .monospace()
                        .color(THEME.text_subdued),
                );

                ui.add_space(12.0);

                // Volume Control
                ui.label(RichText::new("🔊").size(12.0));
                let mut vol = status.volume;
                ui.style_mut().spacing.slider_width = 80.0;
                if ui.add(Slider::new(&mut vol, 0..=100).show_value(false)).changed() {
                    *on_volume_change = Some(vol);
                }

                ui.add_space(12.0);

                // Playback Speed Selector
                ui.menu_button(
                    RichText::new(format!("Speed: {:.1}x", status.speed)).size(11.0),
                    |ui| {
                        if ui.button("0.5x").clicked() {
                            *on_speed_change = Some(0.5);
                            ui.close_menu();
                        }
                        if ui.button("1.0x (Normal)").clicked() {
                            *on_speed_change = Some(1.0);
                            ui.close_menu();
                        }
                        if ui.button("1.25x").clicked() {
                            *on_speed_change = Some(1.25);
                            ui.close_menu();
                        }
                        if ui.button("1.5x").clicked() {
                            *on_speed_change = Some(1.5);
                            ui.close_menu();
                        }
                        if ui.button("2.0x").clicked() {
                            *on_speed_change = Some(2.0);
                            ui.close_menu();
                        }
                    },
                );

                // Quality Selector
                ui.menu_button(
                    RichText::new(format!("Quality: {}", status.quality)).size(11.0),
                    |ui| {
                        if ui.button("1080p (HD)").clicked() {
                            *on_quality_change = Some("1080p".to_string());
                            ui.close_menu();
                        }
                        if ui.button("720p").clicked() {
                            *on_quality_change = Some("720p".to_string());
                            ui.close_menu();
                        }
                        if ui.button("480p").clicked() {
                            *on_quality_change = Some("480p".to_string());
                            ui.close_menu();
                        }
                        if ui.button("360p").clicked() {
                            *on_quality_change = Some("360p".to_string());
                            ui.close_menu();
                        }
                    },
                );
            });
        });

        ui.add_space(12.0);

        // Video Title & Meta Header
        ui.label(
            RichText::new(&selected_video.title)
                .size(18.0)
                .bold()
                .color(THEME.text_white),
        );

        ui.add_space(6.0);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("📺 {}", &selected_video.channel_name))
                    .size(13.0)
                    .bold()
                    .color(THEME.text_white),
            );

            ui.add_space(20.0);

            // Action Buttons (Like / Watch Later)
            let like_text = if is_liked { "👍 Liked" } else { "👍 Like" };
            if ui.button(RichText::new(like_text).size(12.0)).clicked() {
                *on_toggle_like = true;
            }

            let wl_text = if in_watch_later { "🔖 Saved" } else { "📌 Watch Later" };
            if ui.button(RichText::new(wl_text).size(12.0)).clicked() {
                *on_toggle_watch_later = true;
            }
        });

        ui.add_space(12.0);

        // Video Description Box
        if !video_description.is_empty() {
            ui.group(|ui| {
                ui.label(
                    RichText::new("Description")
                        .size(12.0)
                        .bold()
                        .color(THEME.text_subdued),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(video_description)
                        .size(11.5)
                        .color(THEME.text_subdued),
                );
            });
        }
    });
}
