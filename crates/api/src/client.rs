use chrono::Utc;
use reqwest::{Client, StatusCode};
use rusttube_core::{
    ChannelItem, ClassifiedError, PlaylistItem, PlaylistPrivacy, PlaylistVideoItem, SyncErrorKind,
    VideoAvailability, VideoItem, VideoRating,
};
use serde_json::{json, Value};
use std::collections::HashSet;

const YT_API_BASE: &str = "https://www.googleapis.com/youtube/v3";

#[derive(Clone)]
pub struct YouTubeApiClient {
    http: Client,
}

impl Default for YouTubeApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl YouTubeApiClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Classifies HTTP status codes and Google API error payloads into granular SyncErrorKind variants.
    pub fn classify_http_error(status: StatusCode, body_text: &str) -> ClassifiedError {
        let now = Utc::now().timestamp();
        let lower = body_text.to_lowercase();

        if status == StatusCode::UNAUTHORIZED {
            return ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: "Google OAuth access token expired or invalid (HTTP 401)".to_string(),
                occurred_at: now,
            };
        }

        if status == StatusCode::FORBIDDEN {
            if lower.contains("quotaexceeded") || lower.contains("dailylimitexceeded") {
                return ClassifiedError {
                    kind: SyncErrorKind::QuotaExceeded,
                    message: "YouTube Data API v3 daily quota exceeded (HTTP 403)".to_string(),
                    occurred_at: now,
                };
            }
            if lower.contains("insufficientpermissions") || lower.contains("scope") {
                return ClassifiedError {
                    kind: SyncErrorKind::AuthorizationScope,
                    message: "Missing required YouTube OAuth scope for this operation".to_string(),
                    occurred_at: now,
                };
            }
            return ClassifiedError {
                kind: SyncErrorKind::AuthorizationScope,
                message: format!("YouTube API access forbidden (HTTP 403): {}", body_text),
                occurred_at: now,
            };
        }

        ClassifiedError {
            kind: SyncErrorKind::Network,
            message: format!("YouTube API returned HTTP {}", status.as_u16()),
            occurred_at: now,
        }
    }

    /// Retrieves ALL subscribed channels by exhausting nextPageToken pagination,
    /// then resolves each channel's uploads playlist in batched channels.list calls (50 IDs per request).
    pub async fn fetch_all_subscriptions(
        &self,
        access_token: &str,
    ) -> Result<Vec<ChannelItem>, ClassifiedError> {
        let mut subscriptions = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut req = self
                .http
                .get(format!("{}/subscriptions", YT_API_BASE))
                .bearer_auth(access_token)
                .query(&[
                    ("part", "snippet"),
                    ("mine", "true"),
                    ("maxResults", "50"),
                    ("order", "alphabetical"),
                ]);

            if let Some(ref token) = page_token {
                req = req.query(&[("pageToken", token.as_str())]);
            }

            let resp = req.send().await.map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Failed to fetch subscriptions: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(Self::classify_http_error(status, &text));
            }

            let payload: Value = resp.json().await.map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Parsing,
                message: format!("Failed to parse subscriptions JSON: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

            if let Some(items) = payload.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    let sub_id = item.get("id").and_then(|v| v.as_str()).map(String::from);
                    let snippet = item.get("snippet").unwrap_or(&Value::Null);
                    let channel_id = snippet
                        .get("resourceId")
                        .and_then(|r| r.get("channelId"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();

                    if channel_id.is_empty() {
                        continue;
                    }

                    let uploads_guess = if let Some(rest) = channel_id.strip_prefix("UC") {
                        Some(format!("UU{}", rest))
                    } else {
                        None
                    };

                    subscriptions.push(ChannelItem {
                        channel_id,
                        subscription_id: sub_id,
                        title: snippet
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("YouTube Channel")
                            .to_string(),
                        description: snippet
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        custom_url: None,
                        thumbnail_url: snippet
                            .pointer("/thumbnails/high/url")
                            .or_else(|| snippet.pointer("/thumbnails/default/url"))
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        subscriber_count: None,
                        video_count: None,
                        uploads_playlist_id: uploads_guess,
                        synced_at: Utc::now().timestamp(),
                    });
                }
            }

            page_token = payload
                .get("nextPageToken")
                .and_then(|v| v.as_str())
                .map(String::from);
            if page_token.is_none() {
                break;
            }
        }

        // Enrich channels in batches of 50 with contentDetails.relatedPlaylists.uploads & statistics
        for chunk in subscriptions.chunks_mut(50) {
            let ids: Vec<String> = chunk.iter().map(|c| c.channel_id.clone()).collect();
            let joined = ids.join(",");
            if let Ok(resp) = self
                .http
                .get(format!("{}/channels", YT_API_BASE))
                .bearer_auth(access_token)
                .query(&[
                    ("part", "contentDetails,statistics,snippet"),
                    ("id", joined.as_str()),
                ])
                .send()
                .await
            {
                if let Ok(payload) = resp.json::<Value>().await {
                    if let Some(items) = payload.get("items").and_then(|v| v.as_array()) {
                        for info in items {
                            let cid = info.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                            if let Some(target) = chunk.iter_mut().find(|c| c.channel_id == cid) {
                                if let Some(uploads) = info
                                    .pointer("/contentDetails/relatedPlaylists/uploads")
                                    .and_then(|v| v.as_str())
                                {
                                    target.uploads_playlist_id = Some(uploads.to_string());
                                }
                                if let Some(subs) = info
                                    .pointer("/statistics/subscriberCount")
                                    .and_then(|v| v.as_str())
                                {
                                    target.subscriber_count = Some(subs.to_string());
                                }
                                if let Some(custom) =
                                    info.pointer("/snippet/customUrl").and_then(|v| v.as_str())
                                {
                                    target.custom_url = Some(custom.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(subscriptions)
    }

    pub async fn unsubscribe_channel(
        &self,
        access_token: &str,
        subscription_id: &str,
    ) -> Result<(), ClassifiedError> {
        let resp = self
            .http
            .delete(format!("{}/subscriptions", YT_API_BASE))
            .bearer_auth(access_token)
            .query(&[("id", subscription_id)])
            .send()
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Network error unsubscribing: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::classify_http_error(status, &text));
        }
        Ok(())
    }

    /// Retrieves recent uploads from a channel's uploads playlist using playlistItems.list (1 quota unit).
    pub async fn fetch_playlist_videos_page(
        &self,
        access_token: &str,
        playlist_id: &str,
        max_results: u32,
        page_token: Option<&str>,
    ) -> Result<(Vec<PlaylistVideoItem>, Option<String>), ClassifiedError> {
        let max_str = max_results.clamp(1, 50).to_string();
        let mut req = self
            .http
            .get(format!("{}/playlistItems", YT_API_BASE))
            .bearer_auth(access_token)
            .query(&[
                ("part", "snippet,contentDetails,status"),
                ("playlistId", playlist_id),
                ("maxResults", max_str.as_str()),
            ]);

        if let Some(token) = page_token {
            req = req.query(&[("pageToken", token)]);
        }

        let resp = req.send().await.map_err(|e| ClassifiedError {
            kind: SyncErrorKind::Network,
            message: format!("Failed to fetch playlist items: {}", e),
            occurred_at: Utc::now().timestamp(),
        })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::classify_http_error(status, &text));
        }

        let payload: Value = resp.json().await.map_err(|e| ClassifiedError {
            kind: SyncErrorKind::Parsing,
            message: format!("Malformed playlistItems JSON: {}", e),
            occurred_at: Utc::now().timestamp(),
        })?;

        let items = Self::parse_playlist_items_payload(playlist_id, &payload);
        let next_token = payload
            .get("nextPageToken")
            .and_then(|v| v.as_str())
            .map(String::from);

        Ok((items, next_token))
    }

    pub fn parse_playlist_items_payload(
        playlist_id: &str,
        payload: &Value,
    ) -> Vec<PlaylistVideoItem> {
        let mut out = Vec::new();
        let Some(arr) = payload.get("items").and_then(|v| v.as_array()) else {
            return out;
        };

        for (idx, item) in arr.iter().enumerate() {
            let playlist_item_id = item
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let snippet = item.get("snippet").unwrap_or(&Value::Null);
            let title = snippet
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();
            let privacy = item
                .pointer("/status/privacyStatus")
                .and_then(|v| v.as_str())
                .unwrap_or("public");

            let availability = if title == "Private video" || privacy == "private" {
                VideoAvailability::Private
            } else if title == "Deleted video" {
                VideoAvailability::Deleted
            } else {
                VideoAvailability::Available
            };

            let video_id = snippet
                .pointer("/resourceId/videoId")
                .or_else(|| item.pointer("/contentDetails/videoId"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let published_at = item
                .pointer("/contentDetails/videoPublishedAt")
                .or_else(|| snippet.get("publishedAt"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let published_ts = chrono::DateTime::parse_from_rfc3339(&published_at)
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            let position = snippet
                .get("position")
                .and_then(|v| v.as_u64())
                .unwrap_or(idx as u64) as u32;

            out.push(PlaylistVideoItem {
                playlist_item_id,
                playlist_id: playlist_id.to_string(),
                position,
                availability_status: availability,
                video: VideoItem {
                    id: video_id.clone(),
                    title,
                    channel_name: snippet
                        .get("videoOwnerChannelTitle")
                        .or_else(|| snippet.get("channelTitle"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("YouTube Channel")
                        .to_string(),
                    channel_id: snippet
                        .get("videoOwnerChannelId")
                        .or_else(|| snippet.get("channelId"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    duration_seconds: 0,
                    duration_string: String::new(),
                    view_count: 0,
                    views_text: String::new(),
                    published_at,
                    published_timestamp: published_ts,
                    thumbnail_url: snippet
                        .pointer("/thumbnails/high/url")
                        .or_else(|| snippet.pointer("/thumbnails/medium/url"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .unwrap_or_else(|| {
                            format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video_id)
                        }),
                    description: snippet
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    resume_position_seconds: None,
                    source_tag: None,
                },
            });
        }
        out
    }

    /// Builds a personalized Home feed from subscribed channels' uploads playlists, liked videos, and local history,
    /// sorted by publication timestamp descending and deduplicated by video ID.
    pub fn merge_and_deduplicate_personalized_feed(batches: Vec<Vec<VideoItem>>) -> Vec<VideoItem> {
        let mut seen = HashSet::new();
        let mut merged = Vec::new();

        for batch in batches {
            for v in batch {
                if !v.id.is_empty() && seen.insert(v.id.clone()) {
                    merged.push(v);
                }
            }
        }

        merged.sort_by(|a, b| b.published_timestamp.cmp(&a.published_timestamp));
        merged
    }

    /// Retrieves all owned playlists using pagination.
    pub async fn fetch_all_playlists(
        &self,
        access_token: &str,
    ) -> Result<Vec<PlaylistItem>, ClassifiedError> {
        let mut playlists = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut req = self
                .http
                .get(format!("{}/playlists", YT_API_BASE))
                .bearer_auth(access_token)
                .query(&[
                    ("part", "snippet,contentDetails,status"),
                    ("mine", "true"),
                    ("maxResults", "50"),
                ]);

            if let Some(ref token) = page_token {
                req = req.query(&[("pageToken", token.as_str())]);
            }

            let resp = req.send().await.map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Failed to fetch playlists: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                return Err(Self::classify_http_error(status, &text));
            }

            let payload: Value = resp.json().await.map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Parsing,
                message: format!("Malformed playlists response: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

            if let Some(items) = payload.get("items").and_then(|v| v.as_array()) {
                for item in items {
                    let pid = item
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let snippet = item.get("snippet").unwrap_or(&Value::Null);
                    let priv_str = item
                        .pointer("/status/privacyStatus")
                        .and_then(|v| v.as_str())
                        .unwrap_or("private");
                    let count = item
                        .pointer("/contentDetails/itemCount")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;

                    playlists.push(PlaylistItem {
                        playlist_id: pid,
                        title: snippet
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Untitled Playlist")
                            .to_string(),
                        description: snippet
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        item_count: count,
                        thumbnail_url: snippet
                            .pointer("/thumbnails/high/url")
                            .or_else(|| snippet.pointer("/thumbnails/medium/url"))
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        privacy_status: PlaylistPrivacy::from_api_str(priv_str),
                        is_owned: true,
                        synced_at: Utc::now().timestamp(),
                    });
                }
            }

            page_token = payload
                .get("nextPageToken")
                .and_then(|v| v.as_str())
                .map(String::from);
            if page_token.is_none() {
                break;
            }
        }

        Ok(playlists)
    }

    pub async fn create_playlist(
        &self,
        access_token: &str,
        title: &str,
        description: &str,
        privacy: PlaylistPrivacy,
    ) -> Result<PlaylistItem, ClassifiedError> {
        let body = json!({
            "snippet": {
                "title": title,
                "description": description
            },
            "status": {
                "privacyStatus": privacy.as_api_str()
            }
        });

        let resp = self
            .http
            .post(format!("{}/playlists", YT_API_BASE))
            .bearer_auth(access_token)
            .query(&[("part", "snippet,status,contentDetails")])
            .json(&body)
            .send()
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Network error creating playlist: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::classify_http_error(status, &text));
        }

        let item: Value = resp.json().await.map_err(|e| ClassifiedError {
            kind: SyncErrorKind::Parsing,
            message: format!("Failed to parse created playlist: {}", e),
            occurred_at: Utc::now().timestamp(),
        })?;

        Ok(PlaylistItem {
            playlist_id: item
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            title: title.to_string(),
            description: description.to_string(),
            item_count: 0,
            thumbnail_url: String::new(),
            privacy_status: privacy,
            is_owned: true,
            synced_at: Utc::now().timestamp(),
        })
    }

    pub async fn update_playlist(
        &self,
        access_token: &str,
        playlist_id: &str,
        title: &str,
        description: &str,
        privacy: PlaylistPrivacy,
    ) -> Result<(), ClassifiedError> {
        let body = json!({
            "id": playlist_id,
            "snippet": {
                "title": title,
                "description": description
            },
            "status": {
                "privacyStatus": privacy.as_api_str()
            }
        });

        let resp = self
            .http
            .put(format!("{}/playlists", YT_API_BASE))
            .bearer_auth(access_token)
            .query(&[("part", "snippet,status")])
            .json(&body)
            .send()
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Network error updating playlist: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::classify_http_error(status, &text));
        }
        Ok(())
    }

    pub async fn delete_playlist(
        &self,
        access_token: &str,
        playlist_id: &str,
    ) -> Result<(), ClassifiedError> {
        let resp = self
            .http
            .delete(format!("{}/playlists", YT_API_BASE))
            .bearer_auth(access_token)
            .query(&[("id", playlist_id)])
            .send()
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Network error deleting playlist: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::classify_http_error(status, &text));
        }
        Ok(())
    }

    pub async fn add_video_to_playlist(
        &self,
        access_token: &str,
        playlist_id: &str,
        video_id: &str,
    ) -> Result<String, ClassifiedError> {
        let body = json!({
            "snippet": {
                "playlistId": playlist_id,
                "resourceId": {
                    "kind": "youtube#video",
                    "videoId": video_id
                }
            }
        });

        let resp = self
            .http
            .post(format!("{}/playlistItems", YT_API_BASE))
            .bearer_auth(access_token)
            .query(&[("part", "snippet")])
            .json(&body)
            .send()
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Network error adding video to playlist: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::classify_http_error(status, &text));
        }

        let val: Value = resp.json().await.map_err(|e| ClassifiedError {
            kind: SyncErrorKind::Parsing,
            message: format!("Failed to parse playlistItem response: {}", e),
            occurred_at: Utc::now().timestamp(),
        })?;

        Ok(val
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string())
    }

    pub async fn remove_video_from_playlist(
        &self,
        access_token: &str,
        playlist_item_id: &str,
    ) -> Result<(), ClassifiedError> {
        let resp = self
            .http
            .delete(format!("{}/playlistItems", YT_API_BASE))
            .bearer_auth(access_token)
            .query(&[("id", playlist_item_id)])
            .send()
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Network error removing video from playlist: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::classify_http_error(status, &text));
        }
        Ok(())
    }

    pub async fn rate_video(
        &self,
        access_token: &str,
        video_id: &str,
        rating: VideoRating,
    ) -> Result<(), ClassifiedError> {
        let rating_str = match rating {
            VideoRating::Like => "like",
            VideoRating::None => "none",
        };

        let resp = self
            .http
            .post(format!("{}/videos/rate", YT_API_BASE))
            .bearer_auth(access_token)
            .query(&[("id", video_id), ("rating", rating_str)])
            .send()
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Network error rating video: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::classify_http_error(status, &text));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_classification_separates_auth_quota_and_scope() {
        let auth_err = YouTubeApiClient::classify_http_error(StatusCode::UNAUTHORIZED, "Invalid Credentials");
        assert_eq!(auth_err.kind, SyncErrorKind::Authentication);

        let quota_err = YouTubeApiClient::classify_http_error(
            StatusCode::FORBIDDEN,
            r#"{"error":{"errors":[{"reason":"quotaExceeded"}]}}"#,
        );
        assert_eq!(quota_err.kind, SyncErrorKind::QuotaExceeded);

        let scope_err = YouTubeApiClient::classify_http_error(
            StatusCode::FORBIDDEN,
            r#"{"error":{"errors":[{"reason":"insufficientPermissions"}]}}"#,
        );
        assert_eq!(scope_err.kind, SyncErrorKind::AuthorizationScope);
    }

    #[test]
    fn test_playlist_items_handles_private_and_deleted_videos() {
        let raw = json!({
            "items": [
                {
                    "id": "PLI_1",
                    "snippet": {
                        "title": "Private video",
                        "position": 0,
                        "resourceId": { "videoId": "priv1234567" }
                    },
                    "status": { "privacyStatus": "private" }
                },
                {
                    "id": "PLI_2",
                    "snippet": {
                        "title": "Deleted video",
                        "position": 1,
                        "resourceId": { "videoId": "del12345678" }
                    },
                    "status": { "privacyStatus": "public" }
                },
                {
                    "id": "PLI_3",
                    "snippet": {
                        "title": "Rust 2021 Systems Programming",
                        "position": 2,
                        "resourceId": { "videoId": "ok123456789" }
                    },
                    "status": { "privacyStatus": "public" }
                }
            ]
        });

        let parsed = YouTubeApiClient::parse_playlist_items_payload("PL_TEST", &raw);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].availability_status, VideoAvailability::Private);
        assert_eq!(parsed[1].availability_status, VideoAvailability::Deleted);
        assert_eq!(parsed[2].availability_status, VideoAvailability::Available);
    }
}
