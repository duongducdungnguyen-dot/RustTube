use crate::theme::THEME;
use egui::{RichText, TextEdit, Ui};
use rusttube_auth::GoogleUserInfo;

pub fn render_header(
    ui: &mut Ui,
    search_query: &mut String,
    on_search: &mut bool,
    user_info: Option<&GoogleUserInfo>,
    is_authenticating: bool,
    on_login_click: &mut bool,
    is_syncing: bool,
    last_sync_text: &str,
    on_sync_click: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.add_space(8.0);

        // Search Input Box
        let text_edit = TextEdit::singleline(search_query)
            .hint_text("Search videos or paste YouTube URL...")
            .desired_width(480.0);
        let response = ui.add(text_edit);

        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            *on_search = true;
        }

        if ui
            .button(RichText::new("🔍 Search").size(12.0).strong())
            .clicked()
        {
            *on_search = true;
        }

        if !search_query.is_empty() {
            if ui.button("✕").clicked() {
                search_query.clear();
                *on_search = true;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(8.0);

            // Account Pill / Login Button
            if let Some(user) = user_info {
                let label_text = if user.name.is_empty() {
                    &user.email
                } else {
                    &user.name
                };
                ui.menu_button(
                    RichText::new(format!("👤 {}", label_text))
                        .size(12.0)
                        .strong()
                        .color(THEME.text_white),
                    |ui| {
                        ui.label(RichText::new(&user.email).size(11.0).color(THEME.text_muted));
                        ui.separator();
                        if ui.button("⚙ Account Settings").clicked() {
                            ui.close_menu();
                        }
                    },
                );
            } else if is_authenticating {
                ui.label(
                    RichText::new("⏳ Authenticating...")
                        .size(12.0)
                        .color(THEME.text_subdued),
                );
            } else if ui
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

            ui.add_space(12.0);

            // Sync Indicator
            if is_syncing {
                ui.label(
                    RichText::new("🔄 Syncing...")
                        .size(11.0)
                        .color(THEME.text_subdued),
                );
            } else {
                if !last_sync_text.is_empty() {
                    ui.label(
                        RichText::new(format!("Synced {}", last_sync_text))
                            .size(11.0)
                            .color(THEME.text_muted),
                    );
                }
                if ui
                    .button(RichText::new("🔄 Sync").size(11.0))
                    .on_hover_text("Synchronize YouTube account data with local cache")
                    .clicked()
                {
                    *on_sync_click = true;
                }
            }
        });
    });
}
