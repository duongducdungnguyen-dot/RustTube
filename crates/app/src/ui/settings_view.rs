use crate::theme::THEME;
use egui::{RichText, Ui};
use rusttube_auth::GoogleUserInfo;
use rusttube_core::models::AppUpdateInfo;

pub fn render_settings_view(
    ui: &mut Ui,
    user_info: Option<&GoogleUserInfo>,
    is_authenticating: bool,
    cookies_browser: &mut String,
    hwdec_setting: &mut String,
    update_info: Option<&AppUpdateInfo>,
    is_checking_update: bool,
    on_login_click: &mut bool,
    on_logout_click: &mut bool,
    on_clear_history: &mut bool,
    on_clear_cache: &mut bool,
    on_check_update: &mut bool,
    on_apply_update: &mut bool,
) {
    ui.vertical(|ui| {
        ui.label(
            RichText::new("Application Settings")
                .size(18.0)
                .strong()
                .color(THEME.text_white),
        );
        ui.label(
            RichText::new("Manage Google authentication, browser cookies, application updates, and local database storage.")
                .size(12.0)
                .color(THEME.text_muted),
        );

        ui.add_space(20.0);

        // Google Account Group
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("Google Account & OAuth 2.0 PKCE")
                    .size(14.0)
                    .strong()
                    .color(THEME.text_white),
            );
            ui.add_space(6.0);

            if let Some(user) = user_info {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Connected Account:")
                            .size(12.0)
                            .color(THEME.text_subdued),
                    );
                    ui.label(
                        RichText::new(&user.name)
                            .size(12.0)
                            .strong()
                            .color(THEME.text_white),
                    );
                    ui.label(
                        RichText::new(format!("({})", user.email))
                            .size(11.0)
                            .color(THEME.text_muted),
                    );
                });

                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Switch Account").clicked() {
                        *on_login_click = true;
                    }
                    if ui
                        .button(RichText::new("Sign Out").color(THEME.crimson))
                        .clicked()
                    {
                        *on_logout_click = true;
                    }
                });
            } else if is_authenticating {
                ui.label(
                    RichText::new("⏳ Waiting for Google account selection in browser...")
                        .size(12.0)
                        .color(THEME.crimson),
                );
            } else {
                ui.label(
                    RichText::new("Status: Not Signed In")
                        .size(12.0)
                        .color(THEME.text_subdued),
                );
                ui.add_space(8.0);
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
            }
        });

        ui.add_space(16.0);

        // Native Application Updates Group
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("Software Updates (GitHub Releases)")
                    .size(14.0)
                    .strong()
                    .color(THEME.text_white),
            );
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Installed Version: v{}", env!("CARGO_PKG_VERSION")))
                        .size(12.0)
                        .color(THEME.text_subdued),
                );

                ui.add_space(12.0);

                if is_checking_update {
                    ui.label(
                        RichText::new("🔄 Checking GitHub for updates...")
                            .size(12.0)
                            .color(THEME.text_subdued),
                    );
                } else if ui
                    .button(RichText::new("🔍 Check for Updates").size(11.5))
                    .clicked()
                {
                    *on_check_update = true;
                }
            });

            if let Some(info) = update_info {
                ui.add_space(8.0);
                if info.is_update_available {
                    ui.label(
                        RichText::new(format!("🎉 New version available: v{}", info.latest_version))
                            .size(13.0)
                            .strong()
                            .color(THEME.crimson),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(&info.release_notes)
                            .size(11.0)
                            .color(THEME.text_muted),
                    );
                    ui.add_space(8.0);
                    if ui
                        .button(
                            RichText::new("🚀 Download & Apply Update")
                                .size(12.0)
                                .strong()
                                .color(THEME.text_white),
                        )
                        .clicked()
                    {
                        *on_apply_update = true;
                    }
                } else {
                    ui.label(
                        RichText::new("✓ RustTube is up to date.")
                            .size(12.0)
                            .color(THEME.text_subdued),
                    );
                }
            }
        });

        ui.add_space(16.0);

        // Playback & Hardware Decoding Group
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("Playback & Hardware Acceleration")
                    .size(14.0)
                    .strong()
                    .color(THEME.text_white),
            );
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Hardware Decoder (hwdec):")
                        .size(12.0)
                        .color(THEME.text_subdued),
                );
                egui::ComboBox::from_id_source("hwdec_combo")
                    .selected_text(hwdec_setting.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(hwdec_setting, "auto-safe".to_string(), "auto-safe (Recommended)");
                        ui.selectable_value(hwdec_setting, "d3d11va".to_string(), "d3d11va (Windows DirectX)");
                        ui.selectable_value(hwdec_setting, "nvdec".to_string(), "nvdec (NVIDIA GPU)");
                        ui.selectable_value(hwdec_setting, "no".to_string(), "no (Software decoding)");
                    });
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Browser Cookies for yt-dlp:")
                        .size(12.0)
                        .color(THEME.text_subdued),
                );
                egui::ComboBox::from_id_source("cookies_combo")
                    .selected_text(cookies_browser.as_str())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(cookies_browser, "chrome".to_string(), "Google Chrome");
                        ui.selectable_value(cookies_browser, "edge".to_string(), "Microsoft Edge");
                        ui.selectable_value(cookies_browser, "firefox".to_string(), "Mozilla Firefox");
                        ui.selectable_value(cookies_browser, "none".to_string(), "None (Public Streams)");
                    });
            });
        });

        ui.add_space(16.0);

        // Database & Cache Group
        ui.group(|ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("Local Database & Disk Cache")
                    .size(14.0)
                    .strong()
                    .color(THEME.text_white),
            );
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                if ui.button("Clear Watch History").clicked() {
                    *on_clear_history = true;
                }
                if ui.button("Clear Thumbnail Cache").clicked() {
                    *on_clear_cache = true;
                }
            });
        });
    });
}
