use anyhow::Result;
use reqwest::Client;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ThumbnailCache {
    cache_dir: PathBuf,
    max_disk_bytes: u64,
    http_client: Client,
    in_memory: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl ThumbnailCache {
    pub fn new<P: AsRef<Path>>(cache_dir: P, max_disk_mb: u64) -> Self {
        let path = cache_dir.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&path);
        Self {
            cache_dir: path,
            max_disk_bytes: max_disk_mb * 1024 * 1024,
            http_client: Client::builder().build().unwrap_or_default(),
            in_memory: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn url_to_filename(url: &str) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(url, &mut hasher);
        let hash = std::hash::Hasher::finish(&hasher);
        format!("{:016x}.jpg", hash)
    }

    pub async fn get_or_fetch(&self, url: &str) -> Result<Vec<u8>> {
        if url.is_empty() {
            return Ok(Vec::new());
        }

        {
            let mem = self.in_memory.lock().await;
            if let Some(data) = mem.get(url) {
                return Ok(data.clone());
            }
        }

        let filename = Self::url_to_filename(url);
        let file_path = self.cache_dir.join(&filename);

        if file_path.exists() {
            if let Ok(bytes) = tokio::fs::read(&file_path).await {
                let mut mem = self.in_memory.lock().await;
                if mem.len() > 100 {
                    mem.clear();
                }
                mem.insert(url.to_string(), bytes.clone());
                return Ok(bytes);
            }
        }

        let resp = self.http_client.get(url).send().await?;
        let bytes = resp.bytes().await?.to_vec();

        let _ = tokio::fs::write(&file_path, &bytes).await;

        let mut mem = self.in_memory.lock().await;
        if mem.len() > 100 {
            mem.clear();
        }
        mem.insert(url.to_string(), bytes.clone());

        self.cleanup_if_needed().await;

        Ok(bytes)
    }

    async fn cleanup_if_needed(&self) {
        let dir = self.cache_dir.clone();
        let max_bytes = self.max_disk_bytes;
        tokio::spawn(async move {
            let mut total_size = 0u64;
            let mut entries = Vec::new();
            if let Ok(mut read_dir) = tokio::fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = read_dir.next_entry().await {
                    if let Ok(meta) = entry.metadata().await {
                        total_size += meta.len();
                        entries.push((entry.path(), meta.modified().ok(), meta.len()));
                    }
                }
            }

            if total_size > max_bytes {
                entries.sort_by_key(|e| e.1);
                for (path, _, size) in entries {
                    if total_size <= max_bytes {
                        break;
                    }
                    if tokio::fs::remove_file(&path).await.is_ok() {
                        total_size = total_size.saturating_sub(size);
                    }
                }
            }
        });
    }
}
