use crate::theme::THEME;
use egui::{Align, Align2, Color32, Layout, RichText, Sense, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Watch,
    Subscriptions,
    Playlists,
    LikedVideos,
    WatchLater,
    History,
    Settings,
}

pub fn render_sidebar(
    ui: &mut Ui,
    current_page: &mut Page,
    watch_later_count: usize,
    history_count: usize,
) {
    ui.vertical(|ui| {
        ui.add_space(10.0);

        // App Brand Logo
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(26.0, 18.0), Sense::hover());
            ui.painter().rect_filled(rect, 4.0, THEME.crimson);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "▶",
                egui::FontId::proportional(10.0),
                Color32::WHITE,
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new("RustTube")
                    .size(17.0)
                    .strong()
                    .color(THEME.text_white),
            );
        });

        ui.add_space(20.0);

        // Sidebar Navigation Items
        let nav_item = |ui: &mut Ui, label: &str, page: Page, current: Page, count: Option<usize>| -> bool {
            let selected = current == page;
            let fill = if selected {
                Color32::from_rgb(45, 12, 22)
            } else {
                Color32::TRANSPARENT
            };
            let text_color = if selected {
                THEME.crimson
            } else {
                THEME.text_subdued
            };

            let response = ui.allocate_response(Vec2::new(ui.available_width(), 34.0), Sense::click());
            if ui.is_rect_visible(response.rect) {
                if response.hovered() && !selected {
                    ui.painter().rect_filled(
                        response.rect,
                        6.0,
                        Color32::from_rgb(22, 24, 32),
                    );
                } else if selected {
                    ui.painter().rect_filled(response.rect, 6.0, fill);
                    ui.painter().rect_stroke(
                        response.rect,
                        6.0,
                        egui::Stroke::new(1.0, Color32::from_rgb(120, 20, 40)),
                    );
                }

                let text_pos = response.rect.min + Vec2::new(14.0, 8.0);
                ui.painter().text(
                    text_pos,
                    Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(13.0),
                    text_color,
                );

                if let Some(c) = count {
                    if c > 0 {
                        let count_str = c.to_string();
                        let badge_pos = response.rect.max - Vec2::new(14.0, 18.0);
                        ui.painter().text(
                            badge_pos,
                            Align2::RIGHT_CENTER,
                            &count_str,
                            egui::FontId::monospace(11.0),
                            THEME.text_muted,
                        );
                    }
                }
            }

            response.clicked()
        };

        if nav_item(ui, "🏠  Home", Page::Home, *current_page, None) {
            *current_page = Page::Home;
        }
        ui.add_space(2.0);

        if nav_item(ui, "📺  Subscriptions", Page::Subscriptions, *current_page, None) {
            *current_page = Page::Subscriptions;
        }
        ui.add_space(2.0);

        if nav_item(ui, "📁  Playlists", Page::Playlists, *current_page, None) {
            *current_page = Page::Playlists;
        }
        ui.add_space(2.0);

        if nav_item(ui, "👍  Liked Videos", Page::LikedVideos, *current_page, None) {
            *current_page = Page::LikedVideos;
        }
        ui.add_space(2.0);

        if nav_item(
            ui,
            "⏰  Watch Later",
            Page::WatchLater,
            *current_page,
            Some(watch_later_count),
        ) {
            *current_page = Page::WatchLater;
        }
        ui.add_space(2.0);

        if nav_item(
            ui,
            "📜  History",
            Page::History,
            *current_page,
            Some(history_count),
        ) {
            *current_page = Page::History;
        }

        ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
            ui.add_space(10.0);
            if nav_item(ui, "⚙  Settings", Page::Settings, *current_page, None) {
                *current_page = Page::Settings;
            }
        });
    });
}
