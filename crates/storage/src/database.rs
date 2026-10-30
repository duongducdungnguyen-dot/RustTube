use anyhow::Result;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::info;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    db_path: PathBuf,
}

impl Database {
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let path = db_path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path)?;
        
        // WAL mode & pragmas
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;",
        )?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            db_path: path,
        };

        db.initialize_schema()?;
        info!("SQLite database initialized at {:?}", db.db_path);
        Ok(db)
    }

    fn initialize_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS watch_history (
                video_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                channel_name TEXT,
                channel_id TEXT,
                duration_seconds INTEGER DEFAULT 0,
                duration_string TEXT,
                view_count INTEGER DEFAULT 0,
                thumbnail_url TEXT,
                watched_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS watch_later (
                video_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                channel_name TEXT,
                channel_id TEXT,
                duration_seconds INTEGER DEFAULT 0,
                duration_string TEXT,
                view_count INTEGER DEFAULT 0,
                thumbnail_url TEXT,
                added_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS cached_subscriptions (
                channel_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                thumbnail_url TEXT,
                synced_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS cached_playlists (
                playlist_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                item_count INTEGER DEFAULT 0,
                thumbnail_url TEXT,
                synced_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS cached_playlist_items (
                id TEXT PRIMARY KEY,
                playlist_id TEXT NOT NULL,
                video_id TEXT NOT NULL,
                title TEXT,
                channel_name TEXT,
                thumbnail_url TEXT,
                position INTEGER DEFAULT 0,
                FOREIGN KEY(playlist_id) REFERENCES cached_playlists(playlist_id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS cached_liked_videos (
                video_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                channel_name TEXT,
                channel_id TEXT,
                thumbnail_url TEXT,
                view_count INTEGER DEFAULT 0,
                synced_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sync_metadata (
                key_name TEXT PRIMARY KEY,
                last_synced_at INTEGER NOT NULL,
                etag TEXT
            );",
        )?;

        Ok(())
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }
}
