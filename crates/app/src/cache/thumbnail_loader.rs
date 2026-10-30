use egui::{ColorImage, Context, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::warn;

#[derive(Clone)]
pub struct ThumbnailManager {
    textures: Arc<Mutex<HashMap<String, TextureHandle>>>,
    pending: Arc<Mutex<HashMap<String, bool>>>,
    tx: mpsc::Sender<String>,
}

impl ThumbnailManager {
    pub fn new(ctx: Context) -> Self {
        let textures = Arc::new(Mutex::new(HashMap::new()));
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (tx, mut rx) = mpsc::channel::<String>(128);

        let textures_clone = textures.clone();
        let pending_clone = pending.clone();

        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default();

            while let Some(url) = rx.recv().await {
                if url.is_empty() {
                    continue;
                }

                let fetch_client = client.clone();
                let url_str = url.clone();
                let ctx_ctx = ctx.clone();
                let tex_map = textures_clone.clone();
                let pend_map = pending_clone.clone();

                tokio::spawn(async move {
                    if let Ok(resp) = fetch_client.get(&url_str).send().await {
                        if let Ok(bytes) = resp.bytes().await {
                            if let Ok(img) = image::load_from_memory(&bytes) {
                                let size = [img.width() as usize, img.height() as usize];
                                let rgba = img.to_rgba8();
                                let pixels = rgba.into_raw();
                                let color_img = ColorImage::from_rgba_unmultiplied(size, &pixels);

                                let handle = ctx_ctx.load_texture(&url_str, color_img, TextureOptions::LINEAR);
                                tex_map.lock().unwrap().insert(url_str.clone(), handle);
                                ctx_ctx.request_repaint();
                            }
                        }
                    }
                    pend_map.lock().unwrap().remove(&url_str);
                });
            }
        });

        Self {
            textures,
            pending,
            tx,
        }
    }

    pub fn get_or_load(&self, url: &str) -> Option<TextureHandle> {
        if url.trim().is_empty() {
            return None;
        }

        let textures = self.textures.lock().unwrap();
        if let Some(tex) = textures.get(url) {
            return Some(tex.clone());
        }

        let mut pending = self.pending.lock().unwrap();
        if !pending.contains_key(url) {
            pending.insert(url.to_string(), true);
            let _ = self.tx.try_send(url.to_string());
        }

        None
    }
}
