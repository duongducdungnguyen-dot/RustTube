use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub app: GeneralConfig,
    #[serde(default)]
    pub player: PlayerConfig,
    #[serde(default)]
    pub ytdlp: YtDlpConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub theme: String,
    pub window_width: u32,
    pub window_height: u32,
    pub startup_page: String,
    pub auto_refresh_subscriptions: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            window_width: 1380,
            window_height: 860,
            startup_page: "home".to_string(),
            auto_refresh_subscriptions: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerConfig {
    pub hwdec: String,
    pub preferred_quality: String,
    pub volume: u32,
    pub speed: f32,
    pub subtitles_enabled: bool,
    pub audio_device: String,
    pub cache_seconds: u32,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            hwdec: "auto-safe".to_string(),
            preferred_quality: "1080p".to_string(),
            volume: 80,
            speed: 1.0,
            subtitles_enabled: false,
            audio_device: "auto".to_string(),
            cache_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtDlpConfig {
    pub binary_path: String,
    pub cookies_browser: String,
    pub auto_update_check: bool,
    pub max_search_results: u32,
}

impl Default for YtDlpConfig {
    fn default() -> Self {
        Self {
            binary_path: "bin/yt-dlp.exe".to_string(),
            cookies_browser: "chrome".to_string(),
            auto_update_check: true,
            max_search_results: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub google_client_id: String,
    pub redirect_port: u16,
    pub keyring_service_name: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            google_client_id: String::new(),
            redirect_port: 8989,
            keyring_service_name: "RustTube_Google_OAuth".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub disk_cache_mb: u64,
    pub thumbnail_dir: String,
    pub persist_history: bool,
    pub persist_watch_later: bool,
    pub max_history_items: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            disk_cache_mb: 512,
            thumbnail_dir: "cache/thumbnails".to_string(),
            persist_history: true,
            persist_watch_later: true,
            max_history_items: 500,
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app: GeneralConfig::default(),
            player: PlayerConfig::default(),
            ytdlp: YtDlpConfig::default(),
            auth: AuthConfig::default(),
            cache: CacheConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        let p = path.as_ref();
        if p.exists() {
            if let Ok(contents) = std::fs::read_to_string(p) {
                if let Ok(config) = toml::from_str::<AppConfig>(&contents) {
                    return config;
                }
            }
        }
        Self::default()
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let p = path.as_ref();
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let serialized = toml::to_string_pretty(self)?;
        std::fs::write(p, serialized)?;
        Ok(())
    }
}
