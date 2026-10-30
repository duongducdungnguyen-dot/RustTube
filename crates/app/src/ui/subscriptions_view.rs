use crate::ui::ActivePage;
use egui::Ui;
use rusttube_core::{ChannelItem, UiCommand};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_subscriptions_view(
    ui: &mut Ui,
    channels: &[ChannelItem],
    filter_query: &mut String,
    confirm_unsub_channel_id: &mut Option<String>,
    is_loading: bool,
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    ui.horizontal(|ui| {
        ui.heading(format!("Subscribed Channels ({})", channels.len()));
        ui.add(
            egui::TextEdit::singleline(filter_query)
                .hint_text("Filter channels by name or description...")
                .desired_width(240.0),
        );
        if ui
            .add_enabled(!is_loading, egui::Button::new("Refresh Subscriptions"))
            .clicked()
        {
            let _ = cmd_tx.send(UiCommand::LoadSubscriptions {
                force_refresh: true,
            });
        }
    });

    ui.separator();

    let query_lower = filter_query.trim().to_lowercase();
    let filtered: Vec<&ChannelItem> = channels
        .iter()
        .filter(|c| {
            query_lower.is_empty()
                || c.title.to_lowercase().contains(&query_lower)
                || c.description.to_lowercase().contains(&query_lower)
        })
        .collect();

    if filtered.is_empty() {
        ui.label("No subscribed channels match your filter.");
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for ch in filtered {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        if ui.button(format!("📺 {}", ch.title)).clicked() {
                            *current_page = ActivePage::Channel;
                            let _ = cmd_tx.send(UiCommand::OpenChannel {
                                channel_id: ch.channel_id.clone(),
                                page_token: None,
                            });
                        }
                        if !ch.description.is_empty() {
                            ui.label(&ch.description);
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(ref sub_id) = ch.subscription_id {
                            if confirm_unsub_channel_id.as_deref() == Some(&ch.channel_id) {
                                if ui.button("Confirm Unsubscribe").clicked() {
                                    let _ = cmd_tx.send(UiCommand::UnsubscribeChannel {
                                        channel_id: ch.channel_id.clone(),
                                        subscription_id: sub_id.clone(),
                                    });
                                    *confirm_unsub_channel_id = None;
                                }
                                if ui.button("Cancel").clicked() {
                                    *confirm_unsub_channel_id = None;
                                }
                            } else if ui.button("Unsubscribe").clicked() {
                                *confirm_unsub_channel_id = Some(ch.channel_id.clone());
                            }
                        }

                        if ui.button("Open Channel").clicked() {
                            *current_page = ActivePage::Channel;
                            let _ = cmd_tx.send(UiCommand::OpenChannel {
                                channel_id: ch.channel_id.clone(),
                                page_token: None,
                            });
                        }
                    });
                });
            });
        }
    });
}
