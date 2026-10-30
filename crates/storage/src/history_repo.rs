use crate::database::Database;
use anyhow::Result;
use chrono::Utc;
use rusqlite::params;
use rusttube_core::{VideoItem, WatchHistoryEntry};

#[derive(Clone)]
pub struct HistoryRepo {
    db: Database,
}

impl HistoryRepo {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn record_playback(&self, video: &VideoItem, last_position_seconds: u64) -> Result<()> {
        let now = Utc::now().timestamp();
        self.db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO watch_history (
                    video_id, title, channel_name, channel_id, duration_seconds, duration_string,
                    view_count, views_text, published_at, published_timestamp, thumbnail_url,
                    description, watched_at, last_position_seconds
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                ON CONFLICT(video_id) DO UPDATE SET
                    title = excluded.title,
                    channel_name = excluded.channel_name,
                    watched_at = excluded.watched_at,
                    last_position_seconds = CASE
                        WHEN excluded.last_position_seconds > 0 THEN excluded.last_position_seconds
                        ELSE watch_history.last_position_seconds
                    END",
                params![
                    video.id,
                    video.title,
                    video.channel_name,
                    video.channel_id,
                    video.duration_seconds as i64,
                    video.duration_string,
                    video.view_count as i64,
                    video.views_text,
                    video.published_at,
                    video.published_timestamp,
                    video.thumbnail_url,
                    video.description,
                    now,
                    last_position_seconds as i64
                ],
            )?;
            Ok(())
        })
    }

    pub fn list(&self, search_query: Option<&str>, limit: usize) -> Result<Vec<WatchHistoryEntry>> {
        let pattern = search_query
            .map(|q| format!("%{}%", q.trim().to_lowercase()))
            .unwrap_or_else(|| "%".to_string());

        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT video_id, title, channel_name, channel_id, duration_seconds, duration_string,
                        view_count, views_text, published_at, published_timestamp, thumbnail_url,
                        description, watched_at, last_position_seconds
                 FROM watch_history
                 WHERE LOWER(title) LIKE ?1 OR LOWER(channel_name) LIKE ?1
                 ORDER BY watched_at DESC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![pattern, limit as i64], |row| {
                let last_pos = row.get::<_, i64>(13)?.max(0) as u64;
                Ok(WatchHistoryEntry {
                    video: VideoItem {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        channel_name: row.get(2)?,
                        channel_id: row.get(3)?,
                        duration_seconds: row.get::<_, i64>(4)?.max(0) as u64,
                        duration_string: row.get(5)?,
                        view_count: row.get::<_, i64>(6)?.max(0) as u64,
                        views_text: row.get(7)?,
                        published_at: row.get(8)?,
                        published_timestamp: row.get(9)?,
                        thumbnail_url: row.get(10)?,
                        description: row.get(11)?,
                        resume_position_seconds: if last_pos > 0 { Some(last_pos) } else { None },
                        source_tag: Some("Local History".to_string()),
                    },
                    watched_at: row.get(12)?,
                    last_position_seconds: last_pos,
                })
            })?;

            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    pub fn remove_entry(&self, video_id: &str) -> Result<bool> {
        self.db.with_conn(|conn| {
            let affected = conn.execute(
                "DELETE FROM watch_history WHERE video_id = ?1",
                params![video_id],
            )?;
            Ok(affected > 0)
        })
    }

    pub fn clear_all(&self) -> Result<()> {
        self.db.with_conn(|conn| {
            conn.execute("DELETE FROM watch_history", [])?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_video(id: &str, title: &str) -> VideoItem {
        VideoItem {
            id: id.to_string(),
            title: title.to_string(),
            channel_name: "Rust Systems".to_string(),
            channel_id: "UC123".to_string(),
            duration_seconds: 600,
            duration_string: "10:00".to_string(),
            view_count: 1200,
            views_text: "1.2K views".to_string(),
            published_at: "2026-10-01".to_string(),
            published_timestamp: 1790000000,
            thumbnail_url: "https://i.ytimg.com/vi/test/hqdefault.jpg".to_string(),
            description: "Desc".to_string(),
            resume_position_seconds: None,
            source_tag: None,
        }
    }

    #[test]
    fn test_history_persistence_search_and_remove() -> Result<()> {
        let db = Database::open_in_memory()?;
        let repo = HistoryRepo::new(db);

        repo.record_playback(&sample_video("vid00000001", "Async Tokio Deep Dive"), 145)?;
        repo.record_playback(&sample_video("vid00000002", "egui Immediate Mode UI"), 0)?;

        let all = repo.list(None, 50)?;
        assert_eq!(all.len(), 2);

        let filtered = repo.list(Some("tokio"), 50)?;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].last_position_seconds, 145);

        assert!(repo.remove_entry("vid00000001")?);
        assert_eq!(repo.list(None, 50)?.len(), 1);

        repo.clear_all()?;
        assert!(repo.list(None, 50)?.is_empty());
        Ok(())
    }
}
