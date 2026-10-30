use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub auth: AuthConfig,
    pub sync: SyncConfig,
    pub playback: PlaybackConfig,
    pub ytdlp: YtDlpConfig,
    pub storage: StorageConfig,
    pub updater: UpdaterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub google_client_id: String,
    pub redirect_port: u16,
    pub keyring_service_name: String,
    pub auto_restore_session: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub auto_sync_enabled: bool,
    pub sync_interval_minutes: u64,
    pub max_uploads_per_channel: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackConfig {
    pub hwdec: String,
    pub default_quality: String,
    pub default_volume: u8,
    pub default_speed: f64,
    pub resume_playback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtDlpConfig {
    pub binary_path: String,
    pub cookies_browser: String,
    pub experimental_cookie_home_feed: bool,
    pub experimental_cookie_watch_later: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub custom_data_dir: String,
    pub thumbnail_cache_mb: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdaterConfig {
    pub auto_check_updates: bool,
    pub include_prereleases: bool,
    pub github_repo: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            auth: AuthConfig {
                google_client_id: String::new(),
                redirect_port: 8989,
                keyring_service_name: "RustTube_Google_OAuth".to_string(),
                auto_restore_session: true,
            },
            sync: SyncConfig {
                auto_sync_enabled: true,
                sync_interval_minutes: 30,
                max_uploads_per_channel: 8,
            },
            playback: PlaybackConfig {
                hwdec: "auto-safe".to_string(),
                default_quality: "1080p".to_string(),
                default_volume: 80,
                default_speed: 1.0,
                resume_playback: true,
            },
            ytdlp: YtDlpConfig {
                binary_path: if cfg!(windows) {
                    "bin/yt-dlp.exe".to_string()
                } else {
                    "bin/yt-dlp".to_string()
                },
                cookies_browser: "none".to_string(),
                experimental_cookie_home_feed: false,
                experimental_cookie_watch_later: false,
            },
            storage: StorageConfig {
                custom_data_dir: String::new(),
                thumbnail_cache_mb: 256,
            },
            updater: UpdaterConfig {
                auto_check_updates: true,
                include_prereleases: false,
                github_repo: "duongducdungnguyen-dot/RustTube".to_string(),
            },
        }
    }
}

impl AppConfig {
    /// Returns the persistent user configuration file path in %APPDATA%\RustTube\config.toml
    /// separated from the binary installation directory so application updates never touch it.
    pub fn config_file_path() -> PathBuf {
        if let Some(proj) = ProjectDirs::from("com", "RustTube", "RustTube") {
            let dir = proj.config_dir();
            let _ = fs::create_dir_all(dir);
            return dir.join("config.toml");
        }
        PathBuf::from("config.toml")
    }

    /// Returns the persistent user data directory in %LOCALAPPDATA%\RustTube\data
    pub fn user_data_dir(&self) -> PathBuf {
        if !self.storage.custom_data_dir.trim().is_empty() {
            let custom = PathBuf::from(&self.storage.custom_data_dir);
            let _ = fs::create_dir_all(&custom);
            return custom;
        }
        if let Some(proj) = ProjectDirs::from("com", "RustTube", "RustTube") {
            let dir = proj.data_local_dir().join("data");
            let _ = fs::create_dir_all(&dir);
            return dir;
        }
        let fallback = PathBuf::from("cache");
        let _ = fs::create_dir_all(&fallback);
        fallback
    }

    pub fn sqlite_db_path(&self) -> PathBuf {
        self.user_data_dir().join("rusttube.db")
    }

    pub fn load() -> Result<Self> {
        let primary = Self::config_file_path();
        if primary.exists() {
            let raw = fs::read_to_string(&primary)
                .with_context(|| format!("Failed to read {}", primary.display()))?;
            let cfg: AppConfig = toml::from_str(&raw).unwrap_or_default();
            return Ok(cfg);
        }

        let local_fallback = PathBuf::from("config.toml");
        if local_fallback.exists() {
            let raw = fs::read_to_string(&local_fallback)?;
            let cfg: AppConfig = toml::from_str(&raw).unwrap_or_default();
            let _ = cfg.save();
            return Ok(cfg);
        }

        let default_cfg = Self::default();
        let _ = default_cfg.save();
        Ok(default_cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        fs::write(&path, serialized)?;
        Ok(())
    }
}
