use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VideoItem {
    pub id: String,
    pub title: String,
    pub channel_name: String,
    pub channel_id: String,
    pub duration_seconds: u32,
    pub duration_string: String,
    pub view_count: u64,
    pub thumbnail_url: String,
    pub published_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDetail {
    pub item: VideoItem,
    pub description: String,
    pub available_qualities: Vec<String>,
    pub selected_quality: String,
    pub is_liked: bool,
    pub is_watch_later: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub subscriber_count: u64,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub video_count: u32,
    pub thumbnail_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackStatus {
    pub video_id: String,
    pub title: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub is_playing: bool,
    pub is_buffering: bool,
    pub volume: u32,
    pub speed: f32,
    pub quality: String,
    pub subtitles: bool,
    pub is_fullscreen: bool,
}

impl Default for PlaybackStatus {
    fn default() -> Self {
        Self {
            video_id: String::new(),
            title: String::new(),
            position_seconds: 0.0,
            duration_seconds: 0.0,
            is_playing: false,
            is_buffering: false,
            volume: 80,
            speed: 1.0,
            quality: "1080p".to_string(),
            subtitles: false,
            is_fullscreen: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamInfo {
    pub video_url: String,
    pub audio_url: Option<String>,
    pub format_id: String,
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum YouTubeUrlType {
    Video(String),
    Shorts(String),
    Playlist(String),
    Channel(String),
    Search(String),
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AppPage {
    Home,
    Search,
    Subscriptions,
    Playlists,
    PlaylistDetail(String),
    WatchLater,
    History,
    Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthState {
    Unauthenticated,
    Authenticating,
    Authenticated { user_name: String, email: String, avatar_url: String },
    Error(String),
}
