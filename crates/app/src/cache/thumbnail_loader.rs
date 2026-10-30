use std::collections::HashSet;

#[derive(Default)]
pub struct ThumbnailLoader {
    requested: HashSet<String>,
}

impl ThumbnailLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_requested(&mut self, url: &str) -> bool {
        self.requested.insert(url.to_string())
    }

    pub fn clear(&mut self) {
        self.requested.clear();
    }
}
