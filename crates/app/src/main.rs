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

    let thumbnail_cache = ThumbnailCache::new(&config.cache.thumbnail_dir, config.cache.disk_cache_mb);
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

    // Hook Slint UI Callbacks
    {
        let tx = cmd_tx.clone();
        app_window.on_navigate(move |page_str| {
            let page = match page_str.as_str() {
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
        app_window.on_video_selected(move |id, title| {
            let _ = tx.blocking_send(UiCommand::PlayVideo {
                video_id: id.to_string(),
                quality: "1080p".to_string(),
            });
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

    // Check existing sign-in status on startup
    if let Ok(Some(token)) = auth_manager.get_access_token().await {
        if let Ok((channel_title, _, _)) = api_client.get_my_channel(&token).await {
            let win = window_weak.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    w.set_is_logged_in(true);
                    w.set_user_display_name(channel_title.into());
                }
            });
        }
    }

    // Initial load: Home feed
    let initial_tx = cmd_tx.clone();
    tokio::spawn(async move {
        let _ = initial_tx.send(UiCommand::Navigate(AppPage::Home)).await;
    });

    // Background Service Loop (All UI updates dispatched via invoke_from_event_loop)
    let worker_player = player.clone();
    let worker_ytdlp = ytdlp.clone();
    let worker_auth = auth_manager.clone();
    let worker_api = api_client.clone();
    let worker_history = history_store.clone();
    let worker_watch_later = watch_later_store.clone();
    let worker_window = window_weak.clone();

    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                UiCommand::Navigate(page) => {
                    let page_name = match page {
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
                            w.set_is_loading(true);
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
                        AppPage::Settings => {
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

                    // Check if query is a direct YouTube URL
                    match YouTubeUrlParser::parse(&query) {
                        YouTubeUrlType::Video(id) | YouTubeUrlType::Shorts(id) => {
                            let win = worker_window.clone();
                            let id_clone = id.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_status_message(format!("Loading video {}...", id_clone).into());
                                }
                            });
                            match worker_ytdlp.get_stream_info(&id, "1080p").await {
                                Ok(stream) => {
                                    let p = worker_player.lock().await;
                                    let _ = p.play_stream(&stream, &id, &id).await;
                                    let win = worker_window.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(w) = win.upgrade() {
                                            w.set_current_video_id(id.into());
                                            w.set_is_playing(true);
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
                            // Keyword search
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

                UiCommand::PlayVideo { video_id, quality } => {
                    let win = worker_window.clone();
                    let vid = video_id.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_current_video_id(vid.into());
                            w.set_status_message("Connecting stream...".into());
                        }
                    });

                    match worker_ytdlp.get_stream_info(&video_id, &quality).await {
                        Ok(stream) => {
                            let p = worker_player.lock().await;
                            let _ = p.play_stream(&stream, &video_id, &video_id).await;

                            let win = worker_window.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(w) = win.upgrade() {
                                    w.set_is_playing(true);
                                    w.set_status_message("".into());
                                }
                            });

                            let _ = worker_history.record(VideoItem {
                                id: video_id.clone(),
                                title: format!("Video {}", video_id),
                                channel_name: "YouTube".into(),
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
                            let err_msg = format!("Playback error: {}", e);
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
                }

                UiCommand::SeekAbsolute(pos) => {
                    let p = worker_player.lock().await;
                    let _ = p.handle_command(UiCommand::SeekAbsolute(pos)).await;
                }

                UiCommand::ToggleFullscreen => {
                    let p = worker_player.lock().await;
                    let _ = p.handle_command(UiCommand::ToggleFullscreen).await;
                }

                UiCommand::StartGoogleLogin => {
                    let auth = worker_auth.clone();
                    let api = worker_api.clone();
                    let win = worker_window.clone();
                    tokio::spawn(async move {
                        let win1 = win.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win1.upgrade() {
                                w.set_status_message("Waiting for browser authorization...".into());
                            }
                        });

                        match auth.start_oauth_flow().await {
                            Ok(token) => {
                                if let Ok((title, _, _)) = api.get_my_channel(&token).await {
                                    let win2 = win.clone();
                                    let _ = slint::invoke_from_event_loop(move || {
                                        if let Some(w) = win2.upgrade() {
                                            w.set_is_logged_in(true);
                                            w.set_user_display_name(title.into());
                                            w.set_status_message("Signed in successfully.".into());
                                        }
                                    });
                                }
                            }
                            Err(e) => {
                                let win2 = win.clone();
                                let err_msg = format!("Authentication error: {}", e);
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(w) = win2.upgrade() {
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
                            w.set_is_logged_in(false);
                            w.set_user_display_name("Sign In".into());
                            w.set_status_message("Signed out.".into());
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
