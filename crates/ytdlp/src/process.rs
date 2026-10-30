use anyhow::{Context, Result};
use rusttube_core::VideoItem;
use serde_json::Value;
use tokio::process::Command;

#[derive(Clone)]
pub struct YtDlpRunner {
    binary_path: String,
    cookies_browser: String,
}

impl YtDlpRunner {
    pub fn new(binary_path: impl Into<String>, cookies_browser: impl Into<String>) -> Self {
        Self {
            binary_path: binary_path.into(),
            cookies_browser: cookies_browser.into(),
        }
    }

    pub async fn search(&self, query: &str, max_results: usize) -> Result<Vec<VideoItem>> {
        let search_arg = format!("ytsearch{}:{}", max_results.clamp(1, 50), query);
        let args = vec![
            search_arg,
            "--dump-json".to_string(),
            "--flat-playlist".to_string(),
            "--no-warnings".to_string(),
            "--ignore-errors".to_string(),
        ];
        self.run_json_lines(&args, false).await
    }

    /// Optional experimental cookie-based YouTube homepage provider (:ytsubscriptions / :ytfav / homepage)
    /// Requires explicit user opt-in (`experimental_cookie_home_feed = true`).
    pub async fn fetch_experimental_cookie_home(&self, max_results: usize) -> Result<Vec<VideoItem>> {
        if self.cookies_browser == "none" || self.cookies_browser.is_empty() {
            anyhow::bail!("Browser cookies must be enabled in Settings to use experimental homepage extraction");
        }
        let args = vec![
            "https://www.youtube.com".to_string(),
            "--playlist-end".to_string(),
            max_results.to_string(),
            "--dump-json".to_string(),
            "--flat-playlist".to_string(),
            "--no-warnings".to_string(),
        ];
        self.run_json_lines(&args, true).await
    }

    /// Optional experimental cookie-based YouTube Watch Later extraction (`:ytwatchlater`).
    pub async fn fetch_experimental_watch_later(&self, max_results: usize) -> Result<Vec<VideoItem>> {
        if self.cookies_browser == "none" || self.cookies_browser.is_empty() {
            anyhow::bail!("Browser cookies must be configured to read YouTube Watch Later (:ytwatchlater)");
        }
        let args = vec![
            ":ytwatchlater".to_string(),
            "--playlist-end".to_string(),
            max_results.to_string(),
            "--dump-json".to_string(),
            "--flat-playlist".to_string(),
            "--no-warnings".to_string(),
        ];
        self.run_json_lines(&args, true).await
    }

    async fn run_json_lines(&self, base_args: &[String], use_cookies: bool) -> Result<Vec<VideoItem>> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.args(base_args);
        if use_cookies && self.cookies_browser != "none" && !self.cookies_browser.is_empty() {
            cmd.arg("--cookies-from-browser").arg(&self.cookies_browser);
        }

        let output = cmd
            .output()
            .await
            .with_context(|| format!("Failed to execute yt-dlp at {}", self.binary_path))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut videos = Vec::new();

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
                if let Some(item) = Self::parse_video_json(&json) {
                    videos.push(item);
                }
            }
        }
        Ok(videos)
    }

    pub fn parse_video_json(json: &Value) -> Option<VideoItem> {
        let id = json.get("id")?.as_str()?.to_string();
        let duration_sec = json.get("duration").and_then(|v| v.as_u64()).unwrap_or(0);
        let duration_string = json
            .get("duration_string")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| format!("{}:{:02}", duration_sec / 60, duration_sec % 60));

        let view_count = json.get("view_count").and_then(|v| v.as_u64()).unwrap_or(0);
        let views_text = if view_count >= 1_000_000 {
            format!("{:.1}M views", view_count as f64 / 1_000_000.0)
        } else if view_count >= 1_000 {
            format!("{:.1}K views", view_count as f64 / 1_000.0)
        } else {
            format!("{} views", view_count)
        };

        Some(VideoItem {
            id: id.clone(),
            title: json
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string(),
            channel_name: json
                .get("channel")
                .or_else(|| json.get("uploader"))
                .and_then(|v| v.as_str())
                .unwrap_or("YouTube Channel")
                .to_string(),
            channel_id: json
                .get("channel_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            duration_seconds: duration_sec,
            duration_string,
            view_count,
            views_text,
            published_at: json
                .get("upload_date")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            published_timestamp: json.get("timestamp").and_then(|v| v.as_i64()).unwrap_or(0),
            thumbnail_url: json
                .get("thumbnail")
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", id)),
            description: json
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            resume_position_seconds: None,
            source_tag: None,
        })
    }
}
