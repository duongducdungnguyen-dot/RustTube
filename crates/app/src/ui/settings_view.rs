use egui::Ui;
use rusttube_core::{AppConfig, SyncState, UiCommand, UpdateState, UserProfile, APP_VERSION};
use tokio::sync::mpsc::UnboundedSender;

pub fn render_settings_view(
    ui: &mut Ui,
    config: &mut AppConfig,
    profile: &Option<UserProfile>,
    auth_in_progress: bool,
    sync_state: &SyncState,
    update_state: &UpdateState,
    cmd_tx: &UnboundedSender<UiCommand>,
) {
    ui.heading("Application Settings & Native Updater");
    ui.separator();

    // 1. Account & Synchronization
    ui.group(|ui| {
        ui.heading("Google Account & Synchronization");
        ui.horizontal(|ui| {
            ui.label("OAuth Client ID:");
            ui.add(
                egui::TextEdit::singleline(&mut config.auth.google_client_id)
                    .hint_text("1234567890-...apps.googleusercontent.com")
                    .desired_width(420.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("OAuth Client Secret:");
            ui.add(
                egui::TextEdit::singleline(&mut config.auth.google_client_secret)
                    .password(true)
                    .hint_text("Optional Desktop client secret (stored locally in %APPDATA%)")
                    .desired_width(420.0),
            );
        });
        ui.add_space(4.0);

        if let Some(user) = profile {
            ui.label(format!("Connected as: {} ({})", user.display_name, user.email));
            if ui.button("Sign Out").clicked() {
                let _ = cmd_tx.send(UiCommand::Logout);
            }
        } else if auth_in_progress {
            ui.label("Sign-in already in progress — complete authorization in your browser or cancel to retry.");
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new("Sign-in in progress..."));
                if ui.button("Cancel Sign-In").clicked() {
                    let _ = cmd_tx.send(UiCommand::CancelGoogleLogin);
                }
            });
        } else {
            ui.label("Not signed in. Sign in with Google OAuth2 PKCE to sync subscriptions, playlists, and likes.");
            if ui.button("Sign In with Google").clicked() {
                let _ = cmd_tx.send(UiCommand::SaveSettings(config.clone()));
                let _ = cmd_tx.send(UiCommand::StartGoogleLogin);
            }
        }

        ui.checkbox(&mut config.sync.auto_sync_enabled, "Enable automatic background synchronization");
        if ui
            .add_enabled(!sync_state.is_syncing, egui::Button::new("Sync Account Now"))
            .clicked()
        {
            let _ = cmd_tx.send(UiCommand::TriggerAccountSync);
        }
        if let Some(ref err) = sync_state.last_error {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("Sync Error ({:?}): {}", err.kind, err.message),
            );
        }
    });

    ui.add_space(10.0);

    // 2. Native Windows Application Updater
    ui.group(|ui| {
        ui.heading("Application Updates (GitHub Releases)");
        ui.label(format!("Installed Version: v{}", APP_VERSION));
        ui.checkbox(
            &mut config.updater.auto_check_updates,
            "Automatically check GitHub Releases on startup",
        );

        if ui.button("Check for Updates Now").clicked() {
            let _ = cmd_tx.send(UiCommand::CheckForUpdates {
                manual_trigger: true,
            });
        }

        match update_state {
            UpdateState::Idle => {}
            UpdateState::Checking => {
                ui.label("Checking GitHub Releases for updates...");
            }
            UpdateState::UpToDate {
                current_version, ..
            } => {
                ui.label(format!("RustTube v{} is up to date.", current_version));
            }
            UpdateState::UpdateAvailable { manifest, .. } => {
                ui.label(format!(
                    "New Release Available: v{} ({})",
                    manifest.version, manifest.asset_name
                ));
                ui.label(&manifest.release_notes);
                if ui.button("Download & Stage Update").clicked() {
                    let _ = cmd_tx.send(UiCommand::DownloadAndStageUpdate {
                        manifest: manifest.clone(),
                    });
                }
            }
            UpdateState::Downloading {
                version,
                downloaded_bytes,
                total_bytes,
            } => {
                ui.label(format!(
                    "Downloading v{} ({} / {} bytes)...",
                    version, downloaded_bytes, total_bytes
                ));
            }
            UpdateState::StagedReadyToRestart {
                version,
                staged_path,
                sha256_verified,
            } => {
                ui.label(format!(
                    "Update v{} staged (SHA-256 verified: {}). User data, SQLite DB, and OAuth tokens will be preserved.",
                    version, sha256_verified
                ));
                if ui.button("Restart & Apply Update").clicked() {
                    let _ = cmd_tx.send(UiCommand::ApplyStagedUpdateAndRestart {
                        staged_zip_path: staged_path.clone(),
                        target_version: version.clone(),
                    });
                }
            }
            UpdateState::Failed {
                message,
                rolled_back,
            } => {
                ui.colored_label(
                    egui::Color32::LIGHT_RED,
                    format!("Update failed (rolled back: {}): {}", rolled_back, message),
                );
            }
        }
    });

    ui.add_space(10.0);
    if ui.button("Save Settings").clicked() {
        let _ = cmd_tx.send(UiCommand::SaveSettings(config.clone()));
    }
}
