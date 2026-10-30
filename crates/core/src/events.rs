use crate::models::*;

#[derive(Debug, Clone)]
pub enum UiCommand {
    // Feed & Search
    LoadHomeFeed {
        category: FeedCategory,
        force_refresh: bool,
    },
    Search {
        query: String,
    },
    // Subscriptions & Channel
    LoadSubscriptions {
        force_refresh: bool,
    },
    OpenChannel {
        channel_id: String,
        page_token: Option<String>,
    },
    UnsubscribeChannel {
        channel_id: String,
        subscription_id: String,
    },
    // Playlists
    LoadPlaylists {
        force_refresh: bool,
    },
    OpenPlaylist {
        playlist_id: String,
        page_token: Option<String>,
    },
    CreatePlaylist {
        title: String,
        description: String,
        privacy: PlaylistPrivacy,
    },
    UpdatePlaylist {
        playlist_id: String,
        title: String,
        description: String,
        privacy: PlaylistPrivacy,
    },
    DeletePlaylist {
        playlist_id: String,
    },
    AddVideoToPlaylist {
        playlist_id: String,
        video: VideoItem,
    },
    RemoveVideoFromPlaylist {
        playlist_id: String,
        playlist_item_id: String,
    },
    // Liked Videos
    LoadLikedVideos {
        page_token: Option<String>,
        force_refresh: bool,
    },
    RateVideo {
        video: VideoItem,
        rating: VideoRating,
    },
    // Watch Later (Local vs Experimental Cookie)
    LoadWatchLater,
    ToggleLocalWatchLater {
        video: VideoItem,
    },
    RemoveWatchLater {
        video_id: String,
    },
    ImportExperimentalYouTubeWatchLater,
    // Watch History
    LoadHistory {
        search_query: Option<String>,
    },
    RecordHistory {
        video: VideoItem,
        position_seconds: u64,
    },
    RemoveHistoryEntry {
        video_id: String,
    },
    ClearHistory,
    // Playback
    PlayVideo {
        video: VideoItem,
        playlist_queue: Option<Vec<VideoItem>>,
        resume_from_seconds: Option<u64>,
    },
    SetQuality(String),
    SetSpeed(f64),
    SetVolume(u8),
    TogglePause,
    SeekRelative(i64),
    // Auth & Sync
    StartGoogleLogin,
    Logout,
    TriggerAccountSync,
    SaveSettings(crate::config::AppConfig),
    // Application Updater
    CheckForUpdates {
        manual_trigger: bool,
    },
    DownloadAndStageUpdate {
        manifest: ReleaseManifest,
    },
    ApplyStagedUpdateAndRestart {
        staged_zip_path: String,
        target_version: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoRating {
    Like,
    None,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    HomeFeedLoaded {
        category: FeedCategory,
        videos: Vec<VideoItem>,
        next_page_token: Option<String>,
    },
    SearchResultsLoaded {
        query: String,
        videos: Vec<VideoItem>,
    },
    SubscriptionsLoaded {
        channels: Vec<ChannelItem>,
    },
    ChannelUnsubscribed {
        channel_id: String,
    },
    ChannelDetailsLoaded {
        channel: ChannelItem,
        videos: Vec<VideoItem>,
        next_page_token: Option<String>,
    },
    PlaylistsLoaded {
        playlists: Vec<PlaylistItem>,
    },
    PlaylistDetailsLoaded {
        playlist: PlaylistItem,
        items: Vec<PlaylistVideoItem>,
        next_page_token: Option<String>,
    },
    PlaylistMutated {
        message: String,
        playlists: Vec<PlaylistItem>,
    },
    LikedVideosLoaded {
        videos: Vec<VideoItem>,
        next_page_token: Option<String>,
    },
    VideoRated {
        video_id: String,
        liked: bool,
    },
    WatchLaterLoaded {
        local_entries: Vec<WatchLaterEntry>,
        experimental_entries: Vec<WatchLaterEntry>,
    },
    HistoryLoaded {
        entries: Vec<WatchHistoryEntry>,
    },
    StreamResolved {
        video: VideoItem,
        video_url: String,
        audio_url: Option<String>,
        available_qualities: Vec<String>,
    },
    AuthUpdated {
        profile: Option<UserProfile>,
    },
    SyncStateUpdated(SyncState),
    UpdateStateChanged(UpdateState),
    Notification(String),
    Error(ClassifiedError),
}
