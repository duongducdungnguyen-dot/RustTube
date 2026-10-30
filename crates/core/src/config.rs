use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub app: GeneralConfig,
    pub player: PlayerConfig,
    pub ytdlp: YtDlpConfig,
    pub auth: AuthConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtDlpConfig {
    pub binary_path: String,
    pub cookies_browser: String,
    pub auto_update_check: bool,
    pub max_search_results: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub google_client_id: String,
    pub redirect_port: u16,
    pub keyring_service_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub disk_cache_mb: u64,
    pub thumbnail_dir: String,
    pub persist_history: bool,
    pub persist_watch_later: bool,
    pub max_history_items: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app: GeneralConfig {
                theme: "dark".to_string(),
                window_width: 1380,
                window_height: 860,
                startup_page: "home".to_string(),
                auto_refresh_subscriptions: true,
            },
            player: PlayerConfig {
                hwdec: "auto-safe".to_string(),
                preferred_quality: "1080p".to_string(),
                volume: 80,
                speed: 1.0,
                subtitles_enabled: false,
                audio_device: "auto".to_string(),
                cache_seconds: 30,
            },
            ytdlp: YtDlpConfig {
                binary_path: "bin/yt-dlp.exe".to_string(),
                cookies_browser: "chrome".to_string(),
                auto_update_check: true,
                max_search_results: 24,
            },
            auth: AuthConfig {
                google_client_id: String::new(),
                redirect_port: 8989,
                keyring_service_name: "RustTube_Google_OAuth".to_string(),
            },
            cache: CacheConfig {
                disk_cache_mb: 512,
                thumbnail_dir: "cache/thumbnails".to_string(),
                persist_history: true,
                persist_watch_later: true,
                max_history_items: 500,
            },
        }
    }
}

impl AppConfig {
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        let p = path.as_ref();
        if p.exists() {
            if let Ok(contents) = std::fs::read_to_string(p) {
                // Parse simple key/value or toml
                if let Ok(config) = toml_simple_parse(&contents) {
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
        let serialized = format!(
            "[app]\ntheme = \"{}\"\nwindow_width = {}\nwindow_height = {}\nstartup_page = \"{}\"\n\n[player]\nhwdec = \"{}\"\npreferred_quality = \"{}\"\nvolume = {}\nspeed = {}\nsubtitles_enabled = {}\n\n[ytdlp]\nbinary_path = \"{}\"\ncookies_browser = \"{}\"\n\n[auth]\ngoogle_client_id = \"{}\"\nredirect_port = {}\nkeyring_service_name = \"{}\"\n\n[cache]\ndisk_cache_mb = {}\nthumbnail_dir = \"{}\"\n",
            self.app.theme, self.app.window_width, self.app.window_height, self.app.startup_page,
            self.player.hwdec, self.player.preferred_quality, self.player.volume, self.player.speed, self.player.subtitles_enabled,
            self.ytdlp.binary_path, self.ytdlp.cookies_browser,
            self.auth.google_client_id, self.auth.redirect_port, self.auth.keyring_service_name,
            self.cache.disk_cache_mb, self.cache.thumbnail_dir
        );
        std::fs::write(p, serialized)?;
        Ok(())
    }
}

fn toml_simple_parse(input: &str) -> Result<AppConfig> {
    let mut config = AppConfig::default();
    for line in input.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() || line.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let val = v.trim().trim_matches('"');
            match key {
                "theme" => config.app.theme = val.to_string(),
                "preferred_quality" => config.player.preferred_quality = val.to_string(),
                "hwdec" => config.player.hwdec = val.to_string(),
                "volume" => if let Ok(n) = val.parse() { config.player.volume = n; },
                "cookies_browser" => config.ytdlp.cookies_browser = val.to_string(),
                "binary_path" => config.ytdlp.binary_path = val.to_string(),
                "google_client_id" => config.auth.google_client_id = val.to_string(),
                "redirect_port" => if let Ok(n) = val.parse() { config.auth.redirect_port = n; },
                _ => {}
            }
        }
    }
    Ok(config)
}
