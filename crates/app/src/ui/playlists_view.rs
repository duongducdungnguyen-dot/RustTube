use crate::cache::ThumbnailManager;
use crate::theme::THEME;
use egui::{RichText, Ui, Vec2};
use rusttube_core::models::{PlaylistItem, VideoItem};

pub fn render_playlists_view(
    ui: &mut Ui,
    playlists: &[PlaylistItem],
    selected_playlist: &Option<PlaylistItem>,
    playlist_items: &[VideoItem],
    thumbnails: &ThumbnailManager,
    is_logged_in: bool,
    on_login_click: &mut bool,
    on_select_playlist: &mut Option<PlaylistItem>,
    on_select_video: &mut Option<VideoItem>,
    new_playlist_title: &mut String,
    _new_playlist_desc: &mut String,
    on_create_playlist: &mut bool,
    on_delete_playlist: &mut Option<String>,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Your Playlists")
                    .size(18.0)
                    .strong()
                    .color(THEME.text_white),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if is_logged_in {
                    ui.menu_button(
                        RichText::new("➕ New Playlist").size(12.0).strong(),
                        |ui| {
                            ui.label(RichText::new("Create New YouTube Playlist").strong());
                            ui.add_space(4.0);
                            ui.text_edit_singleline(new_playlist_title);
                            ui.add_space(4.0);
                            if ui.button("Create").clicked() {
                                if !new_playlist_title.trim().is_empty() {
                                    *on_create_playlist = true;
                                    ui.close_menu();
                                }
                            }
                        },
                    );
                }
            });
        });

        ui.add_space(16.0);

        if !is_logged_in {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("Sign in with Google to access and edit your YouTube playlists.")
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

        if let Some(pl) = selected_playlist {
            ui.horizontal(|ui| {
                if ui.button("← All Playlists").clicked() {
                    *on_select_playlist = None;
                }
                ui.add_space(10.0);
                ui.label(
                    RichText::new(&pl.title)
                        .size(16.0)
                        .strong()
                        .color(THEME.text_white),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new("🗑 Delete Playlist").size(11.0).color(THEME.crimson)).clicked() {
                        *on_delete_playlist = Some(pl.id.clone());
                    }
                });
            });

            ui.add_space(12.0);

            if playlist_items.is_empty() {
                ui.label(
                    RichText::new("No videos found in this playlist.")
                        .size(12.0)
                        .color(THEME.text_subdued),
                );
            } else {
                crate::ui::video_grid::render_video_grid(
                    ui,
                    playlist_items,
                    thumbnails,
                    &[],
                    on_select_video,
                    &mut None,
                );
            }

            return;
        }

        if playlists.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("No playlists found.")
                        .size(13.0)
                        .color(THEME.text_subdued),
                );
                ui.label(
                    RichText::new("Click 'Sync' above to fetch playlists or create a new playlist.")
                        .size(11.0)
                        .color(THEME.text_muted),
                );
            });
            return;
        }

        egui::Grid::new("playlists_grid")
            .spacing(Vec2::new(16.0, 16.0))
            .show(ui, |ui| {
                for (idx, pl) in playlists.iter().enumerate() {
                    if idx > 0 && idx % 3 == 0 {
                        ui.end_row();
                    }

                    let response = ui.group(|ui| {
                        ui.set_width(260.0);
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(&pl.title)
                                    .size(14.0)
                                    .strong()
                                    .color(THEME.text_white),
                            );
                            ui.label(
                                RichText::new(format!("{} videos", pl.video_count))
                                    .size(11.0)
                                    .color(THEME.text_muted),
                            );
                        });
                    });

                    if response.response.clicked() {
                        *on_select_playlist = Some(pl.clone());
                    }
                }
            });
    });
}
