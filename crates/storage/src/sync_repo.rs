use crate::database::Database;
use anyhow::Result;
use chrono::Utc;
use rusqlite::params;
use rusttube_core::{
    ChannelItem, PlaylistItem, PlaylistPrivacy, PlaylistVideoItem, VideoAvailability, VideoItem,
};

#[derive(Clone)]
pub struct SyncRepo {
    db: Database,
}

impl SyncRepo {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn replace_subscriptions(&self, channels: &[ChannelItem]) -> Result<()> {
        let now = Utc::now().timestamp();
        self.db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM cached_subscriptions", [])?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO cached_subscriptions (
                        channel_id, subscription_id, title, description, custom_url,
                        thumbnail_url, subscriber_count, video_count, uploads_playlist_id, synced_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                )?;
                for ch in channels {
                    stmt.execute(params![
                        ch.channel_id,
                        ch.subscription_id,
                        ch.title,
                        ch.description,
                        ch.custom_url,
                        ch.thumbnail_url,
                        ch.subscriber_count,
                        ch.video_count.map(|v| v as i64),
                        ch.uploads_playlist_id,
                        now
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn remove_subscription(&self, channel_id: &str) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                "DELETE FROM cached_subscriptions WHERE channel_id = ?1",
                params![channel_id],
            )?;
            Ok(())
        })
    }

    pub fn get_subscriptions(&self, filter_query: Option<&str>) -> Result<Vec<ChannelItem>> {
        let pattern = filter_query
            .map(|q| format!("%{}%", q.trim().to_lowercase()))
            .unwrap_or_else(|| "%".to_string());

        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT channel_id, subscription_id, title, description, custom_url,
                        thumbnail_url, subscriber_count, video_count, uploads_playlist_id, synced_at
                 FROM cached_subscriptions
                 WHERE LOWER(title) LIKE ?1 OR LOWER(description) LIKE ?1
                 ORDER BY title COLLATE NOCASE ASC",
            )?;
            let rows = stmt.query_map(params![pattern], |r| {
                Ok(ChannelItem {
                    channel_id: r.get(0)?,
                    subscription_id: r.get(1)?,
                    title: r.get(2)?,
                    description: r.get(3)?,
                    custom_url: r.get(4)?,
                    thumbnail_url: r.get(5)?,
                    subscriber_count: r.get(6)?,
                    video_count: r.get::<_, Option<i64>>(7)?.map(|v| v.max(0) as u64),
                    uploads_playlist_id: r.get(8)?,
                    synced_at: r.get(9)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn replace_playlists(&self, playlists: &[PlaylistItem]) -> Result<()> {
        let now = Utc::now().timestamp();
        self.db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM cached_playlists", [])?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO cached_playlists (
                        playlist_id, title, description, item_count, thumbnail_url,
                        privacy_status, is_owned, synced_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )?;
                for pl in playlists {
                    stmt.execute(params![
                        pl.playlist_id,
                        pl.title,
                        pl.description,
                        pl.item_count as i64,
                        pl.thumbnail_url,
                        pl.privacy_status.as_api_str(),
                        if pl.is_owned { 1 } else { 0 },
                        now
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn get_playlists(&self) -> Result<Vec<PlaylistItem>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT playlist_id, title, description, item_count, thumbnail_url,
                        privacy_status, is_owned, synced_at
                 FROM cached_playlists
                 ORDER BY title COLLATE NOCASE ASC",
            )?;
            let rows = stmt.query_map([], |r| {
                let priv_str: String = r.get(5)?;
                Ok(PlaylistItem {
                    playlist_id: r.get(0)?,
                    title: r.get(1)?,
                    description: r.get(2)?,
                    item_count: r.get::<_, i64>(3)?.max(0) as u32,
                    thumbnail_url: r.get(4)?,
                    privacy_status: PlaylistPrivacy::from_api_str(&priv_str),
                    is_owned: r.get::<_, i64>(6)? != 0,
                    synced_at: r.get(7)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn cache_playlist_items(
        &self,
        playlist_id: &str,
        items: &[PlaylistVideoItem],
    ) -> Result<()> {
        self.db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "DELETE FROM cached_playlist_items WHERE playlist_id = ?1",
                params![playlist_id],
            )?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO cached_playlist_items (
                        playlist_item_id, playlist_id, video_id, title, channel_name, channel_id,
                        duration_seconds, duration_string, thumbnail_url, position, availability_status
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                )?;
                for item in items {
                    let avail = match item.availability_status {
                        VideoAvailability::Available => "Available",
                        VideoAvailability::Private => "Private",
                        VideoAvailability::Deleted => "Deleted",
                        VideoAvailability::Unavailable => "Unavailable",
                    };
                    stmt.execute(params![
                        item.playlist_item_id,
                        playlist_id,
                        item.video.id,
                        item.video.title,
                        item.video.channel_name,
                        item.video.channel_id,
                        item.video.duration_seconds as i64,
                        item.video.duration_string,
                        item.video.thumbnail_url,
                        item.position as i64,
                        avail
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn replace_liked_videos(&self, videos: &[VideoItem]) -> Result<()> {
        let now = Utc::now().timestamp();
        self.db.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM cached_liked_videos", [])?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO cached_liked_videos (
                        video_id, title, channel_name, channel_id, duration_seconds, duration_string,
                        view_count, views_text, published_at, published_timestamp, thumbnail_url,
                        description, synced_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                )?;
                for v in videos {
                    stmt.execute(params![
                        v.id,
                        v.title,
                        v.channel_name,
                        v.channel_id,
                        v.duration_seconds as i64,
                        v.duration_string,
                        v.view_count as i64,
                        v.views_text,
                        v.published_at,
                        v.published_timestamp,
                        v.thumbnail_url,
                        v.description,
                        now
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn get_liked_videos(&self) -> Result<Vec<VideoItem>> {
        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT video_id, title, channel_name, channel_id, duration_seconds, duration_string,
                        view_count, views_text, published_at, published_timestamp, thumbnail_url, description
                 FROM cached_liked_videos
                 ORDER BY synced_at DESC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(VideoItem {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    channel_name: r.get(2)?,
                    channel_id: r.get(3)?,
                    duration_seconds: r.get::<_, i64>(4)?.max(0) as u64,
                    duration_string: r.get(5)?,
                    view_count: r.get::<_, i64>(6)?.max(0) as u64,
                    views_text: r.get(7)?,
                    published_at: r.get(8)?,
                    published_timestamp: r.get(9)?,
                    thumbnail_url: r.get(10)?,
                    description: r.get(11)?,
                    resume_position_seconds: None,
                    source_tag: Some("Liked Videos".to_string()),
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn set_last_sync_timestamp(&self, key: &str, timestamp: i64) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO sync_metadata (key, last_synced_at, status_json)
                 VALUES (?1, ?2, '{}')
                 ON CONFLICT(key) DO UPDATE SET last_synced_at = excluded.last_synced_at",
                params![key, timestamp],
            )?;
            Ok(())
        })
    }

    pub fn get_last_sync_timestamp(&self, key: &str) -> Result<Option<i64>> {
        self.db.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT last_synced_at FROM sync_metadata WHERE key = ?1")?;
            let mut rows = stmt.query(params![key])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get(0)?))
            } else {
                Ok(None)
            }
        })
    }
}
