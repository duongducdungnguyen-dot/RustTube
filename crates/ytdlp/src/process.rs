use anyhow::{anyhow, Result};
use rusttube_core::models::{VideoDetail, VideoItem};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

#[derive(Clone)]
pub struct YtDlpWrapper {
    binary_path: PathBuf,
    cookies_browser: String,
}

impl YtDlpWrapper {
    pub fn new<P: AsRef<Path>>(binary_path: P, cookies_browser: &str) -> Self {
        Self {
            binary_path: binary_path.as_ref().to_path_buf(),
            cookies_browser: cookies_browser.to_string(),
        }
    }

    pub fn binary_exists(&self) -> bool {
        self.binary_path.exists()
    }

    fn apply_cookie_args(&self, cmd: &mut Command) {
        let b = self.cookies_browser.to_lowercase();
        if b != "none" && !b.is_empty() {
            cmd.arg("--cookies-from-browser").arg(&b);
        }
    }

    pub async fn get_version(&self) -> Result<String> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!(
                "Failed to get yt-dlp version: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub async fn search_videos(&self, query: &str, count: usize) -> Result<Vec<VideoItem>> {
        let search_target = format!("ytsearch{}:{}", count, query);
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg(&search_target)
            .arg("--dump-json")
            .arg("--flat-playlist")
            .arg("--no-warnings")
            .arg("--ignore-errors")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.apply_cookie_args(&mut cmd);

        let output = cmd.output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut results = Vec::new();
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(json) = serde_json::from_str::<Value>(line) {
                if let Some(item) = parse_video_json(&json) {
                    results.push(item);
                }
            }
        }

        Ok(results)
    }

    pub async fn get_stream_url(&self, video_id: &str, preferred_quality: &str) -> Result<String> {
        let format_spec = match preferred_quality {
            "4K" | "2160p" => "bestvideo[height<=2160]+bestaudio/best[height<=2160]/best",
            "1440p" => "bestvideo[height<=1440]+bestaudio/best[height<=1440]/best",
            "1080p" => "bestvideo[height<=1080]+bestaudio/best[height<=1080]/best",
            "720p" => "bestvideo[height<=720]+bestaudio/best[height<=720]/best",
            "480p" => "bestvideo[height<=480]+bestaudio/best[height<=480]/best",
            "360p" => "bestvideo[height<=360]+bestaudio/best[height<=360]/best",
            _ => "bestvideo+bestaudio/best",
        };

        let video_url = format!("https://www.youtube.com/watch?v={}", video_id);
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("-g")
            .arg("-f")
            .arg(format_spec)
            .arg("--no-playlist")
            .arg(&video_url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.apply_cookie_args(&mut cmd);

        let output = cmd.output().await?;
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let first_url = stdout.lines().next().unwrap_or("").trim().to_string();
            if !first_url.is_empty() {
                return Ok(first_url);
            }
        }

        // Fallback: pass full video URL directly to mpv with ytdl hook
        Ok(video_url)
    }

    pub async fn get_video_detail(&self, video_id: &str) -> Result<VideoDetail> {
        let video_url = format!("https://www.youtube.com/watch?v={}", video_id);
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("--dump-json")
            .arg("--no-playlist")
            .arg(&video_url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.apply_cookie_args(&mut cmd);

        let output = cmd.output().await?;
        if !output.status.success() {
            return Err(anyhow!(
                "Failed to get video info: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: Value = serde_json::from_str(&stdout)?;

        let item = parse_video_json(&json).ok_or_else(|| anyhow!("Failed to parse video info"))?;
        let description = json["description"].as_str().unwrap_or("").to_string();

        let mut available_qualities = Vec::new();
        if let Some(formats) = json["formats"].as_array() {
            for f in formats {
                if let Some(h) = f["height"].as_u64() {
                    let label = format!("{}p", h);
                    if !available_qualities.contains(&label) {
                        available_qualities.push(label);
                    }
                }
            }
        }
        if available_qualities.is_empty() {
            available_qualities = vec!["360p".into(), "720p".into(), "1080p".into()];
        }

        Ok(VideoDetail {
            item,
            description,
            available_qualities,
            selected_quality: "1080p".to_string(),
            is_liked: false,
            is_watch_later: false,
        })
    }

    pub async fn update_binary(&self) -> Result<String> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("-U")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await?;
        let out_str = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(out_str)
    }
}

fn parse_video_json(json: &Value) -> Option<VideoItem> {
    let id = json["id"].as_str()?.to_string();
    let title = json["title"].as_str()?.to_string();
    let channel_name = json["channel"]
        .as_str()
        .or_else(|| json["uploader"].as_str())
        .unwrap_or("Unknown")
        .to_string();
    let channel_id = json["channel_id"]
        .as_str()
        .or_else(|| json["uploader_id"].as_str())
        .unwrap_or("")
        .to_string();

    let duration_seconds = json["duration"].as_f64().unwrap_or(0.0) as u32;
    let duration_string = format_duration(duration_seconds);
    let view_count = json["view_count"].as_u64().unwrap_or(0);

    let thumbnail_url = json["thumbnail"]
        .as_str()
        .or_else(|| {
            json["thumbnails"]
                .as_array()
                .and_then(|a| a.last())
                .and_then(|t| t["url"].as_str())
        })
        .unwrap_or("")
        .to_string();

    let published_time = json["upload_date"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Some(VideoItem {
        id,
        title,
        channel_name,
        channel_id,
        duration_seconds,
        duration_string,
        view_count,
        thumbnail_url,
        published_time,
    })
}

fn format_duration(total_seconds: u32) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}
