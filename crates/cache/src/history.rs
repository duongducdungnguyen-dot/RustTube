use anyhow::Result;
use rusttube_core::models::VideoItem;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct HistoryStore {
    file_path: PathBuf,
    max_items: usize,
    items: Arc<Mutex<Vec<VideoItem>>>,
}

impl HistoryStore {
    pub fn new<P: AsRef<Path>>(file_path: P, max_items: usize) -> Self {
        let path = file_path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let items = if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<Vec<VideoItem>>(&s).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Self {
            file_path: path,
            max_items,
            items: Arc::new(Mutex::new(items)),
        }
    }

    pub async fn get_all(&self) -> Vec<VideoItem> {
        self.items.lock().await.clone()
    }

    pub async fn record(&self, item: VideoItem) -> Result<()> {
        let mut lock = self.items.lock().await;
        lock.retain(|v| v.id != item.id);
        lock.insert(0, item);
        if lock.len() > self.max_items {
            lock.truncate(self.max_items);
        }
        let data = serde_json::to_string_pretty(&*lock)?;
        tokio::fs::write(&self.file_path, data).await?;
        Ok(())
    }

    pub async fn clear(&self) -> Result<()> {
        let mut lock = self.items.lock().await;
        lock.clear();
        tokio::fs::write(&self.file_path, "[]").await?;
        Ok(())
    }
}
