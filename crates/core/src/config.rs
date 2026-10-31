use crate::models::{ClassifiedError, SyncErrorKind};
use anyhow::{Context, Result};
use chrono::Utc;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;

pub const EXAMPLE_GOOGLE_CLIENT_ID_PLACEHOLDER: &str =
    "YOUR_GOOGLE_OAUTH_DESKTOP_CLIENT_ID.apps.googleusercontent.com";
pub const EXAMPLE_GOOGLE_CLIENT_SECRET_PLACEHOLDER: &str =
    "YOUR_GOOGLE_OAUTH_DESKTOP_CLIENT_SECRET";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub auth: AuthConfig,
    pub sync: SyncConfig,
    pub playback: PlaybackConfig,
    pub ytdlp: YtDlpConfig,
    pub storage: StorageConfig,
    pub updater: UpdaterConfig,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub google_client_id: String,
    #[serde(default)]
    pub google_client_secret: String,
    pub redirect_port: u16,
    pub keyring_service_name: String,
    pub auto_restore_session: bool,
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secret_display = if self.google_client_secret.trim().is_empty() {
            ""
        } else {
            "[REDACTED]"
        };
        f.debug_struct("AuthConfig")
            .field("google_client_id", &self.google_client_id)
            .field("google_client_secret", &secret_display)
            .field("redirect_port", &self.redirect_port)
            .field("keyring_service_name", &self.keyring_service_name)
            .field("auto_restore_session", &self.auto_restore_session)
            .finish()
    }
}

impl AuthConfig {
    pub fn is_placeholder_or_empty(client_id: &str) -> bool {
        let trimmed = client_id.trim();
        if trimmed.is_empty() {
            return true;
        }
        let upper = trimmed.to_ascii_uppercase();
        trimmed.eq_ignore_ascii_case(EXAMPLE_GOOGLE_CLIENT_ID_PLACEHOLDER)
            || upper.starts_with("YOUR_GOOGLE_OAUTH")
            || upper.starts_with("YOUR_CLIENT_ID")
            || upper.contains("YOUR_GOOGLE_OAUTH_DESKTOP_CLIENT_ID")
    }

    pub fn is_placeholder_or_empty_secret(client_secret: &str) -> bool {
        let trimmed = client_secret.trim();
        if trimmed.is_empty() {
            return true;
        }
        let upper = trimmed.to_ascii_uppercase();
        trimmed.eq_ignore_ascii_case(EXAMPLE_GOOGLE_CLIENT_SECRET_PLACEHOLDER)
            || upper.starts_with("YOUR_GOOGLE_OAUTH")
            || upper.starts_with("YOUR_CLIENT_SECRET")
            || upper.contains("YOUR_GOOGLE_OAUTH_DESKTOP_CLIENT_SECRET")
    }

    pub fn normalize_client_secret_str(client_secret: &str) -> Option<String> {
        let trimmed = client_secret.trim();
        if Self::is_placeholder_or_empty_secret(trimmed) {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    pub fn normalized_client_secret(&self) -> Option<String> {
        Self::normalize_client_secret_str(&self.google_client_secret)
    }

    pub fn validate_client_id(&self) -> std::result::Result<String, ClassifiedError> {
        Self::validate_client_id_str(&self.google_client_id)
    }

    pub fn validate_client_id_str(client_id: &str) -> std::result::Result<String, ClassifiedError> {
        let trimmed = client_id.trim();
        if Self::is_placeholder_or_empty(trimmed) {
            return Err(ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: format!(
                    "Google OAuth Client ID is not configured or still set to the example placeholder. Set a valid Desktop App `google_client_id` in `{}` or `./config.toml`.",
                    AppConfig::config_file_path().display()
                ),
                occurred_at: Utc::now().timestamp(),
            });
        }
        Ok(trimmed.to_string())
    }

    pub fn loopback_redirect_uri(&self) -> String {
        let port = if self.redirect_port == 0 {
            8989
        } else {
            self.redirect_port
        };
        format!("http://127.0.0.1:{}/callback", port)
    }
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
                google_client_secret: String::new(),
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
    /// Returns the persistent user configuration file path in
    /// `%APPDATA%\RustTube\RustTube\config\config.toml` (via `ProjectDirs::from("com", "RustTube", "RustTube")`),
    /// separated from the binary installation directory so application updates never touch it.
    pub fn config_file_path() -> PathBuf {
        if let Some(proj) = ProjectDirs::from("com", "RustTube", "RustTube") {
            let dir = proj.config_dir();
            let _ = fs::create_dir_all(dir);
            return dir.join("config.toml");
        }
        PathBuf::from("config.toml")
    }

    /// Returns the persistent user data directory in `%LOCALAPPDATA%\RustTube\RustTube\data\data`
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
        let local_fallback = PathBuf::from("config.toml");

        if primary.exists() {
            let raw = fs::read_to_string(&primary)
                .with_context(|| format!("Failed to read {}", primary.display()))?;
            let mut cfg: AppConfig = toml::from_str(&raw).unwrap_or_default();

            // Merge missing OAuth fields from `./config.toml` if primary config is missing them
            if local_fallback.exists() && local_fallback != primary {
                if let Ok(local_raw) = fs::read_to_string(&local_fallback) {
                    if let Ok(local_cfg) = toml::from_str::<AppConfig>(&local_raw) {
                        let mut updated = false;
                        if AuthConfig::is_placeholder_or_empty(&cfg.auth.google_client_id)
                            && !AuthConfig::is_placeholder_or_empty(
                                &local_cfg.auth.google_client_id,
                            )
                        {
                            cfg.auth.google_client_id = local_cfg.auth.google_client_id;
                            updated = true;
                        }
                        if cfg.auth.normalized_client_secret().is_none()
                            && local_cfg.auth.normalized_client_secret().is_some()
                        {
                            cfg.auth.google_client_secret = local_cfg.auth.google_client_secret;
                            updated = true;
                        }
                        if updated {
                            let _ = cfg.save();
                        }
                    }
                }
            }
            return Ok(cfg);
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_client_id_rejects_empty_and_placeholder() {
        assert!(AuthConfig::validate_client_id_str("").is_err());
        assert!(AuthConfig::validate_client_id_str("   ").is_err());
        assert!(
            AuthConfig::validate_client_id_str(EXAMPLE_GOOGLE_CLIENT_ID_PLACEHOLDER).is_err()
        );
        assert!(
            AuthConfig::validate_client_id_str(
                "your_google_oauth_desktop_client_id.apps.googleusercontent.com"
            )
            .is_err()
        );

        let valid = AuthConfig::validate_client_id_str(
            "  1234567890-abcdefg.apps.googleusercontent.com  ",
        )
        .expect("valid desktop client id");
        assert_eq!(valid, "1234567890-abcdefg.apps.googleusercontent.com");
    }

    #[test]
    fn test_legacy_config_without_client_secret_deserializes_and_redacts_debug() {
        let legacy_toml = r#"
[auth]
google_client_id = "1234567890-legacy.apps.googleusercontent.com"
redirect_port = 8989
keyring_service_name = "RustTube_Google_OAuth"
auto_restore_session = true

[sync]
auto_sync_enabled = true
sync_interval_minutes = 30
max_uploads_per_channel = 8

[playback]
hwdec = "auto-safe"
default_quality = "1080p"
default_volume = 80
default_speed = 1.0
resume_playback = true

[ytdlp]
binary_path = "bin/yt-dlp.exe"
cookies_browser = "none"
experimental_cookie_home_feed = false
experimental_cookie_watch_later = false

[storage]
custom_data_dir = ""
thumbnail_cache_mb = 256

[updater]
auto_check_updates = true
include_prereleases = false
github_repo = "duongducdungnguyen-dot/RustTube"
"#;

        let parsed: AppConfig =
            toml::from_str(legacy_toml).expect("legacy config without google_client_secret must parse");
        assert_eq!(parsed.auth.google_client_secret, "");
        assert!(parsed.auth.normalized_client_secret().is_none());

        let mut with_secret = parsed.auth.clone();
        with_secret.google_client_secret = "  test_only_placeholder_value_999  ".to_string();
        assert_eq!(
            with_secret.normalized_client_secret().as_deref(),
            Some("test_only_placeholder_value_999")
        );

        let debug_str = format!("{:?}", with_secret);
        assert!(!debug_str.contains("test_only_placeholder_value_999"));
        assert!(debug_str.contains("[REDACTED]"));
    }
}
