pub mod channel_view;
pub mod header;
pub mod history_view;
pub mod liked_view;
pub mod playlists_view;
pub mod settings_view;
pub mod sidebar;
pub mod subscriptions_view;
pub mod video_grid;
pub mod watch_later_view;
pub mod watch_view;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePage {
    Home,
    Subscriptions,
    Channel,
    Playlists,
    PlaylistDetail,
    LikedVideos,
    WatchLater,
    History,
    Watch,
    Search,
    Settings,
}
