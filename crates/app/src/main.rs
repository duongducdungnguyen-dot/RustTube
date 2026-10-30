slint::include_modules!();

use anyhow::Result;
use rusttube_api::YouTubeApiClient;
use rusttube_auth::GoogleAuthManager;
use rusttube_cache::{HistoryStore, ThumbnailCache, WatchLaterStore};
use rusttube_core::events::UiCommand;
use rusttube_core::models::{AppPage, VideoItem, YouTubeUrlType};
use rusttube_core::{AppConfig, YouTubeUrlParser};
use rusttube_player::PlayerController;
use rusttube_ytdlp::YtDlpWrapper;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting RustTube client...");

    let config = AppConfig::load_or_default("config.toml");

    let thumbnail_cache = Arc::new(ThumbnailCache::new(&config.cache.thumbnail_dir, config.cache.disk_cache_mb));
    let history_store = HistoryStore::new("cache/history.json", config.cache.max_history_items);
    let watch_later_store = WatchLaterStore::new("cache/watch_later.json");

    let auth_manager = Arc::new(GoogleAuthManager::new(
        &config.auth.google_client_id,
        config.auth.redirect_port,
        &config.auth.keyring_service_name,
    ));

    let api_client = Arc::new(YouTubeApiClient::new());
    let ytdlp = Arc::new(YtDlpWrapper::new(&config.ytdlp.binary_path, &config.ytdlp.cookies_browser));

    let player = Arc::new(tokio::sync::Mutex::new(PlayerController::new(&config.player.hwdec)));
    {
        let mut p = player.lock().await;
        if let Err(e) = p.initialize() {
            warn!("libmpv initialization notice: {}", e);
        }
    }

    let (cmd_tx, mut cmd_rx) = mpsc::channel::<UiCommand>(64);

    let app_window = AppWindow::new()?;
    let window_weak = app_window.as_weak();

    // On Windows, locate the top-level native HWND and connect to player wid
    #[cfg(target_os = "windows")]
    {
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::*;
            let title_u16: Vec<u16> = "RustTube\0".encode_utf16().collect();
            let hwnd = FindWindowW(std::ptr::null(), title_u16.as_ptr());
            if hwnd != 0 {
                info!("Bound native RustTube HWND: {}", hwnd);
                let mut p = player.lock().await;
                p.set_window_id(hwnd as isize);
            }
        }
    }

    // Set initial configuration values in Slint UI
    app_window.set_volume_val(config.player.volume as i32);
    app_window.set_current_speed(config.player.speed);
    app_window.set_current_quality(config.player.preferred_quality.clone().into());

    // Hook Slint UI Callbacks to Tokio Channel
    {
        let tx = cmd_tx.clone();
        app_window.on_navigate(move |page_str| {
            let page = match page_str.as_str() {
                "watch" => AppPage::Watch(String::new()),
                "subscriptions" => AppPage::Subscriptions,
                "playlists" => AppPage::Playlists,
                "watch_later" => AppPage::WatchLater,
                "history" => AppPage::History,
                "settings" => AppPage::Settings,
                _ => AppPage::Home,
            };
            let _ = tx.blocking_send(UiCommand::Navigate(page));
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_search_submitted(move |query| {
            let _ = tx.blocking_send(UiCommand::Search(query.to_string()));
        });
    }

    {
        let tx = cmd_tx.clone();
        let preferred_q = config.player.preferred_quality.clone();
        app_window.on_video_selected(move |id, title, channel| {
            let _ = tx.blocking_send(UiCommand::PlayVideo {
                video_id: id.to_string(),
                title: title.to_string(),
                channel: channel.to_string(),
                quality: preferred_q.clone(),
            });
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_volume_changed(move |v| {
            let _ = tx.blocking_send(UiCommand::SetVolume(v as u32));
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_speed_changed(move |s| {
            let _ = tx.blocking_send(UiCommand::SetSpeed(s));
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_quality_changed(move |q| {
            let _ = tx.blocking_send(UiCommand::SetQuality(q.to_string()));
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_toggle_play_pause(move || {
            let _ = tx.blocking_send(UiCommand::TogglePlayPause);
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_seek_requested(move |pos| {
            let _ = tx.blocking_send(UiCommand::SeekAbsolute(pos as f64));
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_toggle_fullscreen(move || {
            let _ = tx.blocking_send(UiCommand::ToggleFullscreen);
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_toggle_subtitles(move || {
            let _ = tx.blocking_send(UiCommand::ToggleSubtitles);
        });
    }

    {
        let tx = cmd_tx.clone();
        let wl_store = watch_later_store.clone();
        let win = window_weak.clone();
        app_window.on_toggle_watch_later(move |vid, title, channel, thumb| {
            let store = wl_store.clone();
            let v_str = vid.to_string();
            let t_str = title.to_string();
            let c_str = channel.to_string();
            let th_str = thumb.to_string();
            let w = win.clone();
            tokio::spawn(async move {
                let item = VideoItem {
                    id: v_str.clone(),
                    title: if t_str.is_empty() { format!("Video {}", v_str) } else { t_str },
                    channel_name: if c_str.is_empty() { "YouTube".into() } else { c_str },
                    channel_id: "".into(),
                    duration_seconds: 0,
                    duration_string: "--:--".into(),
                    view_count: 0,
                    thumbnail_url: th_str,
                    published_time: "".into(),
                };
                if let Ok(saved) = store.toggle(item).await {
                    let msg = if saved { "Saved to Watch Later" } else { "Removed from Watch Later" };
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = w.upgrade() {
                            ui.set_status_message(msg.into());
                        }
                    });
                }
            });
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_login_google(move || {
            let _ = tx.blocking_send(UiCommand::StartGoogleLogin);
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_logout_google(move || {
            let _ = tx.blocking_send(UiCommand::LogoutGoogle);
        });
    }

    {
        let tx = cmd_tx.clone();
        app_window.on_clear_history(move || {
            let _ = tx.blocking_send(UiCommand::ClearHistory);
        });
    }

    // Startup Authentication Check
    if let Ok(Some((token, user_info))) = auth_manager.restore_session().await {
        let win = window_weak.clone();
        let display_name = if user_info.name.is_empty() {
            user_info.email.clone()
        } else {
            user_info.name.clone()
        };
        let email = user_info.email.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(w) = win.upgrade() {
                w.set_is_logged_in(true);
                w.set_user_display_name(display_name.into());
                w.set_user_email(email.into());
                w.set_auth_state_text("Authenticated".into());
            }
        });

        // Load YouTube channel independently without invalidating OAuth if missing
        if let Ok((channel_title, _, _)) = api_client.get_my_channel(&token).await {
            let win = window_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    w.set_channel_name(channel_title.into());
                    w.set_auth_state_text("Fully connected".into());
                }
            });
        }
    }

    // Load initial feed
    let initial_tx = cmd_tx.clone();
    tokio::spawn(async move {
        let _ = initial_tx.send(UiCommand::Navigate(AppPage::Home)).await;
    });

    // Background Service Loop (UI updates executed via invoke_from_event_loop)
    let worker_player = player.clone();
    let worker_ytdlp = ytdlp.clone();
    let worker_auth = auth_manager.clone();
    let worker_api = api_client.clone();
    let worker_history = history_store.clone();
    let worker_watch_later = watch_later_store.clone();
    let worker_window = window_weak.clone();

    tokio::spawn(async move {
        let mut active_video_id = String::new();
        let mut active_video_title = String::new();
        let mut active_video_channel = String::new();

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                UiCommand::Navigate(page) => {
                    let page_name = match page {
                        AppPage::Watch(_) => "watch",
                        AppPage::Subscriptions => "subscriptions",
                        AppPage::Playlists => "playlists",
                        AppPage::WatchLater => "watch_later",
                        AppPage::History => "history",
                        AppPage::Settings => "settings",
                        _ => "home",
                    };

                    let win = worker_window.clone();
                    let page_clone = page_name.to_string();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_current_page(page_clone.into());
                            if page_clone != "watch" && page_clone != "settings" {
                                w.set_is_loading(true);
                            }
                        }
                    });

                    match page {
                        AppPage::Home => {
                            let feed = worker_ytdlp.fetch_trending_feed(24).await.unwrap_or_default();
                            update_slint_videos(&worker_window, feed);
                        }
                        AppPage::History => {
                            let items = worker_history.get_all().await;
                            update_slint_videos(&worker_window, items);
                        }
                        AppPage::WatchLater => {
                            let items = worker_watch_later.get_all().await;
                            update_slint_videos(&worker_window, items);
                        }
                        AppPage::Subscriptions => {
                            if let Ok(Some(token)) = worker_auth.get_access_token().await {
                                if let Ok((subs, _)) = worker_api.get_subscriptions(&token, None).await {
                                    info!("Loaded {} subscriptions", subs.len());
                                }
                            }
                            let win = worker_window.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_is_loading(false);
                                }
                            });
                        }
                        AppPage::Playlists => {
                            if let Ok(Some(token)) = worker_auth.get_access_token().await {
                                if let Ok((pl, _)) = worker_api.get_playlists(&token, None).await {
                                    info!("Loaded {} playlists", pl.len());
                                }
                            }
                            let win = worker_window.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_is_loading(false);
                                }
                            });
                        }
                        _ => {}
                    }
                }

                UiCommand::Search(raw_query) => {
                    let query = raw_query.trim().to_string();
                    if query.is_empty() {
                        continue;
                    }

                    match YouTubeUrlParser::parse(&query) {
                        YouTubeUrlType::Video(id) | YouTubeUrlType::Shorts(id) => {
                            let win = worker_window.clone();
                            let id_clone = id.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_current_page("watch".into());
                                    w.set_current_video_id(id_clone.clone().into());
                                    w.set_status_message(format!("Resolving video {}...", id_clone).into());
                                }
                            });

                            let ytdlp_meta = worker_ytdlp.clone();
                            let win_meta = worker_window.clone();
                            let id_meta = id.clone();
                            let video_meta = ytdlp_meta.get_video_detail(&id_meta).await.ok();

                            let final_title = video_meta.as_ref().map(|d| d.title.clone()).unwrap_or_else(|| format!("YouTube Video ({})", id_meta));
                            let final_channel = video_meta.as_ref().map(|d| d.channel.clone()).unwrap_or_else(|| "YouTube".to_string());
                            let final_desc = video_meta.as_ref().map(|d| d.description.clone()).unwrap_or_default();

                            let win_set = win_meta.clone();
                            let t_set = final_title.clone();
                            let c_set = final_channel.clone();
                            let d_set = final_desc.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win_set.upgrade() {
                                    w.set_current_video_title(t_set.into());
                                    w.set_current_video_channel(c_set.into());
                                    w.set_current_video_description(d_set.into());
                                }
                            });

                            match worker_ytdlp.get_stream_info(&id, "1080p").await {
                                Ok(stream) => {
                                    let p = worker_player.lock().await;
                                    let _ = p.play_stream(&stream, &id, &final_title).await;
                                    let win = worker_window.clone();
                                    let actual_res = stream.resolution.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(w) = win.upgrade() {
                                            w.set_is_playing(true);
                                            w.set_current_quality(actual_res.into());
                                            w.set_status_message("".into());
                                        }
                                    });
                                }
                                Err(e) => {
                                    let win = worker_window.clone();
                                    let err_msg = format!("Playback error: {}", e);
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(w) = win.upgrade() {
                                            w.set_status_message(err_msg.into());
                                        }
                                    });
                                }
                            }
                        }
                        _ => {
                            let win = worker_window.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_is_loading(true);
                                    w.set_current_page("home".into());
                                }
                            });

                            let results = worker_ytdlp.search_videos(&query, 24).await.unwrap_or_default();
                            update_slint_videos(&worker_window, results);
                        }
                    }
                }

                UiCommand::PlayVideo { video_id, title, channel, quality } => {
                    active_video_id = video_id.clone();
                    active_video_title = title.clone();
                    active_video_channel = channel.clone();

                    let win = worker_window.clone();
                    let vid = video_id.clone();
                    let t_clone = title.clone();
                    let c_clone = channel.clone();
                    let q_clone = quality.clone();

                    // Switch immediately to dedicated Watch page
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_current_page("watch".into());
                            w.set_current_video_id(vid.into());
                            w.set_current_video_title(t_clone.into());
                            w.set_current_video_channel(c_clone.into());
                            w.set_current_quality(q_clone.into());
                            w.set_status_message("Extracting playable streams...".into());
                        }
                    });

                    // Retrieve description and metadata in background
                    let ytdlp_desc = worker_ytdlp.clone();
                    let win_desc = worker_window.clone();
                    let vid_desc = video_id.clone();
                    tokio::spawn(async move {
                        if let Ok(detail) = ytdlp_desc.get_video_detail(&vid_desc).await {
                            let d = detail.description;
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win_desc.upgrade() {
                                    w.set_current_video_description(d.into());
                                }
                            });
                        }
                    });

                    // Resolve audio and video streams
                    match worker_ytdlp.get_stream_info(&video_id, &quality).await {
                        Ok(stream) => {
                            let p = worker_player.lock().await;
                            if let Err(e) = p.play_stream(&stream, &video_id, &title).await {
                                error!("Playback start failed: {}", e);
                            }

                            let win = worker_window.clone();
                            let actual_res = stream.resolution.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_is_playing(true);
                                    w.set_current_quality(actual_res.into());
                                    w.set_status_message("".into());
                                }
                            });

                            // Record actual video metadata to local history
                            let _ = worker_history.record(VideoItem {
                                id: video_id.clone(),
                                title: title.clone(),
                                channel_name: channel.clone(),
                                channel_id: "".into(),
                                duration_seconds: 0,
                                duration_string: "--:--".into(),
                                view_count: 0,
                                thumbnail_url: "".into(),
                                published_time: "".into(),
                            }).await;
                        }
                        Err(e) => {
                            let win = worker_window.clone();
                            let err_msg = format!("{}", e);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_status_message(err_msg.into());
                                }
                            });
                        }
                    }
                }

                UiCommand::TogglePlayPause => {
                    let p = worker_player.lock().await;
                    let _ = p.handle_command(UiCommand::TogglePlayPause).await;
                    let status = p.poll_status().await;
                    let win = worker_window.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_is_playing(status.is_playing);
                        }
                    });
                }

                UiCommand::SetVolume(vol) => {
                    let p = worker_player.lock().await;
                    let _ = p.handle_command(UiCommand::SetVolume(vol)).await;
                    let win = worker_window.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_volume_val(vol as i32);
                        }
                    });
                }

                UiCommand::SetSpeed(speed) => {
                    let p = worker_player.lock().await;
                    let _ = p.handle_command(UiCommand::SetSpeed(speed)).await;
                    let win = worker_window.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_current_speed(speed);
                        }
                    });
                }

                UiCommand::SetQuality(quality) => {
                    if active_video_id.is_empty() {
                        continue;
                    }
                    let vid = active_video_id.clone();
                    let title = active_video_title.clone();
                    let win = worker_window.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_status_message(format!("Switching quality to {}...", quality).into());
                        }
                    });

                    let p_lock = worker_player.lock().await;
                    let current_pos = p_lock.poll_status().await.position_seconds;
                    drop(p_lock);

                    match worker_ytdlp.get_stream_info(&vid, &quality).await {
                        Ok(stream) => {
                            let p = worker_player.lock().await;
                            let _ = p.play_stream(&stream, &vid, &title).await;
                            let _ = p.seek_absolute(current_pos).await;
                            let win = worker_window.clone();
                            let actual_res = stream.resolution.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_current_quality(actual_res.into());
                                    w.set_is_playing(true);
                                    w.set_status_message("".into());
                                }
                            });
                        }
                        Err(e) => {
                            let win = worker_window.clone();
                            let err_msg = format!("Quality switch error: {}", e);
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_status_message(err_msg.into());
                                }
                            });
                        }
                    }
                }

                UiCommand::SeekAbsolute(pos) => {
                    let p = worker_player.lock().await;
                    let _ = p.handle_command(UiCommand::SeekAbsolute(pos)).await;
                }

                UiCommand::ToggleFullscreen => {
                    let p = worker_player.lock().await;
                    let _ = p.handle_command(UiCommand::ToggleFullscreen).await;
                }

                UiCommand::ToggleSubtitles => {
                    let p = worker_player.lock().await;
                    let _ = p.handle_command(UiCommand::ToggleSubtitles).await;
                }

                UiCommand::StartGoogleLogin => {
                    if worker_auth.is_authenticating() {
                        let win = worker_window.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win.upgrade() {
                                w.set_status_message("Authentication already in progress...".into());
                            }
                        });
                        continue;
                    }

                    let auth = worker_auth.clone();
                    let api = worker_api.clone();
                    let win = worker_window.clone();
                    tokio::spawn(async move {
                        let win1 = win.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win1.upgrade() {
                                w.set_is_authenticating(true);
                                w.set_auth_state_text("Waiting for account selection in browser...".into());
                                w.set_status_message("Opening Google sign-in in browser...".into());
                            }
                        });

                        match auth.start_oauth_flow().await {
                            Ok((token, user_info)) => {
                                let win2 = win.clone();
                                let display_name = if user_info.name.is_empty() {
                                    user_info.email.clone()
                                } else {
                                    user_info.name.clone()
                                };
                                let email = user_info.email.clone();
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(w) = win2.upgrade() {
                                        w.set_is_authenticating(false);
                                        w.set_is_logged_in(true);
                                        w.set_user_display_name(display_name.into());
                                        w.set_user_email(email.into());
                                        w.set_auth_state_text("Google authenticated".into());
                                        w.set_status_message("Signed in with Google. Loading YouTube channel...".into());
                                    }
                                });

                                // Fetch YouTube channel separately so failure does not invalidate OAuth
                                match api.get_my_channel(&token).await {
                                    Ok((channel_title, _, _)) => {
                                        let win3 = win.clone();
                                        let _ = slint::invoke_from_event_loop(move || {
                                            if let Some(w) = win3.upgrade() {
                                                w.set_channel_name(channel_title.into());
                                                w.set_auth_state_text("Fully connected".into());
                                                w.set_status_message("YouTube account connected successfully.".into());
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        warn!("Could not fetch YouTube channel for account: {}", e);
                                        let win3 = win.clone();
                                        let _ = slint::invoke_from_event_loop(move || {
                                            if let Some(w) = win3.upgrade() {
                                                w.set_channel_name("".into());
                                                w.set_auth_state_text("Google authenticated (no YouTube channel)".into());
                                                w.set_status_message("Google authenticated (channel not found or accessible)".into());
                                            }
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                let win2 = win.clone();
                                let err_msg = format!("Google sign in error: {}", e);
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(w) = win2.upgrade() {
                                        w.set_is_authenticating(false);
                                        w.set_auth_state_text("Authentication cancelled or failed".into());
                                        w.set_status_message(err_msg.into());
                                    }
                                });
                            }
                        }
                    });
                }

                UiCommand::LogoutGoogle => {
                    let _ = worker_auth.logout().await;
                    let win = worker_window.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_is_authenticating(false);
                            w.set_is_logged_in(false);
                            w.set_user_display_name("Sign In".into());
                            w.set_user_email("".into());
                            w.set_channel_name("".into());
                            w.set_auth_state_text("Signed out".into());
                            w.set_status_message("Signed out and cleared credentials.".into());
                        }
                    });
                }

                UiCommand::ClearHistory => {
                    let _ = worker_history.clear().await;
                    let win = worker_window.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_status_message("Watch history cleared.".into());
                        }
                    });
                }

                _ => {}
            }
        }
    });

    // Polling Player Status Ticker
    let ticker_player = player.clone();
    let ticker_window = window_weak.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        loop {
            interval.tick().await;
            let status = {
                let p = ticker_player.lock().await;
                p.poll_status().await
            };

            let win = ticker_window.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    w.set_is_playing(status.is_playing);
                    w.set_playback_position(status.position_seconds as f32);
                    w.set_playback_duration(status.duration_seconds.max(1.0) as f32);
                    let cur_m = (status.position_seconds as u32) / 60;
                    let cur_s = (status.position_seconds as u32) % 60;
                    let dur_m = (status.duration_seconds as u32) / 60;
                    let dur_s = (status.duration_seconds as u32) % 60;
                    w.set_time_display(format!("{}:{:02} / {}:{:02}", cur_m, cur_s, dur_m, dur_s).into());
                }
            });
        }
    });

    app_window.run()?;
    Ok(())
}

fn update_slint_videos(window: &slint::Weak<AppWindow>, items: Vec<VideoItem>) {
    let cards: Vec<VideoCardData> = items
        .into_iter()
        .map(|item| VideoCardData {
            id: item.id.into(),
            title: item.title.into(),
            channel: item.channel_name.into(),
            duration: item.duration_string.into(),
            views: format_views(item.view_count).into(),
            thumbnail_url: item.thumbnail_url.into(),
        })
        .collect();

    let win = window.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = win.upgrade() {
            let model = Rc::new(VecModel::from(cards));
            w.set_video_list(ModelRc::from(model));
            w.set_is_loading(false);
        }
    });
}

fn format_views(views: u64) -> String {
    if views >= 1_000_000 {
        format!("{:.1}M views", views as f64 / 1_000_000.0)
    } else if views >= 1_000 {
        format!("{:.1}K views", views as f64 / 1_000.0)
    } else if views > 0 {
        format!("{} views", views)
    } else {
        "".to_string()
    }
}
