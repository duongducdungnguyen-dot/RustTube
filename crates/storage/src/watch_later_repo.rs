use crate::database::Database;
use anyhow::Result;
use chrono::Utc;
use rusqlite::params;
use rusttube_core::{VideoItem, WatchLaterEntry, WatchLaterSource};

#[derive(Clone)]
pub struct WatchLaterRepo {
    db: Database,
}

impl WatchLaterRepo {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Toggles a video in the local RustTube Watch Later queue.
    /// Prevents duplicates via primary key and never overwrites local entries when importing cookie items.
    pub fn toggle_local(&self, video: &VideoItem) -> Result<bool> {
        let exists: bool = self.db.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM watch_later WHERE video_id = ?1 AND source = 'local'",
                params![video.id],
                |r| r.get(0),
            )?;
            Ok(count > 0)
        })?;

        if exists {
            self.remove(&video.id)?;
            Ok(false)
        } else {
            self.upsert(video, WatchLaterSource::LocalRustTube)?;
            Ok(true)
        }
    }

    pub fn upsert(&self, video: &VideoItem, source: WatchLaterSource) -> Result<()> {
        let now = Utc::now().timestamp();
        let source_str = match source {
            WatchLaterSource::LocalRustTube => "local",
            WatchLaterSource::ExperimentalYouTubeCookie => "yt_cookie",
        };

        self.db.with_conn(|conn| {
            // Never overwrite an existing 'local' entry with 'yt_cookie'
            conn.execute(
                "INSERT INTO watch_later (
                    video_id, title, channel_name, channel_id, duration_seconds, duration_string,
                    view_count, views_text, published_at, published_timestamp, thumbnail_url,
                    description, added_at, source
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                ON CONFLICT(video_id) DO UPDATE SET
                    title = excluded.title,
                    channel_name = excluded.channel_name,
                    source = CASE WHEN watch_later.source = 'local' THEN 'local' ELSE excluded.source END",
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
                    source_str
                ],
            )?;
            Ok(())
        })
    }

    pub fn remove(&self, video_id: &str) -> Result<bool> {
        self.db.with_conn(|conn| {
            let affected = conn.execute(
                "DELETE FROM watch_later WHERE video_id = ?1",
                params![video_id],
            )?;
            Ok(affected > 0)
        })
    }

    pub fn list_by_source(&self, source: WatchLaterSource) -> Result<Vec<WatchLaterEntry>> {
        let source_str = match source {
            WatchLaterSource::LocalRustTube => "local",
            WatchLaterSource::ExperimentalYouTubeCookie => "yt_cookie",
        };

        self.db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT video_id, title, channel_name, channel_id, duration_seconds, duration_string,
                        view_count, views_text, published_at, published_timestamp, thumbnail_url,
                        description, added_at
                 FROM watch_later
                 WHERE source = ?1
                 ORDER BY added_at DESC",
            )?;

            let rows = stmt.query_map(params![source_str], |row| {
                Ok(WatchLaterEntry {
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
                        resume_position_seconds: None,
                        source_tag: Some(source_str.to_string()),
                    },
                    added_at: row.get(12)?,
                    source,
                })
            })?;

            let mut list = Vec::new();
            for r in rows {
                list.push(r?);
            }
            Ok(list)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_video(id: &str) -> VideoItem {
        VideoItem {
            id: id.to_string(),
            title: format!("Video {}", id),
            channel_name: "Channel".to_string(),
            channel_id: "UC1".to_string(),
            duration_seconds: 120,
            duration_string: "2:00".to_string(),
            view_count: 100,
            views_text: "100 views".to_string(),
            published_at: "2026-10-10".to_string(),
            published_timestamp: 1791000000,
            thumbnail_url: String::new(),
            description: String::new(),
            resume_position_seconds: None,
            source_tag: None,
        }
    }

    #[test]
    fn test_local_watch_later_prevents_duplicates_and_cookie_overwrite() -> Result<()> {
        let db = Database::open_in_memory()?;
        let repo = WatchLaterRepo::new(db);
        let v = dummy_video("abc12345678");

        assert!(repo.toggle_local(&v)?);
        assert_eq!(repo.list_by_source(WatchLaterSource::LocalRustTube)?.len(), 1);

        // Attempting to import same video from experimental cookie must NOT overwrite local source
        repo.upsert(&v, WatchLaterSource::ExperimentalYouTubeCookie)?;
        assert_eq!(repo.list_by_source(WatchLaterSource::LocalRustTube)?.len(), 1);
        assert_eq!(
            repo.list_by_source(WatchLaterSource::ExperimentalYouTubeCookie)?
                .len(),
            0
        );

        // Toggling again removes it
        assert!(!repo.toggle_local(&v)?);
        assert_eq!(repo.list_by_source(WatchLaterSource::LocalRustTube)?.len(), 0);
        Ok(())
    }
}
