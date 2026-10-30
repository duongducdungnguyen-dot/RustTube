use crate::ui::ActivePage;
use egui::Ui;
use rusttube_core::{FeedCategory, UiCommand};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_sidebar(
    ui: &mut Ui,
    current_page: &mut ActivePage,
    watch_later_count: usize,
    history_count: usize,
    liked_count: usize,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 6.0;

        if ui
            .selectable_label(*current_page == ActivePage::Home, "Home")
            .clicked()
        {
            *current_page = ActivePage::Home;
            let _ = cmd_tx.send(UiCommand::LoadHomeFeed {
                category: FeedCategory::Personalized,
                force_refresh: false,
            });
        }

        if ui
            .selectable_label(*current_page == ActivePage::Subscriptions, "Subscriptions")
            .clicked()
        {
            *current_page = ActivePage::Subscriptions;
            let _ = cmd_tx.send(UiCommand::LoadSubscriptions {
                force_refresh: false,
            });
        }

        if ui
            .selectable_label(
                *current_page == ActivePage::Playlists
                    || *current_page == ActivePage::PlaylistDetail,
                "Playlists",
            )
            .clicked()
        {
            *current_page = ActivePage::Playlists;
            let _ = cmd_tx.send(UiCommand::LoadPlaylists {
                force_refresh: false,
            });
        }

        if ui
            .selectable_label(
                *current_page == ActivePage::LikedVideos,
                format!("Liked Videos ({})", liked_count),
            )
            .clicked()
        {
            *current_page = ActivePage::LikedVideos;
            let _ = cmd_tx.send(UiCommand::LoadLikedVideos {
                page_token: None,
                force_refresh: false,
            });
        }

        if ui
            .selectable_label(
                *current_page == ActivePage::WatchLater,
                format!("Watch Later ({})", watch_later_count),
            )
            .clicked()
        {
            *current_page = ActivePage::WatchLater;
            let _ = cmd_tx.send(UiCommand::LoadWatchLater);
        }

        if ui
            .selectable_label(
                *current_page == ActivePage::History,
                format!("History ({})", history_count),
            )
            .clicked()
        {
            *current_page = ActivePage::History;
            let _ = cmd_tx.send(UiCommand::LoadHistory { search_query: None });
        }

        ui.separator();

        if ui
            .selectable_label(*current_page == ActivePage::Settings, "Settings & Updates")
            .clicked()
        {
            *current_page = ActivePage::Settings;
        }
    });
}
