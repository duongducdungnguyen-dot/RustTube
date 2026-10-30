use crate::ui::ActivePage;
use egui::Ui;
use rusttube_core::{
    PlaylistItem, PlaylistPrivacy, PlaylistVideoItem, UiCommand, VideoAvailability, VideoItem,
};
use tokio::sync::mpsc::UnboundedSender;

pub struct PlaylistFormState {
    pub new_title: String,
    pub new_description: String,
    pub new_privacy: PlaylistPrivacy,
    pub edit_title: String,
    pub edit_description: String,
    pub edit_privacy: PlaylistPrivacy,
    pub confirm_delete_id: Option<String>,
}

impl Default for PlaylistFormState {
    fn default() -> Self {
        Self {
            new_title: String::new(),
            new_description: String::new(),
            new_privacy: PlaylistPrivacy::Private,
            edit_title: String::new(),
            edit_description: String::new(),
            edit_privacy: PlaylistPrivacy::Private,
            confirm_delete_id: None,
        }
    }
}

pub fn render_playlists_overview(
    ui: &mut Ui,
    playlists: &[PlaylistItem],
    form: &mut PlaylistFormState,
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    ui.horizontal(|ui| {
        ui.heading(format!("Your Playlists ({})", playlists.len()));
        if ui.button("Refresh Playlists").clicked() {
            let _ = cmd_tx.send(UiCommand::LoadPlaylists {
                force_refresh: true,
            });
        }
    });

    ui.group(|ui| {
        ui.label("Create New Playlist");
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut form.new_title)
                    .hint_text("Playlist title")
                    .desired_width(180.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut form.new_description)
                    .hint_text("Description")
                    .desired_width(220.0),
            );
            egui::ComboBox::from_id_salt("new_playlist_privacy")
                .selected_text(form.new_privacy.as_api_str())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.new_privacy, PlaylistPrivacy::Private, "private");
                    ui.selectable_value(
                        &mut form.new_privacy,
                        PlaylistPrivacy::Unlisted,
                        "unlisted",
                    );
                    ui.selectable_value(&mut form.new_privacy, PlaylistPrivacy::Public, "public");
                });
            if ui
                .add_enabled(
                    !form.new_title.trim().is_empty(),
                    egui::Button::new("Create Playlist"),
                )
                .clicked()
            {
                let _ = cmd_tx.send(UiCommand::CreatePlaylist {
                    title: form.new_title.trim().to_string(),
                    description: form.new_description.trim().to_string(),
                    privacy: form.new_privacy,
                });
                form.new_title.clear();
                form.new_description.clear();
            }
        });
    });

    ui.separator();

    for pl in playlists {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                if ui
                    .button(format!(
                        "📂 {} ({} videos · {})",
                        pl.title,
                        pl.item_count,
                        pl.privacy_status.as_api_str()
                    ))
                    .clicked()
                {
                    form.edit_title = pl.title.clone();
                    form.edit_description = pl.description.clone();
                    form.edit_privacy = pl.privacy_status;
                    *current_page = ActivePage::PlaylistDetail;
                    let _ = cmd_tx.send(UiCommand::OpenPlaylist {
                        playlist_id: pl.playlist_id.clone(),
                        page_token: None,
                    });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if form.confirm_delete_id.as_deref() == Some(&pl.playlist_id) {
                        if ui.button("Confirm Delete").clicked() {
                            let _ = cmd_tx.send(UiCommand::DeletePlaylist {
                                playlist_id: pl.playlist_id.clone(),
                            });
                            form.confirm_delete_id = None;
                        }
                        if ui.button("Cancel").clicked() {
                            form.confirm_delete_id = None;
                        }
                    } else if ui.button("Delete Playlist").clicked() {
                        form.confirm_delete_id = Some(pl.playlist_id.clone());
                    }
                });
            });
        });
    }
}

pub fn render_playlist_detail_view(
    ui: &mut Ui,
    playlist: Option<&PlaylistItem>,
    items: &[PlaylistVideoItem],
    next_page_token: &Option<String>,
    form: &mut PlaylistFormState,
    current_page: &mut ActivePage,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    let Some(pl) = playlist else {
        ui.label("Loading playlist contents...");
        return;
    };

    ui.horizontal(|ui| {
        if ui.button("← All Playlists").clicked() {
            *current_page = ActivePage::Playlists;
        }
        ui.heading(&pl.title);
        if !items.is_empty() && ui.button("▶ Play All Sequentially").clicked() {
            let queue: Vec<VideoItem> = items
                .iter()
                .filter(|i| i.availability_status == VideoAvailability::Available)
                .map(|i| i.video.clone())
                .collect();
            if let Some(first) = queue.first().cloned() {
                *current_page = ActivePage::Watch;
                let _ = cmd_tx.send(UiCommand::PlayVideo {
                    video: first,
                    playlist_queue: Some(queue),
                    resume_from_seconds: None,
                });
            }
        }
    });

    ui.group(|ui| {
        ui.label("Edit Playlist Metadata & Privacy");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut form.edit_title).desired_width(180.0));
            ui.add(egui::TextEdit::singleline(&mut form.edit_description).desired_width(220.0));
            egui::ComboBox::from_id_salt("edit_playlist_privacy")
                .selected_text(form.edit_privacy.as_api_str())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut form.edit_privacy, PlaylistPrivacy::Private, "private");
                    ui.selectable_value(
                        &mut form.edit_privacy,
                        PlaylistPrivacy::Unlisted,
                        "unlisted",
                    );
                    ui.selectable_value(&mut form.edit_privacy, PlaylistPrivacy::Public, "public");
                });
            if ui.button("Save Changes").clicked() {
                let _ = cmd_tx.send(UiCommand::UpdatePlaylist {
                    playlist_id: pl.playlist_id.clone(),
                    title: form.edit_title.trim().to_string(),
                    description: form.edit_description.trim().to_string(),
                    privacy: form.edit_privacy,
                });
            }
        });
    });

    ui.separator();

    for item in items {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(format!("#{}", item.position + 1));
                match item.availability_status {
                    VideoAvailability::Available => {
                        if ui.button(&item.video.title).clicked() {
                            *current_page = ActivePage::Watch;
                            let _ = cmd_tx.send(UiCommand::PlayVideo {
                                video: item.video.clone(),
                                playlist_queue: None,
                                resume_from_seconds: None,
                            });
                        }
                    }
                    VideoAvailability::Private => {
                        ui.label("[Private Video - Unavailable]");
                    }
                    VideoAvailability::Deleted => {
                        ui.label("[Deleted Video]");
                    }
                    VideoAvailability::Unavailable => {
                        ui.label("[Unavailable Video]");
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Remove from Playlist").clicked() {
                        let _ = cmd_tx.send(UiCommand::RemoveVideoFromPlaylist {
                            playlist_id: pl.playlist_id.clone(),
                            playlist_item_id: item.playlist_item_id.clone(),
                        });
                    }
                });
            });
        });
    }

    if let Some(ref token) = next_page_token {
        if ui.button("Load More Playlist Items").clicked() {
            let _ = cmd_tx.send(UiCommand::OpenPlaylist {
                playlist_id: pl.playlist_id.clone(),
                page_token: Some(token.clone()),
            });
        }
    }
}
