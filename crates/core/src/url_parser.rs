use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedYouTubeInput {
    Video(String),
    Shorts(String),
    Playlist(String),
    Channel(String),
    Search(String),
}

pub fn parse_youtube_input(input: &str) -> ParsedYouTubeInput {
    let trimmed = input.trim();
    if is_valid_video_id(trimmed) {
        return ParsedYouTubeInput::Video(trimmed.to_string());
    }

    let candidate = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.starts_with("youtube.com/")
        || trimmed.starts_with("www.youtube.com/")
        || trimmed.starts_with("youtu.be/")
    {
        format!("https://{}", trimmed)
    } else {
        return ParsedYouTubeInput::Search(trimmed.to_string());
    };

    if let Ok(parsed) = Url::parse(&candidate) {
        let host = parsed.host_str().unwrap_or_default().to_lowercase();
        if host.contains("youtu.be") {
            if let Some(seg) = parsed.path_segments().and_then(|mut s| s.next()) {
                if is_valid_video_id(seg) {
                    return ParsedYouTubeInput::Video(seg.to_string());
                }
            }
        } else if host.contains("youtube.com") {
            let path = parsed.path();
            if path == "/watch" {
                for (k, v) in parsed.query_pairs() {
                    if k == "v" && is_valid_video_id(&v) {
                        return ParsedYouTubeInput::Video(v.into_owned());
                    }
                }
            } else if let Some(rest) = path.strip_prefix("/shorts/") {
                let id = rest.split('/').next().unwrap_or_default();
                if is_valid_video_id(id) {
                    return ParsedYouTubeInput::Shorts(id.to_string());
                }
            } else if path == "/playlist" {
                for (k, v) in parsed.query_pairs() {
                    if k == "list" && !v.is_empty() {
                        return ParsedYouTubeInput::Playlist(v.into_owned());
                    }
                }
            } else if let Some(rest) = path.strip_prefix("/channel/") {
                let cid = rest.split('/').next().unwrap_or_default();
                if !cid.is_empty() {
                    return ParsedYouTubeInput::Channel(cid.to_string());
                }
            }
        }
    }

    ParsedYouTubeInput::Search(trimmed.to_string())
}

pub fn is_valid_video_id(s: &str) -> bool {
    s.len() == 11
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_direct_video_id() {
        assert_eq!(
            parse_youtube_input("dQw4w9WgXcQ"),
            ParsedYouTubeInput::Video("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_parse_watch_and_short_links() {
        assert_eq!(
            parse_youtube_input("https://www.youtube.com/watch?v=jfKfPfyJRdk&t=10s"),
            ParsedYouTubeInput::Video("jfKfPfyJRdk".to_string())
        );
        assert_eq!(
            parse_youtube_input("https://youtu.be/L_LUpnjgPso"),
            ParsedYouTubeInput::Video("L_LUpnjgPso".to_string())
        );
        assert_eq!(
            parse_youtube_input("https://www.youtube.com/shorts/5C_HPTJg5ek"),
            ParsedYouTubeInput::Shorts("5C_HPTJg5ek".to_string())
        );
    }

    #[test]
    fn test_parse_playlist_and_channel() {
        assert_eq!(
            parse_youtube_input("https://www.youtube.com/playlist?list=PL1234567890"),
            ParsedYouTubeInput::Playlist("PL1234567890".to_string())
        );
        assert_eq!(
            parse_youtube_input("https://www.youtube.com/channel/UCSJ4gkVC6NrvII8umztf0Ow"),
            ParsedYouTubeInput::Channel("UCSJ4gkVC6NrvII8umztf0Ow".to_string())
        );
    }
}
