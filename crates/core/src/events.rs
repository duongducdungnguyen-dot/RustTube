use crate::models::{AppPage, ChannelItem, PlaybackStatus, PlaylistItem, VideoDetail, VideoItem};

#[derive(Debug, Clone)]
pub enum UiCommand {
    Navigate(AppPage),
    Search(String),
    LoadMoreSearch,
    PlayVideo { video_id: String, title: String, channel: String, quality: String },
    TogglePlayPause,
    SeekRelative(f64),
    SeekAbsolute(f64),
    SetVolume(u32),
    SetSpeed(f32),
    SetQuality(String),
    ToggleSubtitles,
    ToggleFullscreen,
    ToggleLike(String),
    ToggleWatchLater(VideoItem),
    RemoveFromWatchLater(String),
    ClearHistory,
    StartGoogleLogin,
    LogoutGoogle,
    RefreshSubscriptions,
    LoadPlaylist(String),
    CheckYtDlpUpdate,
    UpdateYtDlp,
}

#[derive(Debug, Clone)]
pub enum CoreEvent {
    PageChanged(AppPage),
    SearchResultsLoaded {
        query: String,
        items: Vec<VideoItem>,
        has_more: bool,
    },
    SearchResultsAppended(Vec<VideoItem>),
    HomeVideosLoaded(Vec<VideoItem>),
    SubscriptionsLoaded(Vec<ChannelItem>),
    PlaylistsLoaded(Vec<PlaylistItem>),
    PlaylistVideosLoaded { playlist_id: String, items: Vec<VideoItem> },
    WatchLaterLoaded(Vec<VideoItem>),
    HistoryLoaded(Vec<VideoItem>),
    VideoDetailLoaded(VideoDetail),
    PlaybackStateUpdated(PlaybackStatus),
    AuthStatusUpdated {
        is_logged_in: bool,
        user_name: String,
        email: String,
    },
    YtDlpVersionInfo { current: String, latest: Option<String> },
    ErrorMessage(String),
    NotificationMessage(String),
}
