use crate::database::Database;
use anyhow::Result;
use rusttube_core::models::{ChannelItem, PlaylistItem, VideoItem};
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct SyncRepository {
    db: Database,
}

impl SyncRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn now_ts(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    // --- Subscriptions ---
    pub fn get_subscriptions(&self) -> Result<Vec<ChannelItem>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT channel_id, title, description, thumbnail_url
             FROM cached_subscriptions
             ORDER BY title ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ChannelItem {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2).unwrap_or_default(),
                subscriber_count: 0,
                avatar_url: row.get(3).unwrap_or_default(),
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn save_subscriptions(&self, channels: &[ChannelItem]) -> Result<()> {
        let conn = self.db.conn();
        let now = self.now_ts();

        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM cached_subscriptions", [])?;
        for ch in channels {
            tx.execute(
                "INSERT INTO cached_subscriptions (channel_id, title, description, thumbnail_url, synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![ch.id, ch.title, ch.description, ch.avatar_url, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // --- Playlists ---
    pub fn get_playlists(&self) -> Result<Vec<PlaylistItem>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT playlist_id, title, description, item_count, thumbnail_url
             FROM cached_playlists
             ORDER BY title ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(PlaylistItem {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2).unwrap_or_default(),
                video_count: row.get::<_, u32>(3).unwrap_or(0),
                thumbnail_url: row.get(4).unwrap_or_default(),
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn save_playlists(&self, playlists: &[PlaylistItem]) -> Result<()> {
        let conn = self.db.conn();
        let now = self.now_ts();

        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM cached_playlists", [])?;
        for pl in playlists {
            tx.execute(
                "INSERT INTO cached_playlists (playlist_id, title, description, item_count, thumbnail_url, synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![pl.id, pl.title, pl.description, pl.video_count, pl.thumbnail_url, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // --- Playlist Items ---
    pub fn get_playlist_items(&self, playlist_id: &str) -> Result<Vec<VideoItem>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT video_id, title, channel_name, thumbnail_url
             FROM cached_playlist_items
             WHERE playlist_id = ?1
             ORDER BY position ASC",
        )?;

        let rows = stmt.query_map(params![playlist_id], |row| {
            Ok(VideoItem {
                id: row.get(0)?,
                title: row.get(1)?,
                channel_name: row.get(2).unwrap_or_default(),
                channel_id: String::new(),
                duration_seconds: 0,
                duration_string: "--:--".to_string(),
                view_count: 0,
                thumbnail_url: row.get(3).unwrap_or_default(),
                published_time: String::new(),
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn save_playlist_items(&self, playlist_id: &str, items: &[VideoItem]) -> Result<()> {
        let conn = self.db.conn();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM cached_playlist_items WHERE playlist_id = ?1", params![playlist_id])?;
        for (idx, item) in items.iter().enumerate() {
            let item_key = format!("{}_{}", playlist_id, item.id);
            tx.execute(
                "INSERT INTO cached_playlist_items (id, playlist_id, video_id, title, channel_name, thumbnail_url, position)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![item_key, playlist_id, item.id, item.title, item.channel_name, item.thumbnail_url, idx as i32],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // --- Liked Videos ---
    pub fn get_liked_videos(&self) -> Result<Vec<VideoItem>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT video_id, title, channel_name, channel_id, thumbnail_url, view_count
             FROM cached_liked_videos
             ORDER BY synced_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(VideoItem {
                id: row.get(0)?,
                title: row.get(1)?,
                channel_name: row.get(2).unwrap_or_default(),
                channel_id: row.get(3).unwrap_or_default(),
                duration_seconds: 0,
                duration_string: "--:--".to_string(),
                view_count: row.get::<_, u64>(5).unwrap_or(0),
                thumbnail_url: row.get(4).unwrap_or_default(),
                published_time: String::new(),
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn save_liked_videos(&self, videos: &[VideoItem]) -> Result<()> {
        let conn = self.db.conn();
        let now = self.now_ts();

        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM cached_liked_videos", [])?;
        for v in videos {
            tx.execute(
                "INSERT INTO cached_liked_videos (video_id, title, channel_name, channel_id, thumbnail_url, view_count, synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![v.id, v.title, v.channel_name, v.channel_id, v.thumbnail_url, v.view_count as i64, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // --- Metadata / Last Sync ---
    pub fn set_last_synced(&self, key: &str) -> Result<()> {
        let conn = self.db.conn();
        let now = self.now_ts();
        conn.execute(
            "INSERT OR REPLACE INTO sync_metadata (key_name, last_synced_at) VALUES (?1, ?2)",
            params![key, now],
        )?;
        Ok(())
    }

    pub fn get_last_synced(&self, key: &str) -> Option<i64> {
        let conn = self.db.conn();
        conn.query_row(
            "SELECT last_synced_at FROM sync_metadata WHERE key_name = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok()
    }
}
