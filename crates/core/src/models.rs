use serde::{Deserialize, Serialize};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedCategory {
    Personalized,
    Subscriptions,
    RelatedToLiked,
    ContinueWatching,
    Trending,
    ExperimentalCookieHome,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoItem {
    pub id: String,
    pub title: String,
    pub channel_name: String,
    pub channel_id: String,
    pub duration_seconds: u64,
    pub duration_string: String,
    pub view_count: u64,
    pub views_text: String,
    pub published_at: String,
    pub published_timestamp: i64,
    pub thumbnail_url: String,
    pub description: String,
    pub resume_position_seconds: Option<u64>,
    pub source_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChannelItem {
    pub channel_id: String,
    pub subscription_id: Option<String>,
    pub title: String,
    pub description: String,
    pub custom_url: Option<String>,
    pub thumbnail_url: String,
    pub subscriber_count: Option<String>,
    pub video_count: Option<u64>,
    pub uploads_playlist_id: Option<String>,
    pub synced_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaylistPrivacy {
    Public,
    Unlisted,
    Private,
}

impl PlaylistPrivacy {
    pub fn as_api_str(&self) -> &'static str {
        match self {
            PlaylistPrivacy::Public => "public",
            PlaylistPrivacy::Unlisted => "unlisted",
            PlaylistPrivacy::Private => "private",
        }
    }

    pub fn from_api_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "public" => PlaylistPrivacy::Public,
            "unlisted" => PlaylistPrivacy::Unlisted,
            _ => PlaylistPrivacy::Private,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaylistItem {
    pub playlist_id: String,
    pub title: String,
    pub description: String,
    pub item_count: u32,
    pub thumbnail_url: String,
    pub privacy_status: PlaylistPrivacy,
    pub is_owned: bool,
    pub synced_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaylistVideoItem {
    pub playlist_item_id: String,
    pub playlist_id: String,
    pub video: VideoItem,
    pub position: u32,
    pub availability_status: VideoAvailability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoAvailability {
    Available,
    Private,
    Deleted,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WatchLaterSource {
    LocalRustTube,
    ExperimentalYouTubeCookie,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchLaterEntry {
    pub video: VideoItem,
    pub added_at: i64,
    pub source: WatchLaterSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchHistoryEntry {
    pub video: VideoItem,
    pub watched_at: i64,
    pub last_position_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub channel_id: String,
    pub display_name: String,
    pub email: String,
    pub handle: String,
    pub avatar_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncErrorKind {
    Authentication,
    AuthorizationScope,
    Network,
    QuotaExceeded,
    Parsing,
    Database,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedError {
    pub kind: SyncErrorKind,
    pub message: String,
    pub occurred_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub is_syncing: bool,
    pub last_successful_sync: Option<i64>,
    pub subscriptions_count: usize,
    pub playlists_count: usize,
    pub liked_videos_count: usize,
    pub last_error: Option<ClassifiedError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseManifest {
    pub version: String,
    pub tag_name: String,
    pub published_at: String,
    pub release_notes: String,
    pub html_url: String,
    pub asset_name: String,
    pub download_url: String,
    pub sha256: Option<String>,
    pub size_bytes: u64,
    pub is_prerelease: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate {
        current_version: String,
        checked_at: i64,
    },
    UpdateAvailable {
        current_version: String,
        manifest: ReleaseManifest,
        checked_at: i64,
    },
    Downloading {
        version: String,
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    StagedReadyToRestart {
        version: String,
        staged_path: String,
        sha256_verified: bool,
    },
    Failed {
        message: String,
        rolled_back: bool,
    },
}
