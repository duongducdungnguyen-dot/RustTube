use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub struct ThumbnailDiskCache {
    dir: PathBuf,
}

impl ThumbnailDiskCache {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        let dir = base_dir.join("thumbnails");
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    pub fn clear(&self) -> Result<()> {
        if self.dir.exists() {
            fs::remove_dir_all(&self.dir)?;
            fs::create_dir_all(&self.dir)?;
        }
        Ok(())
    }
}
