use crate::cache::ThumbnailManager;
use crate::theme::THEME;
use egui::{RichText, Ui, Vec2};
use rusttube_core::models::ChannelItem;

pub fn render_subscriptions_view(
    ui: &mut Ui,
    channels: &[ChannelItem],
    _thumbnails: &ThumbnailManager,
    is_logged_in: bool,
    on_login_click: &mut bool,
    on_select_channel: &mut Option<ChannelItem>,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new("Subscribed Channels")
                .size(18.0)
                .strong()
                .color(THEME.text_white),
        );
        ui.label(
            RichText::new("Channels you follow on YouTube. Click any channel to open its dedicated page.")
                .size(12.0)
                .color(THEME.text_muted),
        );

        ui.add_space(16.0);

        if !is_logged_in {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("Sign in with Google to sync your YouTube channel subscriptions.")
                        .size(13.0)
                        .color(THEME.text_subdued),
                );
                ui.add_space(12.0);
                if ui
                    .button(
                        RichText::new("Sign In with Google")
                            .size(12.0)
                            .strong()
                            .color(THEME.text_white),
                    )
                    .clicked()
                {
                    *on_login_click = true;
                }
            });
            return;
        }

        if channels.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("No subscriptions found in local database.")
                        .size(13.0)
                        .color(THEME.text_subdued),
                );
                ui.label(
                    RichText::new("Click 'Sync' in the top header bar to fetch your latest YouTube subscriptions.")
                        .size(11.0)
                        .color(THEME.text_muted),
                );
            });
            return;
        }

        egui::Grid::new("subscriptions_grid")
            .spacing(Vec2::new(16.0, 16.0))
            .show(ui, |ui| {
                for (idx, ch) in channels.iter().enumerate() {
                    if idx > 0 && idx % 3 == 0 {
                        ui.end_row();
                    }

                    let response = ui.group(|ui| {
                        ui.set_width(260.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&ch.title)
                                        .size(13.0)
                                        .strong()
                                        .color(THEME.text_white),
                                );
                                ui.label(
                                    RichText::new("View Channel →")
                                        .size(11.0)
                                        .color(THEME.crimson),
                                );
                            });
                        });
                    });

                    if response.response.clicked() {
                        *on_select_channel = Some(ch.clone());
                    }
                }
            });
    });
}
