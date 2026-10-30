use crate::cache::ThumbnailManager;
use crate::theme::{configure_theme, THEME};
use crate::ui::{
    render_header, render_history_view, render_playlists_view, render_settings_view,
    render_sidebar, render_subscriptions_view, render_video_grid, render_watch_later_view,
    render_watch_view, Page,
};
use eframe::App;
use egui::{CentralPanel, Context, SidePanel, TopBottomPanel};
use rusttube_api::YouTubeApiClient;
use rusttube_auth::{GoogleAuthManager, GoogleUserInfo};
use rusttube_core::models::{ChannelItem, PlaybackStatus, PlaylistItem, VideoItem};
use rusttube_player::PlayerController;
use rusttube_storage::{Database, HistoryRepository, SyncRepository, WatchLaterRepository};
use rusttube_ytdlp::YtDlpWrapper;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

pub enum WorkerEvent {
    FeedLoaded(Vec<VideoItem>),
    HistoryUpdated(Vec<VideoItem>),
    WatchLaterUpdated(Vec<VideoItem>),
    SubscriptionsLoaded(Vec<ChannelItem>),
    PlaylistsLoaded(Vec<PlaylistItem>),
    PlaylistItemsLoaded(Vec<VideoItem>),
    LikedVideosLoaded(Vec<VideoItem>),
    AuthSuccess(GoogleUserInfo),
    AuthLoggedOut,
    SyncFinished,
    PlaybackStatusUpdated(PlaybackStatus),
    VideoDetailLoaded { description: String },
    StatusMessage(String),
}

pub struct RustTubeApp {
    db: Database,
    history_repo: HistoryRepository,
    watch_later_repo: WatchLaterRepository,
    sync_repo: SyncRepository,
    auth_manager: Arc<GoogleAuthManager>,
    api_client: Arc<YouTubeApiClient>,
    ytdlp: Arc<YtDlpWrapper>,
    player: Arc<tokio::sync::Mutex<PlayerController>>,
    thumbnails: ThumbnailManager,

    // UI state
    current_page: Page,
    search_query: String,
    status_message: String,
    is_loading: bool,
    is_syncing: bool,
    last_sync_text: String,

    // Lists
    feed_videos: Vec<VideoItem>,
    history_videos: Vec<VideoItem>,
    watch_later_videos: Vec<VideoItem>,
    subscriptions: Vec<ChannelItem>,
    playlists: Vec<PlaylistItem>,
    liked_videos: Vec<VideoItem>,
    selected_playlist: Option<PlaylistItem>,
    playlist_items: Vec<VideoItem>,

    // Selected / Playing
    selected_video: Option<VideoItem>,
    video_description: String,
    playback_status: PlaybackStatus,

    // Auth
    user_info: Option<GoogleUserInfo>,
    is_authenticating: bool,

    // Settings
    cookies_browser: String,
    hwdec_setting: String,
    new_playlist_title: String,
    new_playlist_desc: String,

    // Channels
    worker_tx: mpsc::Sender<WorkerEvent>,
    worker_rx: mpsc::Receiver<WorkerEvent>,
}

impl RustTubeApp {
    pub fn new(
        cc: &eframe::CreationContext,
        db: Database,
        auth_manager: Arc<GoogleAuthManager>,
        api_client: Arc<YouTubeApiClient>,
        ytdlp: Arc<YtDlpWrapper>,
        player: Arc<tokio::sync::Mutex<PlayerController>>,
    ) -> Self {
        configure_theme(&cc.egui_ctx);

        let history_repo = HistoryRepository::new(db.clone(), 500);
        let watch_later_repo = WatchLaterRepository::new(db.clone());
        let sync_repo = SyncRepository::new(db.clone());
        let thumbnails = ThumbnailManager::new(cc.egui_ctx.clone());

        let (worker_tx, worker_rx) = mpsc::channel::<WorkerEvent>(256);

        // Initial data loading from local SQLite storage
        let initial_history = history_repo.get_all().unwrap_or_default();
        let initial_watch_later = watch_later_repo.get_all().unwrap_or_default();
        let initial_subs = sync_repo.get_subscriptions().unwrap_or_default();
        let initial_playlists = sync_repo.get_playlists().unwrap_or_default();
        let initial_likes = sync_repo.get_liked_videos().unwrap_or_default();

        let app = Self {
            db,
            history_repo,
            watch_later_repo,
            sync_repo,
            auth_manager,
            api_client,
            ytdlp,
            player,
            thumbnails,

            current_page: Page::Home,
            search_query: String::new(),
            status_message: String::new(),
            is_loading: false,
            is_syncing: false,
            last_sync_text: String::new(),

            feed_videos: Vec::new(),
            history_videos: initial_history,
            watch_later_videos: initial_watch_later,
            subscriptions: initial_subs,
            playlists: initial_playlists,
            liked_videos: initial_likes,
            selected_playlist: None,
            playlist_items: Vec::new(),

            selected_video: None,
            video_description: String::new(),
            playback_status: PlaybackStatus::default(),

            user_info: None,
            is_authenticating: false,

            cookies_browser: "chrome".to_string(),
            hwdec_setting: "auto-safe".to_string(),
            new_playlist_title: String::new(),
            new_playlist_desc: String::new(),

            worker_tx,
            worker_rx,
        };

        app.trigger_feed_fetch();
        app.trigger_session_restore();

        app
    }

    fn trigger_feed_fetch(&self) {
        let tx = self.worker_tx.clone();
        let ytdlp = self.ytdlp.clone();

        tokio::spawn(async move {
            if let Ok(videos) = ytdlp.fetch_trending_feed(24).await {
                let _ = tx.send(WorkerEvent::FeedLoaded(videos)).await;
            }
        });
    }

    fn trigger_session_restore(&self) {
        let tx = self.worker_tx.clone();
        let auth = self.auth_manager.clone();

        tokio::spawn(async move {
            if let Ok(Some((_token, user_info))) = auth.restore_session().await {
                let _ = tx.send(WorkerEvent::AuthSuccess(user_info)).await;
            }
        });
    }

    fn trigger_search(&self, query: String) {
        let tx = self.worker_tx.clone();
        let ytdlp = self.ytdlp.clone();

        tokio::spawn(async move {
            if query.trim().is_empty() {
                if let Ok(videos) = ytdlp.fetch_trending_feed(24).await {
                    let _ = tx.send(WorkerEvent::FeedLoaded(videos)).await;
                }
            } else {
                if let Ok(videos) = ytdlp.search_videos(&query, 24).await {
                    let _ = tx.send(WorkerEvent::FeedLoaded(videos)).await;
                }
            }
        });
    }

    fn trigger_google_login(&self) {
        let tx = self.worker_tx.clone();
        let auth = self.auth_manager.clone();

        tokio::spawn(async move {
            let _ = tx
                .send(WorkerEvent::StatusMessage(
                    "Opening Google Account selection in default browser...".to_string(),
                ))
                .await;

            match auth.start_auth_flow().await {
                Ok((_token, user_info)) => {
                    let _ = tx.send(WorkerEvent::AuthSuccess(user_info)).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(WorkerEvent::StatusMessage(format!("Sign in error: {}", e)))
                        .await;
                }
            }
        });
    }

    fn trigger_google_logout(&self) {
        let tx = self.worker_tx.clone();
        let auth = self.auth_manager.clone();

        tokio::spawn(async move {
            auth.logout().await;
            let _ = tx.send(WorkerEvent::AuthLoggedOut).await;
        });
    }

    fn trigger_play_video(&self, video: VideoItem) {
        let tx = self.worker_tx.clone();
        let ytdlp = self.ytdlp.clone();
        let player = self.player.clone();
        let history_repo = self.history_repo.clone();

        // Save to local Watch History
        let _ = history_repo.record(&video);
        let updated_history = history_repo.get_all().unwrap_or_default();
        let _ = self.worker_tx.try_send(WorkerEvent::HistoryUpdated(updated_history));

        let vid = video.id.clone();
        let vtitle = video.title.clone();

        tokio::spawn(async move {
            let _ = tx
                .send(WorkerEvent::StatusMessage(format!(
                    "Extracting stream for {}...",
                    vid
                )))
                .await;

            // Fetch video detail description
            if let Ok(detail) = ytdlp.get_video_detail(&vid).await {
                let _ = tx
                    .send(WorkerEvent::VideoDetailLoaded {
                        description: detail.description,
                    })
                    .await;
            }

            // Extract stream info and play in libmpv
            match ytdlp.get_stream_info(&vid, "1080p").await {
                Ok(stream) => {
                    let p = player.lock().await;
                    if let Err(e) = p.play_stream(&stream, &vid, &vtitle).await {
                        let _ = tx
                            .send(WorkerEvent::StatusMessage(format!("Playback start error: {}", e)))
                            .await;
                    } else {
                        let _ = tx.send(WorkerEvent::StatusMessage(String::new())).await;
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(WorkerEvent::StatusMessage(format!("Stream error: {}", e)))
                        .await;
                }
            }
        });
    }

    fn trigger_sync(&self) {
        let tx = self.worker_tx.clone();
        let auth = self.auth_manager.clone();
        let api = self.api_client.clone();
        let sync_repo = self.sync_repo.clone();

        tokio::spawn(async move {
            if let Ok(Some(token)) = auth.get_access_token().await {
                let _ = tx
                    .send(WorkerEvent::StatusMessage("Syncing subscriptions...".to_string()))
                    .await;

                if let Ok((subs, _)) = api.get_subscriptions(&token, None).await {
                    let _ = sync_repo.save_subscriptions(&subs);
                    let _ = tx.send(WorkerEvent::SubscriptionsLoaded(subs)).await;
                }

                let _ = tx
                    .send(WorkerEvent::StatusMessage("Syncing playlists...".to_string()))
                    .await;

                if let Ok((playlists, _)) = api.get_playlists(&token, None).await {
                    let _ = sync_repo.save_playlists(&playlists);
                    let _ = tx.send(WorkerEvent::PlaylistsLoaded(playlists)).await;
                }

                let _ = tx
                    .send(WorkerEvent::StatusMessage("Syncing liked videos...".to_string()))
                    .await;

                if let Ok(likes) = api.get_liked_videos(&token).await {
                    let _ = sync_repo.save_liked_videos(&likes);
                    let _ = tx.send(WorkerEvent::LikedVideosLoaded(likes)).await;
                }

                let _ = sync_repo.set_last_synced("account_sync");
                let _ = tx.send(WorkerEvent::SyncFinished).await;
            } else {
                let _ = tx
                    .send(WorkerEvent::StatusMessage(
                        "Please sign in with Google to sync account data.".to_string(),
                    ))
                    .await;
            }
        });
    }

    fn process_worker_events(&mut self) {
        while let Ok(event) = self.worker_rx.try_recv() {
            match event {
                WorkerEvent::FeedLoaded(videos) => {
                    self.feed_videos = videos;
                    self.is_loading = false;
                }
                WorkerEvent::HistoryUpdated(items) => {
                    self.history_videos = items;
                }
                WorkerEvent::WatchLaterUpdated(items) => {
                    self.watch_later_videos = items;
                }
                WorkerEvent::SubscriptionsLoaded(subs) => {
                    self.subscriptions = subs;
                }
                WorkerEvent::PlaylistsLoaded(pls) => {
                    self.playlists = pls;
                }
                WorkerEvent::PlaylistItemsLoaded(items) => {
                    self.playlist_items = items;
                }
                WorkerEvent::LikedVideosLoaded(likes) => {
                    self.liked_videos = likes;
                }
                WorkerEvent::AuthSuccess(user) => {
                    self.user_info = Some(user);
                    self.is_authenticating = false;
                    self.status_message = "Signed in successfully.".to_string();
                    self.trigger_sync();
                }
                WorkerEvent::AuthLoggedOut => {
                    self.user_info = None;
                    self.is_authenticating = false;
                    self.status_message = "Signed out and cleared tokens.".to_string();
                }
                WorkerEvent::SyncFinished => {
                    self.is_syncing = false;
                    self.last_sync_text = "Just now".to_string();
                    self.status_message = "Account synchronized successfully.".to_string();
                }
                WorkerEvent::PlaybackStatusUpdated(status) => {
                    self.playback_status = status;
                }
                WorkerEvent::VideoDetailLoaded { description } => {
                    self.video_description = description;
                }
                WorkerEvent::StatusMessage(msg) => {
                    self.status_message = msg;
                }
            }
        }
    }
}

impl App for RustTubeApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.process_worker_events();

        // Top Header
        TopBottomPanel::top("top_header_panel")
            .exact_height(52.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                let mut on_search = false;
                let mut on_login_click = false;
                let mut on_sync_click = false;

                render_header(
                    ui,
                    &mut self.search_query,
                    &mut on_search,
                    self.user_info.as_ref(),
                    self.is_authenticating,
                    &mut on_login_click,
                    self.is_syncing,
                    &self.last_sync_text,
                    &mut on_sync_click,
                );

                if on_search {
                    self.is_loading = true;
                    self.current_page = Page::Home;
                    self.trigger_search(self.search_query.clone());
                }

                if on_login_click {
                    self.is_authenticating = true;
                    self.trigger_google_login();
                }

                if on_sync_click {
                    self.is_syncing = true;
                    self.trigger_sync();
                }
            });

        // Left Navigation Sidebar
        SidePanel::left("left_sidebar_panel")
            .exact_width(210.0)
            .show(ctx, |ui| {
                let watch_later_count = self.watch_later_videos.len();
                let history_count = self.history_videos.len();

                render_sidebar(
                    ui,
                    &mut self.current_page,
                    watch_later_count,
                    history_count,
                );
            });

        // Central Main Workspace
        CentralPanel::default().show(ctx, |ui| {
            if !self.status_message.is_empty() {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(&self.status_message)
                            .size(11.5)
                            .color(THEME.crimson),
                    );
                });
                ui.add_space(8.0);
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                let watch_later_ids: Vec<String> =
                    self.watch_later_videos.iter().map(|v| v.id.clone()).collect();

                let mut on_select_video: Option<VideoItem> = None;
                let mut on_toggle_watch_later: Option<VideoItem> = None;

                match self.current_page {
                    Page::Home => {
                        render_video_grid(
                            ui,
                            &self.feed_videos,
                            &self.thumbnails,
                            &watch_later_ids,
                            &mut on_select_video,
                            &mut on_toggle_watch_later,
                        );
                    }
                    Page::Watch => {
                        if let Some(ref selected) = self.selected_video {
                            let mut on_back = false;
                            let mut on_toggle_play = false;
                            let mut on_seek: Option<f64> = None;
                            let mut on_volume_change: Option<u32> = None;
                            let mut on_speed_change: Option<f32> = None;
                            let mut on_quality_change: Option<String> = None;
                            let mut on_toggle_wl = false;
                            let mut on_toggle_like = false;
                            let mut on_select_related: Option<VideoItem> = None;

                            let in_wl = watch_later_ids.contains(&selected.id);
                            let is_liked = self.liked_videos.iter().any(|v| v.id == selected.id);

                            render_watch_view(
                                ui,
                                selected,
                                &self.video_description,
                                &self.playback_status,
                                &self.feed_videos,
                                &self.thumbnails,
                                in_wl,
                                is_liked,
                                &mut on_back,
                                &mut on_toggle_play,
                                &mut on_seek,
                                &mut on_volume_change,
                                &mut on_speed_change,
                                &mut on_quality_change,
                                &mut on_toggle_wl,
                                &mut on_toggle_like,
                                &mut on_select_related,
                            );

                            if on_back {
                                self.current_page = Page::Home;
                            }

                            if on_toggle_play {
                                let player = self.player.clone();
                                tokio::spawn(async move {
                                    let p = player.lock().await;
                                    let _ = p.toggle_play_pause().await;
                                });
                            }

                            if let Some(pos) = on_seek {
                                let player = self.player.clone();
                                tokio::spawn(async move {
                                    let p = player.lock().await;
                                    let _ = p.seek_absolute(pos).await;
                                });
                            }

                            if let Some(vol) = on_volume_change {
                                let player = self.player.clone();
                                tokio::spawn(async move {
                                    let p = player.lock().await;
                                    let _ = p.set_volume(vol).await;
                                });
                            }

                            if let Some(speed) = on_speed_change {
                                let player = self.player.clone();
                                tokio::spawn(async move {
                                    let p = player.lock().await;
                                    let _ = p.set_speed(speed).await;
                                });
                            }

                            if on_toggle_wl {
                                let _ = self.watch_later_repo.toggle(selected);
                                self.watch_later_videos =
                                    self.watch_later_repo.get_all().unwrap_or_default();
                            }

                            if let Some(rel) = on_select_related {
                                on_select_video = Some(rel);
                            }
                        } else {
                            self.current_page = Page::Home;
                        }
                    }
                    Page::Subscriptions => {
                        let mut on_login_click = false;
                        render_subscriptions_view(
                            ui,
                            &self.subscriptions,
                            &self.thumbnails,
                            self.user_info.is_some(),
                            &mut on_login_click,
                        );
                        if on_login_click {
                            self.is_authenticating = true;
                            self.trigger_google_login();
                        }
                    }
                    Page::Playlists => {
                        let mut on_login_click = false;
                        let mut on_select_playlist = None;
                        let mut on_create_playlist = false;

                        render_playlists_view(
                            ui,
                            &self.playlists,
                            &self.selected_playlist,
                            &self.playlist_items,
                            &self.thumbnails,
                            self.user_info.is_some(),
                            &mut on_login_click,
                            &mut on_select_playlist,
                            &mut on_select_video,
                            &mut self.new_playlist_title,
                            &mut self.new_playlist_desc,
                            &mut on_create_playlist,
                        );

                        if on_login_click {
                            self.is_authenticating = true;
                            self.trigger_google_login();
                        }

                        if let Some(pl) = on_select_playlist {
                            self.selected_playlist = Some(pl.clone());
                            self.playlist_items =
                                self.sync_repo.get_playlist_items(&pl.id).unwrap_or_default();
                        }
                    }
                    Page::LikedVideos => {
                        render_video_grid(
                            ui,
                            &self.liked_videos,
                            &self.thumbnails,
                            &watch_later_ids,
                            &mut on_select_video,
                            &mut on_toggle_watch_later,
                        );
                    }
                    Page::WatchLater => {
                        render_watch_later_view(
                            ui,
                            &self.watch_later_videos,
                            &self.thumbnails,
                            &watch_later_ids,
                            &mut on_select_video,
                            &mut on_toggle_watch_later,
                        );
                    }
                    Page::History => {
                        let mut on_clear_history = false;
                        render_history_view(
                            ui,
                            &self.history_videos,
                            &self.thumbnails,
                            &watch_later_ids,
                            &mut on_clear_history,
                            &mut on_select_video,
                            &mut on_toggle_watch_later,
                        );

                        if on_clear_history {
                            let _ = self.history_repo.clear();
                            self.history_videos.clear();
                        }
                    }
                    Page::Settings => {
                        let mut on_login_click = false;
                        let mut on_logout_click = false;
                        let mut on_clear_history = false;
                        let mut on_clear_cache = false;

                        render_settings_view(
                            ui,
                            self.user_info.as_ref(),
                            self.is_authenticating,
                            &mut self.cookies_browser,
                            &mut self.hwdec_setting,
                            &mut on_login_click,
                            &mut on_logout_click,
                            &mut on_clear_history,
                            &mut on_clear_cache,
                        );

                        if on_login_click {
                            self.is_authenticating = true;
                            self.trigger_google_login();
                        }

                        if on_logout_click {
                            self.trigger_google_logout();
                        }

                        if on_clear_history {
                            let _ = self.history_repo.clear();
                            self.history_videos.clear();
                        }
                    }
                }

                if let Some(video) = on_select_video {
                    self.selected_video = Some(video.clone());
                    self.current_page = Page::Watch;
                    self.trigger_play_video(video);
                }

                if let Some(video) = on_toggle_watch_later {
                    let _ = self.watch_later_repo.toggle(&video);
                    self.watch_later_videos =
                        self.watch_later_repo.get_all().unwrap_or_default();
                }
            });
        });

        ctx.request_repaint();
    }
}
