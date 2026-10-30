use crate::database::Database;
use anyhow::Result;
use rusttube_core::models::VideoItem;
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct HistoryRepository {
    db: Database,
    max_items: usize,
}

impl HistoryRepository {
    pub fn new(db: Database, max_items: usize) -> Self {
        Self { db, max_items }
    }

    pub fn get_all(&self) -> Result<Vec<VideoItem>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT video_id, title, channel_name, channel_id, duration_seconds, duration_string, view_count, thumbnail_url
             FROM watch_history
             ORDER BY watched_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![self.max_items as i64], |row| {
            Ok(VideoItem {
                id: row.get(0)?,
                title: row.get(1)?,
                channel_name: row.get(2).unwrap_or_default(),
                channel_id: row.get(3).unwrap_or_default(),
                duration_seconds: row.get::<_, u32>(4).unwrap_or(0),
                duration_string: row.get(5).unwrap_or_else(|_| "--:--".to_string()),
                view_count: row.get::<_, u64>(6).unwrap_or(0),
                thumbnail_url: row.get(7).unwrap_or_default(),
                published_time: String::new(),
            })
        })?;

        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    pub fn record(&self, item: &VideoItem) -> Result<()> {
        let conn = self.db.conn();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        conn.execute(
            "INSERT OR REPLACE INTO watch_history
             (video_id, title, channel_name, channel_id, duration_seconds, duration_string, view_count, thumbnail_url, watched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                item.id,
                item.title,
                item.channel_name,
                item.channel_id,
                item.duration_seconds,
                item.duration_string,
                item.view_count as i64,
                item.thumbnail_url,
                now
            ],
        )?;

        // Prune excessive old items
        conn.execute(
            "DELETE FROM watch_history
             WHERE video_id NOT IN (
                 SELECT video_id FROM watch_history ORDER BY watched_at DESC LIMIT ?1
             )",
            params![self.max_items as i64],
        )?;

        Ok(())
    }

    pub fn remove(&self, video_id: &str) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM watch_history WHERE video_id = ?1", params![video_id])?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        let conn = self.db.conn();
        conn.execute("DELETE FROM watch_history", [])?;
        Ok(())
    }
}
