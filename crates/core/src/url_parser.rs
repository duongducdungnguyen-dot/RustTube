use crate::models::YouTubeUrlType;
use url::Url;

pub struct YouTubeUrlParser;

impl YouTubeUrlParser {
    pub fn is_valid_video_id(id: &str) -> bool {
        id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    pub fn parse(input: &str) -> YouTubeUrlType {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return YouTubeUrlType::Invalid;
        }

        if Self::is_valid_video_id(trimmed) {
            return YouTubeUrlType::Video(trimmed.to_string());
        }

        let parsed_url = match Url::parse(trimmed) {
            Ok(u) => u,
            Err(_) => {
                if let Ok(u) = Url::parse(&format!("https://{}", trimmed)) {
                    u
                } else {
                    return YouTubeUrlType::Search(trimmed.to_string());
                }
            }
        };

        let host = match parsed_url.host_str() {
            Some(h) => h.to_lowercase(),
            None => return YouTubeUrlType::Search(trimmed.to_string()),
        };

        if !host.contains("youtube.com") && !host.contains("youtu.be") {
            return YouTubeUrlType::Search(trimmed.to_string());
        }

        if host == "youtu.be" || host.ends_with(".youtu.be") {
            let path_segments: Vec<&str> = parsed_url.path_segments().map(|c| c.collect()).unwrap_or_default();
            if let Some(&first) = path_segments.first() {
                if Self::is_valid_video_id(first) {
                    return YouTubeUrlType::Video(first.to_string());
                }
            }
        }

        let path = parsed_url.path();

        if path.starts_with("/shorts/") {
            let id = path.trim_start_matches("/shorts/").split('/').next().unwrap_or("");
            if Self::is_valid_video_id(id) {
                return YouTubeUrlType::Shorts(id.to_string());
            }
        }

        if path == "/watch" {
            for (key, val) in parsed_url.query_pairs() {
                if key == "v" && Self::is_valid_video_id(&val) {
                    return YouTubeUrlType::Video(val.to_string());
                }
            }
        }

        if path == "/playlist" {
            for (key, val) in parsed_url.query_pairs() {
                if key == "list" && !val.is_empty() {
                    return YouTubeUrlType::Playlist(val.to_string());
                }
            }
        }

        if path.starts_with("/@") {
            let channel_handle = path.trim_start_matches('/');
            return YouTubeUrlType::Channel(channel_handle.to_string());
        }

        if path.starts_with("/channel/") {
            let channel_id = path.trim_start_matches("/channel/");
            return YouTubeUrlType::Channel(channel_id.to_string());
        }

        YouTubeUrlType::Search(trimmed.to_string())
    }
}
