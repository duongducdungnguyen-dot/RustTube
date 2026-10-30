use anyhow::{anyhow, Result};
use reqwest::Client;
use rusttube_core::models::{ChannelItem, PlaylistItem, VideoItem};
use serde_json::Value;

pub struct YouTubeApiClient {
    client: Client,
    base_url: &'static str,
}

impl Default for YouTubeApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl YouTubeApiClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            base_url: "https://www.googleapis.com/youtube/v3",
        }
    }

    pub async fn get_my_channel(&self, access_token: &str) -> Result<(String, String, String)> {
        let url = format!("{}/channels?part=snippet&mine=true", self.base_url);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("YouTube API channels error: {}", resp.status()));
        }

        let json: Value = resp.json().await?;
        let item = json["items"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| anyhow!("No channel found for authenticated account"))?;

        let title = item["snippet"]["title"]
            .as_str()
            .unwrap_or("Personal Account")
            .to_string();
        let custom_url = item["snippet"]["customUrl"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let avatar_url = item["snippet"]["thumbnails"]["default"]["url"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok((title, custom_url, avatar_url))
    }

    pub async fn get_subscriptions(&self, access_token: &str, page_token: Option<&str>) -> Result<(Vec<ChannelItem>, Option<String>)> {
        let mut url = format!(
            "{}/subscriptions?part=snippet&mine=true&maxResults=50&order=alphabetical",
            self.base_url
        );
        if let Some(token) = page_token {
            url.push_str(&format!("&pageToken={}", token));
        }

        let resp = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("YouTube API subscriptions error: {}", resp.status()));
        }

        let json: Value = resp.json().await?;
        let next_page_token = json["nextPageToken"].as_str().map(|s| s.to_string());
        let mut results = Vec::new();

        if let Some(items) = json["items"].as_array() {
            for item in items {
                let snippet = &item["snippet"];
                let id = snippet["resourceId"]["channelId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let title = snippet["title"].as_str().unwrap_or_default().to_string();
                let description = snippet["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let avatar_url = snippet["thumbnails"]["default"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                results.push(ChannelItem {
                    id,
                    title,
                    description,
                    subscriber_count: 0,
                    avatar_url,
                });
            }
        }

        Ok((results, next_page_token))
    }

    pub async fn get_playlists(&self, access_token: &str, page_token: Option<&str>) -> Result<(Vec<PlaylistItem>, Option<String>)> {
        let mut url = format!(
            "{}/playlists?part=snippet,contentDetails&mine=true&maxResults=50",
            self.base_url
        );
        if let Some(token) = page_token {
            url.push_str(&format!("&pageToken={}", token));
        }

        let resp = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("YouTube API playlists error: {}", resp.status()));
        }

        let json: Value = resp.json().await?;
        let next_page_token = json["nextPageToken"].as_str().map(|s| s.to_string());
        let mut results = Vec::new();

        if let Some(items) = json["items"].as_array() {
            for item in items {
                let id = item["id"].as_str().unwrap_or_default().to_string();
                let snippet = &item["snippet"];
                let title = snippet["title"].as_str().unwrap_or_default().to_string();
                let description = snippet["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let video_count = item["contentDetails"]["itemCount"]
                    .as_u64()
                    .unwrap_or(0) as u32;
                let thumbnail_url = snippet["thumbnails"]["medium"]["url"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                results.push(PlaylistItem {
                    id,
                    title,
                    description,
                    video_count,
                    thumbnail_url,
                });
            }
        }

        Ok((results, next_page_token))
    }

    pub async fn get_playlist_items(
        &self,
        access_token: &str,
        playlist_id: &str,
    ) -> Result<Vec<VideoItem>> {
        let url = format!(
            "{}/playlistItems?part=snippet,contentDetails&playlistId={}&maxResults=50",
            self.base_url, playlist_id
        );
        let resp = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("YouTube API playlistItems error: {}", resp.status()));
        }

        let json: Value = resp.json().await?;
        let mut results = Vec::new();

        if let Some(items) = json["items"].as_array() {
            for item in items {
                let snippet = &item["snippet"];
                let video_id = snippet["resourceId"]["videoId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let title = snippet["title"].as_str().unwrap_or_default().to_string();
                let channel_name = snippet["videoOwnerChannelTitle"]
                    .as_str()
                    .unwrap_or("YouTube Channel")
                    .to_string();
                let channel_id = snippet["videoOwnerChannelId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let thumbnail_url = snippet["thumbnails"]["high"]["url"]
                    .as_str()
                    .or_else(|| snippet["thumbnails"]["medium"]["url"].as_str())
                    .unwrap_or_default()
                    .to_string();
                let published_time = snippet["publishedAt"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                results.push(VideoItem {
                    id: video_id,
                    title,
                    channel_name,
                    channel_id,
                    duration_seconds: 0,
                    duration_string: "--:--".to_string(),
                    view_count: 0,
                    thumbnail_url,
                    published_time,
                });
            }
        }

        Ok(results)
    }

    pub async fn get_liked_videos(&self, access_token: &str) -> Result<Vec<VideoItem>> {
        let url = format!(
            "{}/videos?part=snippet,contentDetails,statistics&myRating=like&maxResults=50",
            self.base_url
        );
        let resp = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("YouTube API liked videos error: {}", resp.status()));
        }

        let json: Value = resp.json().await?;
        let mut results = Vec::new();

        if let Some(items) = json["items"].as_array() {
            for item in items {
                let id = item["id"].as_str().unwrap_or_default().to_string();
                let snippet = &item["snippet"];
                let title = snippet["title"].as_str().unwrap_or_default().to_string();
                let channel_name = snippet["channelTitle"]
                    .as_str()
                    .unwrap_or("YouTube Channel")
                    .to_string();
                let channel_id = snippet["channelId"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let thumbnail_url = snippet["thumbnails"]["high"]["url"]
                    .as_str()
                    .or_else(|| snippet["thumbnails"]["medium"]["url"].as_str())
                    .unwrap_or_default()
                    .to_string();
                let view_count = item["statistics"]["viewCount"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let published_time = snippet["publishedAt"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                results.push(VideoItem {
                    id,
                    title,
                    channel_name,
                    channel_id,
                    duration_seconds: 0,
                    duration_string: "--:--".to_string(),
                    view_count,
                    thumbnail_url,
                    published_time,
                });
            }
        }

        Ok(results)
    }

    pub async fn set_video_rating(&self, access_token: &str, video_id: &str, rating: &str) -> Result<()> {
        let url = format!("{}/videos/rate?id={}&rating={}", self.base_url, video_id, rating);
        let resp = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .header("Content-Length", 0)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to rate video: {}", resp.status()));
        }
        Ok(())
    }

    pub async fn create_playlist(
        &self,
        access_token: &str,
        title: &str,
        description: &str,
        is_private: bool,
    ) -> Result<PlaylistItem> {
        let url = format!("{}/playlists?part=snippet,status", self.base_url);
        let privacy_status = if is_private { "private" } else { "public" };
        let body = serde_json::json!({
            "snippet": {
                "title": title,
                "description": description,
            },
            "status": {
                "privacyStatus": privacy_status
            }
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to create playlist: {}", resp.status()));
        }

        let json: Value = resp.json().await?;
        let id = json["id"].as_str().unwrap_or_default().to_string();
        let snippet = &json["snippet"];
        let res_title = snippet["title"].as_str().unwrap_or(title).to_string();
        let res_desc = snippet["description"].as_str().unwrap_or(description).to_string();

        Ok(PlaylistItem {
            id,
            title: res_title,
            description: res_desc,
            video_count: 0,
            thumbnail_url: String::new(),
        })
    }

    pub async fn insert_playlist_item(
        &self,
        access_token: &str,
        playlist_id: &str,
        video_id: &str,
    ) -> Result<String> {
        let url = format!("{}/playlistItems?part=snippet", self.base_url);
        let body = serde_json::json!({
            "snippet": {
                "playlistId": playlist_id,
                "resourceId": {
                    "kind": "youtube#video",
                    "videoId": video_id
                }
            }
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to add video to playlist: {}", resp.status()));
        }

        let json: Value = resp.json().await?;
        let item_id = json["id"].as_str().unwrap_or_default().to_string();
        Ok(item_id)
    }

    pub async fn delete_playlist_item(&self, access_token: &str, playlist_item_id: &str) -> Result<()> {
        let url = format!("{}/playlistItems?id={}", self.base_url, playlist_item_id);
        let resp = self
            .client
            .delete(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to remove playlist item: {}", resp.status()));
        }
        Ok(())
    }

    pub async fn subscribe_channel(&self, access_token: &str, channel_id: &str) -> Result<()> {
        let url = format!("{}/subscriptions?part=snippet", self.base_url);
        let body = serde_json::json!({
            "snippet": {
                "resourceId": {
                    "kind": "youtube#channel",
                    "channelId": channel_id
                }
            }
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to subscribe to channel: {}", resp.status()));
        }
        Ok(())
    }

    pub async fn unsubscribe_channel(&self, access_token: &str, subscription_id: &str) -> Result<()> {
        let url = format!("{}/subscriptions?id={}", self.base_url, subscription_id);
        let resp = self
            .client
            .delete(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to unsubscribe from channel: {}", resp.status()));
        }
        Ok(())
    }

    pub async fn get_channel_detail(
        &self,
        access_token: Option<&str>,
        channel_id: &str,
    ) -> Result<rusttube_core::models::ChannelDetail> {
        let url = format!(
            "{}/channels?part=snippet,contentDetails,statistics,brandingSettings&id={}",
            self.base_url, channel_id
        );
        let mut req = self.client.get(&url);
        if let Some(token) = access_token {
            req = req.bearer_auth(token);
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("YouTube API channel error: {}", resp.status()));
        }

        let json: Value = resp.json().await?;
        let item = json["items"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| anyhow!("Channel {} not found", channel_id))?;

        let snippet = &item["snippet"];
        let stats = &item["statistics"];
        let content_details = &item["contentDetails"];
        let branding = &item["brandingSettings"];

        let title = snippet["title"].as_str().unwrap_or("YouTube Channel").to_string();
        let description = snippet["description"].as_str().unwrap_or_default().to_string();
        let avatar_url = snippet["thumbnails"]["high"]["url"]
            .as_str()
            .or_else(|| snippet["thumbnails"]["default"]["url"].as_str())
            .unwrap_or_default()
            .to_string();
        let banner_url = branding["image"]["bannerExternalUrl"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let sub_count = stats["subscriberCount"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let video_count = stats["videoCount"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let uploads_playlist_id = content_details["relatedPlaylists"]["uploads"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let subscriber_text = if sub_count >= 1_000_000 {
            format!("{:.1}M subscribers", sub_count as f64 / 1_000_000.0)
        } else if sub_count >= 1_000 {
            format!("{:.1}K subscribers", sub_count as f64 / 1_000.0)
        } else if sub_count > 0 {
            format!("{} subscribers", sub_count)
        } else {
            "Subscribers hidden".to_string()
        };

        Ok(rusttube_core::models::ChannelDetail {
            item: ChannelItem {
                id: channel_id.to_string(),
                title,
                description,
                subscriber_count: sub_count,
                avatar_url,
            },
            uploads_playlist_id,
            banner_url,
            subscriber_text,
            video_count,
        })
    }

    pub async fn get_personalized_feed(&self, access_token: &str) -> Result<Vec<VideoItem>> {
        let (subs, _) = self.get_subscriptions(access_token, None).await?;
        let mut feed_videos = Vec::new();

        // Fetch recent uploads for top subscribed channels
        for ch in subs.iter().take(8) {
            if let Ok(ch_detail) = self.get_channel_detail(Some(access_token), &ch.id).await {
                if !ch_detail.uploads_playlist_id.is_empty() {
                    if let Ok(items) = self.get_playlist_items(access_token, &ch_detail.uploads_playlist_id).await {
                        feed_videos.extend(items);
                    }
                }
            }
        }

        // Deduplicate and sort by publication time if available
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for item in feed_videos {
            if !item.id.is_empty() && seen.insert(item.id.clone()) {
                result.push(item);
            }
        }

        Ok(result)
    }

    pub async fn update_playlist(
        &self,
        access_token: &str,
        playlist_id: &str,
        title: &str,
        description: &str,
        is_private: bool,
    ) -> Result<()> {
        let url = format!("{}/playlists?part=snippet,status", self.base_url);
        let privacy_status = if is_private { "private" } else { "public" };
        let body = serde_json::json!({
            "id": playlist_id,
            "snippet": {
                "title": title,
                "description": description
            },
            "status": {
                "privacyStatus": privacy_status
            }
        });

        let resp = self
            .client
            .put(&url)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to update playlist: {}", resp.status()));
        }
        Ok(())
    }

    pub async fn delete_playlist(&self, access_token: &str, playlist_id: &str) -> Result<()> {
        let url = format!("{}/playlists?id={}", self.base_url, playlist_id);
        let resp = self
            .client
            .delete(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Failed to delete playlist: {}", resp.status()));
        }
        Ok(())
    }
}
