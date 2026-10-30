use anyhow::Result;
use rusttube_core::models::VideoItem;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct WatchLaterStore {
    file_path: PathBuf,
    items: Arc<Mutex<Vec<VideoItem>>>,
}

impl WatchLaterStore {
    pub fn new<P: AsRef<Path>>(file_path: P) -> Self {
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
            items: Arc::new(Mutex::new(items)),
        }
    }

    pub async fn get_all(&self) -> Vec<VideoItem> {
        self.items.lock().await.clone()
    }

    pub async fn contains(&self, video_id: &str) -> bool {
        let lock = self.items.lock().await;
        lock.iter().any(|v| v.id == video_id)
    }

    pub async fn toggle(&self, item: VideoItem) -> Result<bool> {
        let mut lock = self.items.lock().await;
        let is_now_in;
        if let Some(idx) = lock.iter().position(|v| v.id == item.id) {
            lock.remove(idx);
            is_now_in = false;
        } else {
            lock.insert(0, item);
            is_now_in = true;
        }
        let data = serde_json::to_string_pretty(&*lock)?;
        tokio::fs::write(&self.file_path, data).await?;
        Ok(is_now_in)
    }

    pub async fn remove(&self, video_id: &str) -> Result<()> {
        let mut lock = self.items.lock().await;
        lock.retain(|v| v.id != video_id);
        let data = serde_json::to_string_pretty(&*lock)?;
        tokio::fs::write(&self.file_path, data).await?;
        Ok(())
    }
}
