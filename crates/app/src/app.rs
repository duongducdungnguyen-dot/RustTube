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
use tokio::runtime::Handle;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub struct RustTubeApp {
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
    sync_state: SyncState,
    update_state: UpdateState,
    notification: Option<String>,
    is_loading: bool,

    cmd_tx: UnboundedSender<UiCommand>,
    evt_rx: UnboundedReceiver<AppEvent>,
}

impl RustTubeApp {
    pub fn new(config: AppConfig, rt_handle: Handle) -> Self {
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

        let worker_cfg = config.clone();
        rt_handle.spawn(async move {
            let api = YouTubeApiClient::new();
            let ytdlp = YtDlpRunner::new(
                worker_cfg.ytdlp.binary_path.clone(),
                worker_cfg.ytdlp.cookies_browser.clone(),
            );
            let auth = OAuthSessionManager::new(
                worker_cfg.auth.google_client_id.clone(),
                worker_cfg.auth.keyring_service_name.clone(),
            );

            // Automatic session restoration at startup
            if worker_cfg.auth.auto_restore_session {
                let _ = auth.restore_or_refresh_session().await;
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
