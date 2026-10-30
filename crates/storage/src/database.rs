use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub const CURRENT_SCHEMA_VERSION: i32 = 3;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("Failed to open SQLite database at {}", path.as_ref().display()))?;
        Self::configure_and_migrate(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure_and_migrate(conn)
    }

    fn configure_and_migrate(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.run_migrations()?;
        Ok(db)
    }

    pub fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let guard = self
            .conn
            .lock()
            .map_err(|_| anyhow::anyhow!("Database mutex poisoned"))?;
        f(&guard)
    }

    /// Runs backward-compatible incremental migrations without ever dropping existing tables or rows.
    pub fn run_migrations(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at INTEGER NOT NULL
                );",
            )?;

            let current_version: i32 = conn
                .query_row(
                    "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            if current_version < 1 {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS watch_history (
                        video_id TEXT PRIMARY KEY,
                        title TEXT NOT NULL,
                        channel_name TEXT NOT NULL,
                        channel_id TEXT NOT NULL DEFAULT '',
                        duration_seconds INTEGER NOT NULL DEFAULT 0,
                        duration_string TEXT NOT NULL DEFAULT '',
                        view_count INTEGER NOT NULL DEFAULT 0,
                        views_text TEXT NOT NULL DEFAULT '',
                        published_at TEXT NOT NULL DEFAULT '',
                        published_timestamp INTEGER NOT NULL DEFAULT 0,
                        thumbnail_url TEXT NOT NULL DEFAULT '',
                        description TEXT NOT NULL DEFAULT '',
                        watched_at INTEGER NOT NULL,
                        last_position_seconds INTEGER NOT NULL DEFAULT 0
                    );
                    CREATE INDEX IF NOT EXISTS idx_history_watched_at ON watch_history(watched_at DESC);

                    CREATE TABLE IF NOT EXISTS watch_later (
                        video_id TEXT PRIMARY KEY,
                        title TEXT NOT NULL,
                        channel_name TEXT NOT NULL,
                        channel_id TEXT NOT NULL DEFAULT '',
                        duration_seconds INTEGER NOT NULL DEFAULT 0,
                        duration_string TEXT NOT NULL DEFAULT '',
                        view_count INTEGER NOT NULL DEFAULT 0,
                        views_text TEXT NOT NULL DEFAULT '',
                        published_at TEXT NOT NULL DEFAULT '',
                        published_timestamp INTEGER NOT NULL DEFAULT 0,
                        thumbnail_url TEXT NOT NULL DEFAULT '',
                        description TEXT NOT NULL DEFAULT '',
                        added_at INTEGER NOT NULL,
                        source TEXT NOT NULL DEFAULT 'local'
                    );
                    CREATE INDEX IF NOT EXISTS idx_watch_later_added_at ON watch_later(added_at DESC);",
                )?;
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (1, strftime('%s', 'now'))",
                    [],
                )?;
            }

            if current_version < 2 {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS cached_subscriptions (
                        channel_id TEXT PRIMARY KEY,
                        subscription_id TEXT,
                        title TEXT NOT NULL,
                        description TEXT NOT NULL DEFAULT '',
                        custom_url TEXT,
                        thumbnail_url TEXT NOT NULL DEFAULT '',
                        subscriber_count TEXT,
                        video_count INTEGER,
                        uploads_playlist_id TEXT,
                        synced_at INTEGER NOT NULL
                    );
                    CREATE INDEX IF NOT EXISTS idx_subs_title ON cached_subscriptions(title COLLATE NOCASE);

                    CREATE TABLE IF NOT EXISTS cached_playlists (
                        playlist_id TEXT PRIMARY KEY,
                        title TEXT NOT NULL,
                        description TEXT NOT NULL DEFAULT '',
                        item_count INTEGER NOT NULL DEFAULT 0,
                        thumbnail_url TEXT NOT NULL DEFAULT '',
                        privacy_status TEXT NOT NULL DEFAULT 'private',
                        is_owned INTEGER NOT NULL DEFAULT 1,
                        synced_at INTEGER NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS cached_playlist_items (
                        playlist_item_id TEXT PRIMARY KEY,
                        playlist_id TEXT NOT NULL,
                        video_id TEXT NOT NULL,
                        title TEXT NOT NULL,
                        channel_name TEXT NOT NULL DEFAULT '',
                        channel_id TEXT NOT NULL DEFAULT '',
                        duration_seconds INTEGER NOT NULL DEFAULT 0,
                        duration_string TEXT NOT NULL DEFAULT '',
                        thumbnail_url TEXT NOT NULL DEFAULT '',
                        position INTEGER NOT NULL DEFAULT 0,
                        availability_status TEXT NOT NULL DEFAULT 'Available',
                        FOREIGN KEY(playlist_id) REFERENCES cached_playlists(playlist_id) ON DELETE CASCADE
                    );
                    CREATE INDEX IF NOT EXISTS idx_playlist_items_pid ON cached_playlist_items(playlist_id, position ASC);

                    CREATE TABLE IF NOT EXISTS cached_liked_videos (
                        video_id TEXT PRIMARY KEY,
                        title TEXT NOT NULL,
                        channel_name TEXT NOT NULL,
                        channel_id TEXT NOT NULL DEFAULT '',
                        duration_seconds INTEGER NOT NULL DEFAULT 0,
                        duration_string TEXT NOT NULL DEFAULT '',
                        view_count INTEGER NOT NULL DEFAULT 0,
                        views_text TEXT NOT NULL DEFAULT '',
                        published_at TEXT NOT NULL DEFAULT '',
                        published_timestamp INTEGER NOT NULL DEFAULT 0,
                        thumbnail_url TEXT NOT NULL DEFAULT '',
                        description TEXT NOT NULL DEFAULT '',
                        synced_at INTEGER NOT NULL
                    );

                    CREATE TABLE IF NOT EXISTS sync_metadata (
                        key TEXT PRIMARY KEY,
                        last_synced_at INTEGER NOT NULL,
                        status_json TEXT NOT NULL DEFAULT '{}'
                    );",
                )?;
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (2, strftime('%s', 'now'))",
                    [],
                )?;
            }

            if current_version < 3 {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS cached_home_feed (
                        category TEXT NOT NULL,
                        video_id TEXT NOT NULL,
                        title TEXT NOT NULL,
                        channel_name TEXT NOT NULL,
                        channel_id TEXT NOT NULL DEFAULT '',
                        duration_seconds INTEGER NOT NULL DEFAULT 0,
                        duration_string TEXT NOT NULL DEFAULT '',
                        view_count INTEGER NOT NULL DEFAULT 0,
                        views_text TEXT NOT NULL DEFAULT '',
                        published_at TEXT NOT NULL DEFAULT '',
                        published_timestamp INTEGER NOT NULL DEFAULT 0,
                        thumbnail_url TEXT NOT NULL DEFAULT '',
                        description TEXT NOT NULL DEFAULT '',
                        source_tag TEXT,
                        cached_at INTEGER NOT NULL,
                        PRIMARY KEY (category, video_id)
                    );",
                )?;
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (3, strftime('%s', 'now'))",
                    [],
                )?;
            }

            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_migrations_preserve_existing_user_data() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("rusttube_test.db");

        {
            let db = Database::open(&db_path)?;
            db.with_conn(|conn| {
                conn.execute(
                    "INSERT INTO watch_later (video_id, title, channel_name, added_at, source)
                     VALUES ('dQw4w9WgXcQ', 'Test Video', 'Test Channel', 1700000000, 'local')",
                    [],
                )?;
                Ok(())
            })?;
        }

        // Re-open simulating an application update running migrations again
        let db_reopened = Database::open(&db_path)?;
        db_reopened.run_migrations()?;

        let count: i64 = db_reopened.with_conn(|conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM watch_later", [], |r| r.get(0))?)
        })?;
        assert_eq!(count, 1);
        Ok(())
    }
}
