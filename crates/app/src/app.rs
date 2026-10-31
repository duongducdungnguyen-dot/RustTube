use crate::ui::{
    channel_view::render_channel_view,
    header::render_header,
    history_view::render_history_view,
    liked_view::render_liked_view,
    playlists_view::{render_playlist_detail_view, render_playlists_overview, PlaylistFormState},
    settings_view::render_settings_view,
    sidebar::render_sidebar,
    subscriptions_view::render_subscriptions_view,
    video_grid::render_video_grid,
    watch_later_view::render_watch_later_view,
    watch_view::render_watch_view,
    ActivePage,
};
use chrono::Utc;
use eframe::egui;
use rusttube_api::YouTubeApiClient;
use rusttube_auth::OAuthSessionManager;
use rusttube_core::*;
use rusttube_storage::{Database, HistoryRepo, SyncRepo, WatchLaterRepo};
use rusttube_updater::UpdateManager;
use rusttube_ytdlp::YtDlpRunner;
use std::future::Future;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// Coordinates in-flight Google OAuth2 PKCE login attempts off the command-dispatch loop so:
/// 1. The command worker remains responsive while waiting up to 120s for the browser callback.
/// 2. Repeated clicks on "Sign In with Google" never bind a second listener on the callback port.
/// 3. Pending attempts can be cancelled immediately (`UiCommand::CancelGoogleLogin`), and
///    `in_progress` is always reset on cancel, timeout, callback error, token/profile error, or success.
#[derive(Default)]
pub struct LoginAttemptCoordinator {
    in_progress: Arc<AtomicBool>,
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl LoginAttemptCoordinator {
    pub fn new() -> Self {
        Self {
            in_progress: Arc::new(AtomicBool::new(false)),
            cancel_tx: None,
        }
    }

    pub fn is_in_progress(&self) -> bool {
        self.in_progress.load(Ordering::SeqCst)
    }

    pub fn cancel_active(&mut self, evt_tx: &UnboundedSender<AppEvent>) -> bool {
        let was_active = self.in_progress.swap(false, Ordering::SeqCst);
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        if was_active {
            let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: false });
            let _ = evt_tx.send(AppEvent::Notification(
                "Google Sign-In cancelled. You can start a new sign-in attempt whenever ready."
                    .to_string(),
            ));
        }
        was_active
    }

    pub fn start_login(
        &mut self,
        auth: &OAuthSessionManager,
        api: &YouTubeApiClient,
        redirect_port: u16,
        evt_tx: UnboundedSender<AppEvent>,
    ) {
        let auth_clone = auth.clone();
        let api_clone = api.clone();
        self.start_login_with_future(auth, evt_tx, move |cancel_rx| async move {
            let tokens = auth_clone
                .start_pkce_login_cancellable(redirect_port, Some(cancel_rx))
                .await?;
            api_clone
                .fetch_authenticated_profile(&tokens.access_token)
                .await
        });
    }

    pub fn start_login_with_future<F, Fut>(
        &mut self,
        auth: &OAuthSessionManager,
        evt_tx: UnboundedSender<AppEvent>,
        make_future: F,
    ) where
        F: FnOnce(tokio::sync::oneshot::Receiver<()>) -> Fut,
        Fut: Future<Output = std::result::Result<UserProfile, ClassifiedError>> + Send + 'static,
    {
        if self.in_progress.load(Ordering::SeqCst) {
            let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: true });
            let _ = evt_tx.send(AppEvent::Notification(
                "Sign-in already in progress — complete authorization in your browser or click Cancel Sign-In."
                    .to_string(),
            ));
            return;
        }

        if let Err(err) = auth.validate_configured_client_id() {
            self.in_progress.store(false, Ordering::SeqCst);
            let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: false });
            let _ = evt_tx.send(AppEvent::Error(err));
            return;
        }

        self.in_progress.store(true, Ordering::SeqCst);
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        self.cancel_tx = Some(cancel_tx);

        let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: true });
        let _ = evt_tx.send(AppEvent::Notification(
            "Opening system browser for Google Sign-In...".to_string(),
        ));

        let in_progress_flag = Arc::clone(&self.in_progress);
        let fut = make_future(cancel_rx);

        tokio::spawn(async move {
            let result = fut.await;
            let still_marked_active = in_progress_flag.swap(false, Ordering::SeqCst);
            let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: false });

            match result {
                Ok(profile) => {
                    let display_name = profile.display_name.clone();
                    let _ = evt_tx.send(AppEvent::AuthUpdated {
                        profile: Some(profile),
                    });
                    let _ = evt_tx.send(AppEvent::Notification(format!(
                        "Signed in with Google as {}",
                        display_name
                    )));
                }
                Err(err) => {
                    // Avoid overwriting the explicit cancellation notice if cancel_active already fired
                    if still_marked_active || !err.message.contains("cancelled") {
                        let _ = evt_tx.send(AppEvent::Error(err));
                    }
                }
            }
        });
    }
}

pub struct RustTubeApp {
    _runtime: Runtime,
    config: AppConfig,
    current_page: ActivePage,
    nav_history: Vec<ActivePage>,
    feed_category: FeedCategory,
    search_query: String,
    subscriptions_filter: String,
    history_search: String,
    confirm_unsub_channel_id: Option<String>,
    playlist_form: PlaylistFormState,

    home_videos: Vec<VideoItem>,
    search_videos: Vec<VideoItem>,
    subscriptions: Vec<ChannelItem>,
    active_channel: Option<ChannelItem>,
    channel_videos: Vec<VideoItem>,
    channel_next_page: Option<String>,
    playlists: Vec<PlaylistItem>,
    active_playlist: Option<PlaylistItem>,
    playlist_items: Vec<PlaylistVideoItem>,
    playlist_next_page: Option<String>,
    liked_videos: Vec<VideoItem>,
    liked_next_page: Option<String>,
    local_watch_later: Vec<WatchLaterEntry>,
    experimental_watch_later: Vec<WatchLaterEntry>,
    watch_history: Vec<WatchHistoryEntry>,
    selected_video: Option<VideoItem>,

    profile: Option<UserProfile>,
    auth_in_progress: bool,
    sync_state: SyncState,
    update_state: UpdateState,
    notification: Option<String>,
    is_loading: bool,

    cmd_tx: UnboundedSender<UiCommand>,
    evt_rx: UnboundedReceiver<AppEvent>,
}

impl RustTubeApp {
    pub fn new(config: AppConfig, runtime: Runtime) -> Self {
        let (cmd_tx, mut cmd_rx) = unbounded_channel::<UiCommand>();
        let (evt_tx, evt_rx) = unbounded_channel::<AppEvent>();

        let db = Database::open(config.sqlite_db_path())
            .unwrap_or_else(|_| Database::open_in_memory().expect("SQLite init"));
        let history_repo = HistoryRepo::new(db.clone());
        let wl_repo = WatchLaterRepo::new(db.clone());
        let sync_repo = SyncRepo::new(db);

        let initial_subs = sync_repo.get_subscriptions(None).unwrap_or_default();
        let initial_playlists = sync_repo.get_playlists().unwrap_or_default();
        let initial_likes = sync_repo.get_liked_videos().unwrap_or_default();
        let initial_wl = wl_repo
            .list_by_source(WatchLaterSource::LocalRustTube)
            .unwrap_or_default();
        let initial_hist = history_repo.list(None, 200).unwrap_or_default();
        let last_sync = sync_repo.get_last_sync_timestamp("account").unwrap_or(None);

        let mut worker_cfg = config.clone();
        runtime.spawn(async move {
            let api = YouTubeApiClient::new();
            let ytdlp = YtDlpRunner::new(
                worker_cfg.ytdlp.binary_path.clone(),
                worker_cfg.ytdlp.cookies_browser.clone(),
            );
            let mut auth = OAuthSessionManager::new(
                worker_cfg.auth.google_client_id.clone(),
                worker_cfg.auth.keyring_service_name.clone(),
            );
            let mut login_coordinator = LoginAttemptCoordinator::new();

            // Automatic session restoration at startup when a valid client_id is configured
            if worker_cfg.auth.auto_restore_session
                && !AuthConfig::is_placeholder_or_empty(&worker_cfg.auth.google_client_id)
            {
                match auth.restore_or_refresh_session().await {
                    Ok(Some(tokens)) => match api
                        .fetch_authenticated_profile(&tokens.access_token)
                        .await
                    {
                        Ok(profile) => {
                            let _ = evt_tx.send(AppEvent::AuthUpdated {
                                profile: Some(profile.clone()),
                            });
                            let _ = evt_tx.send(AppEvent::Notification(format!(
                                "Restored Google session as {}",
                                profile.display_name
                            )));
                        }
                        Err(err) => {
                            let _ = evt_tx.send(AppEvent::Error(err));
                        }
                    },
                    Ok(None) => {}
                    Err(err) => {
                        let _ = evt_tx.send(AppEvent::Error(err));
                    }
                }
            }

            // Automatic startup update check against GitHub Releases
            if worker_cfg.updater.auto_check_updates {
                let updater = UpdateManager::new(
                    worker_cfg.updater.github_repo.clone(),
                    APP_VERSION.to_string(),
                );
                if let Ok(Some(manifest)) = updater
                    .check_for_updates(worker_cfg.updater.include_prereleases)
                    .await
                {
                    let _ = evt_tx.send(AppEvent::UpdateStateChanged(
                        UpdateState::UpdateAvailable {
                            current_version: APP_VERSION.to_string(),
                            manifest,
                            checked_at: Utc::now().timestamp(),
                        },
                    ));
                }
            }

            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    UiCommand::StartGoogleLogin => {
                        login_coordinator.start_login(
                            &auth,
                            &api,
                            worker_cfg.auth.redirect_port,
                            evt_tx.clone(),
                        );
                    }
                    UiCommand::CancelGoogleLogin => {
                        login_coordinator.cancel_active(&evt_tx);
                    }
                    UiCommand::Logout => {
                        login_coordinator.cancel_active(&evt_tx);
                        handle_logout(&auth, &evt_tx);
                    }
                    UiCommand::TriggerAccountSync => {
                        match auth.restore_or_refresh_session().await {
                            Ok(Some(tokens)) => {
                                let _ = evt_tx.send(AppEvent::Notification(
                                    "Synchronizing YouTube subscriptions and playlists...".to_string(),
                                ));
                                match api.fetch_all_subscriptions(&tokens.access_token).await {
                                    Ok(channels) => {
                                        let _ = sync_repo.replace_subscriptions(&channels);
                                        let _ = evt_tx
                                            .send(AppEvent::SubscriptionsLoaded { channels });
                                    }
                                    Err(err) => {
                                        let _ = evt_tx.send(AppEvent::Error(err));
                                        continue;
                                    }
                                }
                                match api.fetch_all_playlists(&tokens.access_token).await {
                                    Ok(playlists) => {
                                        let _ = sync_repo.replace_playlists(&playlists);
                                        let _ =
                                            evt_tx.send(AppEvent::PlaylistsLoaded { playlists });
                                    }
                                    Err(err) => {
                                        let _ = evt_tx.send(AppEvent::Error(err));
                                        continue;
                                    }
                                }
                                let now = Utc::now().timestamp();
                                let _ = sync_repo.set_last_sync_timestamp("account", now);
                                let subs_len = sync_repo
                                    .get_subscriptions(None)
                                    .map(|v| v.len())
                                    .unwrap_or(0);
                                let pls_len =
                                    sync_repo.get_playlists().map(|v| v.len()).unwrap_or(0);
                                let likes_len =
                                    sync_repo.get_liked_videos().map(|v| v.len()).unwrap_or(0);
                                let _ = evt_tx.send(AppEvent::SyncStateUpdated(SyncState {
                                    is_syncing: false,
                                    last_successful_sync: Some(now),
                                    subscriptions_count: subs_len,
                                    playlists_count: pls_len,
                                    liked_videos_count: likes_len,
                                    last_error: None,
                                }));
                                let _ = evt_tx.send(AppEvent::Notification(
                                    "YouTube account synchronization complete.".to_string(),
                                ));
                            }
                            Ok(None) => {
                                let _ = evt_tx.send(AppEvent::Error(ClassifiedError {
                                    kind: SyncErrorKind::Authentication,
                                    message: "Not signed in. Please click 'Sign In with Google' first.".to_string(),
                                    occurred_at: Utc::now().timestamp(),
                                }));
                            }
                            Err(err) => {
                                let _ = evt_tx.send(AppEvent::Error(err));
                            }
                        }
                    }
                    UiCommand::SaveSettings(new_cfg) => {
                        worker_cfg = new_cfg;
                        auth.set_client_id(worker_cfg.auth.google_client_id.clone());
                        match worker_cfg.save() {
                            Ok(()) => {
                                let _ = evt_tx.send(AppEvent::Notification(
                                    "Application settings saved.".to_string(),
                                ));
                            }
                            Err(e) => {
                                let _ = evt_tx.send(AppEvent::Error(ClassifiedError {
                                    kind: SyncErrorKind::Database,
                                    message: format!("Failed to save config.toml: {}", e),
                                    occurred_at: Utc::now().timestamp(),
                                }));
                            }
                        }
                    }
                    UiCommand::Search { query } => {
                        if let Ok(videos) = ytdlp.search(&query, 24).await {
                            let _ = evt_tx.send(AppEvent::SearchResultsLoaded { query, videos });
                        }
                    }
                    UiCommand::ToggleLocalWatchLater { video } => {
                        if let Ok(added) = wl_repo.toggle_local(&video) {
                            let local_entries = wl_repo
                                .list_by_source(WatchLaterSource::LocalRustTube)
                                .unwrap_or_default();
                            let experimental_entries = wl_repo
                                .list_by_source(WatchLaterSource::ExperimentalYouTubeCookie)
                                .unwrap_or_default();
                            let _ = evt_tx.send(AppEvent::WatchLaterLoaded {
                                local_entries,
                                experimental_entries,
                            });
                            let _ = evt_tx.send(AppEvent::Notification(if added {
                                "Saved to RustTube Local Watch Later".to_string()
                            } else {
                                "Removed from RustTube Local Watch Later".to_string()
                            }));
                        }
                    }
                    UiCommand::RemoveWatchLater { video_id } => {
                        let _ = wl_repo.remove(&video_id);
                        let local_entries = wl_repo
                            .list_by_source(WatchLaterSource::LocalRustTube)
                            .unwrap_or_default();
                        let experimental_entries = wl_repo
                            .list_by_source(WatchLaterSource::ExperimentalYouTubeCookie)
                            .unwrap_or_default();
                        let _ = evt_tx.send(AppEvent::WatchLaterLoaded {
                            local_entries,
                            experimental_entries,
                        });
                    }
                    UiCommand::LoadHistory { search_query } => {
                        if let Ok(entries) = history_repo.list(search_query.as_deref(), 250) {
                            let _ = evt_tx.send(AppEvent::HistoryLoaded { entries });
                        }
                    }
                    UiCommand::RemoveHistoryEntry { video_id } => {
                        let _ = history_repo.remove_entry(&video_id);
                        if let Ok(entries) = history_repo.list(None, 250) {
                            let _ = evt_tx.send(AppEvent::HistoryLoaded { entries });
                        }
                    }
                    UiCommand::ClearHistory => {
                        let _ = history_repo.clear_all();
                        let _ = evt_tx.send(AppEvent::HistoryLoaded {
                            entries: Vec::new(),
                        });
                    }
                    UiCommand::PlayVideo {
                        video,
                        resume_from_seconds,
                        ..
                    } => {
                        let _ = history_repo
                            .record_playback(&video, resume_from_seconds.unwrap_or(0));
                        if let Ok(entries) = history_repo.list(None, 250) {
                            let _ = evt_tx.send(AppEvent::HistoryLoaded { entries });
                        }
                    }
                    UiCommand::CheckForUpdates { .. } => {
                        let _ = evt_tx.send(AppEvent::UpdateStateChanged(UpdateState::Checking));
                        let updater = UpdateManager::new(
                            worker_cfg.updater.github_repo.clone(),
                            APP_VERSION.to_string(),
                        );
                        match updater
                            .check_for_updates(worker_cfg.updater.include_prereleases)
                            .await
                        {
                            Ok(Some(manifest)) => {
                                let _ = evt_tx.send(AppEvent::UpdateStateChanged(
                                    UpdateState::UpdateAvailable {
                                        current_version: APP_VERSION.to_string(),
                                        manifest,
                                        checked_at: Utc::now().timestamp(),
                                    },
                                ));
                            }
                            Ok(None) => {
                                let _ = evt_tx.send(AppEvent::UpdateStateChanged(
                                    UpdateState::UpToDate {
                                        current_version: APP_VERSION.to_string(),
                                        checked_at: Utc::now().timestamp(),
                                    },
                                ));
                            }
                            Err(e) => {
                                let _ = evt_tx.send(AppEvent::UpdateStateChanged(
                                    UpdateState::Failed {
                                        message: e.to_string(),
                                        rolled_back: false,
                                    },
                                ));
                            }
                        }
                    }
                    _ => {
                        let _ = &api;
                    }
                }
            }
        });

        Self {
            _runtime: runtime,
            config,
            current_page: ActivePage::Home,
            nav_history: Vec::new(),
            feed_category: FeedCategory::Personalized,
            search_query: String::new(),
            subscriptions_filter: String::new(),
            history_search: String::new(),
            confirm_unsub_channel_id: None,
            playlist_form: PlaylistFormState::default(),
            home_videos: Vec::new(),
            search_videos: Vec::new(),
            subscriptions: initial_subs.clone(),
            active_channel: None,
            channel_videos: Vec::new(),
            channel_next_page: None,
            playlists: initial_playlists.clone(),
            active_playlist: None,
            playlist_items: Vec::new(),
            playlist_next_page: None,
            liked_videos: initial_likes.clone(),
            liked_next_page: None,
            local_watch_later: initial_wl,
            experimental_watch_later: Vec::new(),
            watch_history: initial_hist,
            selected_video: None,
            profile: None,
            auth_in_progress: false,
            sync_state: SyncState {
                is_syncing: false,
                last_successful_sync: last_sync,
                subscriptions_count: initial_subs.len(),
                playlists_count: initial_playlists.len(),
                liked_videos_count: initial_likes.len(),
                last_error: None,
            },
            update_state: UpdateState::Idle,
            notification: None,
            is_loading: false,
            cmd_tx,
            evt_rx,
        }
    }
}

pub async fn handle_start_google_login(
    auth: &OAuthSessionManager,
    api: &YouTubeApiClient,
    redirect_port: u16,
    evt_tx: &UnboundedSender<AppEvent>,
) {
    if let Err(err) = auth.validate_configured_client_id() {
        let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: false });
        let _ = evt_tx.send(AppEvent::Error(err));
        return;
    }

    let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: true });
    let _ = evt_tx.send(AppEvent::Notification(
        "Opening system browser for Google Sign-In...".to_string(),
    ));

    match auth.start_pkce_login(redirect_port).await {
        Ok(tokens) => match api.fetch_authenticated_profile(&tokens.access_token).await {
            Ok(profile) => {
                let display_name = profile.display_name.clone();
                let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: false });
                let _ = evt_tx.send(AppEvent::AuthUpdated {
                    profile: Some(profile),
                });
                let _ = evt_tx.send(AppEvent::Notification(format!(
                    "Signed in with Google as {}",
                    display_name
                )));
            }
            Err(err) => {
                let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: false });
                let _ = evt_tx.send(AppEvent::Error(err));
            }
        },
        Err(err) => {
            let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: false });
            let _ = evt_tx.send(AppEvent::Error(err));
        }
    }
}

pub fn handle_logout(auth: &OAuthSessionManager, evt_tx: &UnboundedSender<AppEvent>) {
    let _ = evt_tx.send(AppEvent::AuthLoginProgress { in_progress: false });
    match auth.logout() {
        Ok(()) => {
            let _ = evt_tx.send(AppEvent::AuthUpdated { profile: None });
            let _ = evt_tx.send(AppEvent::Notification(
                "Signed out of Google account and cleared stored credentials.".to_string(),
            ));
        }
        Err(e) => {
            let _ = evt_tx.send(AppEvent::Error(ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: format!("Failed to clear credentials on logout: {}", e),
                occurred_at: Utc::now().timestamp(),
            }));
        }
    }
}

impl eframe::App for RustTubeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(evt) = self.evt_rx.try_recv() {
            match evt {
                AppEvent::HomeFeedLoaded { videos, .. } => {
                    self.home_videos = videos;
                    self.is_loading = false;
                }
                AppEvent::SearchResultsLoaded { videos, .. } => {
                    self.search_videos = videos;
                    self.is_loading = false;
                }
                AppEvent::SubscriptionsLoaded { channels } => {
                    self.subscriptions = channels;
                    self.is_loading = false;
                }
                AppEvent::ChannelUnsubscribed { channel_id } => {
                    self.subscriptions.retain(|c| c.channel_id != channel_id);
                }
                AppEvent::ChannelDetailsLoaded {
                    channel,
                    videos,
                    next_page_token,
                } => {
                    self.active_channel = Some(channel);
                    self.channel_videos = videos;
                    self.channel_next_page = next_page_token;
                }
                AppEvent::PlaylistsLoaded { playlists } => {
                    self.playlists = playlists;
                }
                AppEvent::PlaylistDetailsLoaded {
                    playlist,
                    items,
                    next_page_token,
                } => {
                    self.active_playlist = Some(playlist);
                    self.playlist_items = items;
                    self.playlist_next_page = next_page_token;
                }
                AppEvent::PlaylistMutated { message, playlists } => {
                    self.playlists = playlists;
                    self.notification = Some(message);
                }
                AppEvent::LikedVideosLoaded {
                    videos,
                    next_page_token,
                } => {
                    self.liked_videos = videos;
                    self.liked_next_page = next_page_token;
                }
                AppEvent::WatchLaterLoaded {
                    local_entries,
                    experimental_entries,
                } => {
                    self.local_watch_later = local_entries;
                    self.experimental_watch_later = experimental_entries;
                }
                AppEvent::HistoryLoaded { entries } => {
                    self.watch_history = entries;
                }
                AppEvent::AuthUpdated { profile } => {
                    self.profile = profile;
                    self.auth_in_progress = false;
                    self.sync_state.last_error = None;
                }
                AppEvent::AuthLoginProgress { in_progress } => {
                    self.auth_in_progress = in_progress;
                }
                AppEvent::SyncStateUpdated(state) => {
                    self.sync_state = state;
                }
                AppEvent::UpdateStateChanged(state) => {
                    self.update_state = state;
                }
                AppEvent::Notification(msg) => {
                    self.notification = Some(msg);
                }
                AppEvent::Error(err) => {
                    if err.kind == SyncErrorKind::Authentication {
                        self.auth_in_progress = false;
                    }
                    self.notification = Some(format!("Error ({:?}): {}", err.kind, err.message));
                    self.sync_state.last_error = Some(err);
                    self.is_loading = false;
                }
                _ => {}
            }
        }

        let prev_page = self.current_page;
        let mut go_back = false;

        egui::TopBottomPanel::top("rusttube_header").show(ctx, |ui| {
            render_header(
                ui,
                &mut self.search_query,
                &self.profile,
                self.auth_in_progress,
                &self.sync_state,
                &self.update_state,
                !self.nav_history.is_empty(),
                &mut go_back,
                &mut self.current_page,
                &self.cmd_tx,
            );
        });

        egui::SidePanel::left("rusttube_sidebar")
            .resizable(false)
            .default_width(210.0)
            .show(ctx, |ui| {
                render_sidebar(
                    ui,
                    &mut self.current_page,
                    self.local_watch_later.len(),
                    self.watch_history.len(),
                    self.liked_videos.len(),
                    &self.cmd_tx,
                );
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref msg) = self.notification {
                ui.label(msg);
                ui.separator();
            }

            match self.current_page {
                ActivePage::Home => {
                    ui.horizontal(|ui| {
                        for (cat, label) in [
                            (FeedCategory::Personalized, "Personalized Feed"),
                            (FeedCategory::Subscriptions, "Latest Subscriptions"),
                            (FeedCategory::RelatedToLiked, "Based on Liked"),
                            (FeedCategory::ContinueWatching, "Continue Watching"),
                            (FeedCategory::Trending, "Popular / Trending"),
                        ] {
                            if ui
                                .selectable_label(self.feed_category == cat, label)
                                .clicked()
                            {
                                self.feed_category = cat;
                                let _ = self.cmd_tx.send(UiCommand::LoadHomeFeed {
                                    category: cat,
                                    force_refresh: false,
                                });
                            }
                        }
                    });
                    ui.separator();
                    render_video_grid(
                        ui,
                        &self.home_videos,
                        &self.playlists,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::Search => {
                    ui.heading(format!("Search Results for \"{}\"", self.search_query));
                    ui.separator();
                    render_video_grid(
                        ui,
                        &self.search_videos,
                        &self.playlists,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::Subscriptions => {
                    render_subscriptions_view(
                        ui,
                        &self.subscriptions,
                        &mut self.subscriptions_filter,
                        &mut self.confirm_unsub_channel_id,
                        self.is_loading,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::Channel => {
                    render_channel_view(
                        ui,
                        self.active_channel.as_ref(),
                        &self.channel_videos,
                        &self.channel_next_page,
                        &self.playlists,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::Playlists => {
                    render_playlists_overview(
                        ui,
                        &self.playlists,
                        &mut self.playlist_form,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::PlaylistDetail => {
                    render_playlist_detail_view(
                        ui,
                        self.active_playlist.as_ref(),
                        &self.playlist_items,
                        &self.playlist_next_page,
                        &mut self.playlist_form,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::LikedVideos => {
                    render_liked_view(
                        ui,
                        &self.liked_videos,
                        &self.liked_next_page,
                        &self.playlists,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::WatchLater => {
                    render_watch_later_view(
                        ui,
                        &self.local_watch_later,
                        &self.experimental_watch_later,
                        self.config.ytdlp.experimental_cookie_watch_later,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::History => {
                    render_history_view(
                        ui,
                        &self.watch_history,
                        &mut self.history_search,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::Watch => {
                    let is_liked = self
                        .selected_video
                        .as_ref()
                        .map(|v| self.liked_videos.iter().any(|l| l.id == v.id))
                        .unwrap_or(false);
                    let in_wl = self
                        .selected_video
                        .as_ref()
                        .map(|v| self.local_watch_later.iter().any(|w| w.video.id == v.id))
                        .unwrap_or(false);
                    render_watch_view(
                        ui,
                        self.selected_video.as_ref(),
                        is_liked,
                        in_wl,
                        &self.playlists,
                        &mut self.current_page,
                        &self.cmd_tx,
                    );
                }
                ActivePage::Settings => {
                    render_settings_view(
                        ui,
                        &mut self.config,
                        &self.profile,
                        self.auth_in_progress,
                        &self.sync_state,
                        &self.update_state,
                        &self.cmd_tx,
                    );
                }
            }
        });

        if go_back {
            if let Some(prev) = self.nav_history.pop() {
                self.current_page = prev;
            }
        } else if self.current_page != prev_page {
            self.nav_history.push(prev_page);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    #[tokio::test]
    async fn test_start_google_login_dispatch_emits_error_when_client_id_is_placeholder() {
        let (evt_tx, mut evt_rx) = unbounded_channel::<AppEvent>();
        let auth = OAuthSessionManager::new(
            EXAMPLE_GOOGLE_CLIENT_ID_PLACEHOLDER.to_string(),
            "RustTube_Test_Keyring".to_string(),
        );
        let api = YouTubeApiClient::new();

        handle_start_google_login(&auth, &api, 8989, &evt_tx).await;

        let first = evt_rx.try_recv().expect("must emit progress reset");
        assert!(matches!(
            first,
            AppEvent::AuthLoginProgress { in_progress: false }
        ));

        let evt = evt_rx
            .try_recv()
            .expect("StartGoogleLogin must emit an error event instead of being silently ignored");
        match evt {
            AppEvent::Error(err) => {
                assert_eq!(err.kind, SyncErrorKind::Authentication);
                assert!(err.message.contains("google_client_id"));
            }
            other => panic!("Expected AppEvent::Error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_worker_responsiveness_duplicate_clicks_cancellation_and_retry() {
        let (evt_tx, mut evt_rx) = unbounded_channel::<AppEvent>();
        let auth = OAuthSessionManager::new(
            "1234567890-desktop.apps.googleusercontent.com".to_string(),
            "RustTube_Test_Keyring_Coordinator".to_string(),
        );
        let mut coordinator = LoginAttemptCoordinator::new();
        let spawn_count = Arc::new(AtomicUsize::new(0));

        // 1. Start first login attempt that waits for cancellation or long timeout
        let count_ref1 = Arc::clone(&spawn_count);
        coordinator.start_login_with_future(&auth, evt_tx.clone(), move |cancel_rx| async move {
            count_ref1.fetch_add(1, Ordering::SeqCst);
            let _ = cancel_rx.await;
            Err(ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: "Google Sign-In was cancelled.".to_string(),
                occurred_at: Utc::now().timestamp(),
            })
        });

        assert!(coordinator.is_in_progress());
        assert_eq!(spawn_count.load(Ordering::SeqCst), 1);

        // 2. Duplicate click while active must NOT start a second listener/flow
        let count_ref2 = Arc::clone(&spawn_count);
        coordinator.start_login_with_future(&auth, evt_tx.clone(), move |_cancel_rx| async move {
            count_ref2.fetch_add(1, Ordering::SeqCst);
            unreachable!("Duplicate login click must not spawn a second OAuth attempt");
        });
        assert_eq!(spawn_count.load(Ordering::SeqCst), 1);
        assert!(coordinator.is_in_progress());

        // Drain events and confirm duplicate-click message was emitted while worker stays non-blocking
        let mut saw_duplicate_notice = false;
        while let Ok(evt) = evt_rx.try_recv() {
            if let AppEvent::Notification(msg) = evt {
                if msg.contains("Sign-in already in progress") {
                    saw_duplicate_notice = true;
                }
            }
        }
        assert!(saw_duplicate_notice);

        // 3. Cancel active attempt and verify state resets immediately
        assert!(coordinator.cancel_active(&evt_tx));
        assert!(!coordinator.is_in_progress());
        tokio::time::sleep(Duration::from_millis(15)).await;
        while evt_rx.try_recv().is_ok() {}

        // 4. Retry after failure/timeout resets state and allows a subsequent successful login
        coordinator.start_login_with_future(&auth, evt_tx.clone(), move |_cancel_rx| async move {
            Err(ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: "Google Sign-In timed out after 120 seconds".to_string(),
                occurred_at: Utc::now().timestamp(),
            })
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !coordinator.is_in_progress(),
            "in_progress must reset after timeout/error"
        );
        while evt_rx.try_recv().is_ok() {}

        // 5. Retry again and succeed
        coordinator.start_login_with_future(&auth, evt_tx.clone(), move |_cancel_rx| async move {
            Ok(UserProfile {
                channel_id: "UC_TEST".to_string(),
                display_name: "RustTube User".to_string(),
                email: "@rusttube".to_string(),
                handle: "@rusttube".to_string(),
                avatar_url: String::new(),
            })
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!coordinator.is_in_progress());

        let mut saw_auth_updated = false;
        while let Ok(evt) = evt_rx.try_recv() {
            if let AppEvent::AuthUpdated {
                profile: Some(prof),
            } = evt
            {
                assert_eq!(prof.display_name, "RustTube User");
                saw_auth_updated = true;
            }
        }
        assert!(saw_auth_updated);
    }

    #[test]
    fn test_logout_dispatch_emits_auth_updated_or_error() {
        let (evt_tx, mut evt_rx) = unbounded_channel::<AppEvent>();
        let auth = OAuthSessionManager::new(
            "1234567890-test.apps.googleusercontent.com".to_string(),
            "RustTube_Test_Keyring_Logout".to_string(),
        );

        handle_logout(&auth, &evt_tx);

        let first = evt_rx.try_recv().expect("must emit progress reset");
        assert!(matches!(
            first,
            AppEvent::AuthLoginProgress { in_progress: false }
        ));

        let evt = evt_rx
            .try_recv()
            .expect("Logout must emit an event to update the UI");
        match evt {
            AppEvent::AuthUpdated { profile } => assert!(profile.is_none()),
            AppEvent::Error(err) => assert_eq!(err.kind, SyncErrorKind::Authentication),
            other => panic!("Unexpected event from logout: {:?}", other),
        }
    }
}
