use crate::ui::ActivePage;
use egui::Ui;
use rusttube_core::{SyncState, UiCommand, UpdateState, UserProfile};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_header(
    ui: &mut Ui,
    search_query: &mut String,
    profile: &Option<UserProfile>,
    sync_state: &SyncState,
    update_state: &UpdateState,
    can_go_back: bool,
    on_back: &mut bool,
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(can_go_back, egui::Button::new("← Back"))
            .clicked()
        {
            *on_back = true;
        }

        if ui.button("RustTube").clicked() {
            *current_page = ActivePage::Home;
            let _ = cmd_tx.send(UiCommand::LoadHomeFeed {
                category: rusttube_core::FeedCategory::Personalized,
                force_refresh: false,
            });
        }

        ui.separator();

        let response = ui.add(
            egui::TextEdit::singleline(search_query)
                .hint_text("Search videos, channels, or paste YouTube link...")
                .desired_width(360.0),
        );

        if (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || ui.button("Search").clicked()
        {
            let q = search_query.trim().to_string();
            if !q.is_empty() {
                *current_page = ActivePage::Search;
                let _ = cmd_tx.send(UiCommand::Search { query: q });
            }
        }

        if let UpdateState::UpdateAvailable { manifest, .. } = update_state {
            if ui
                .button(format!("Update v{} Available", manifest.version))
                .clicked()
            {
                *current_page = ActivePage::Settings;
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(user) = profile {
                if ui.button(&user.display_name).clicked() {
                    *current_page = ActivePage::Settings;
                }
                if ui
                    .add_enabled(!sync_state.is_syncing, egui::Button::new("Sync Account"))
                    .clicked()
                {
                    let _ = cmd_tx.send(UiCommand::TriggerAccountSync);
                }
            } else if ui.button("Sign In with Google").clicked() {
                let _ = cmd_tx.send(UiCommand::StartGoogleLogin);
            }
        });
    });
}
