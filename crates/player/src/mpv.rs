use anyhow::Result;

#[derive(Debug, Clone)]
pub struct MpvConfig {
    pub hwdec: String,
    pub volume: u8,
    pub speed: f64,
}

pub struct MpvInstance {
    pub config: MpvConfig,
    pub is_paused: bool,
    pub current_position_seconds: u64,
}

impl MpvInstance {
    pub fn new(config: MpvConfig) -> Result<Self> {
        Ok(Self {
            config,
            is_paused: false,
            current_position_seconds: 0,
        })
    }

    pub fn load_dual_stream(
        &mut self,
        _video_url: &str,
        _audio_url: Option<&str>,
        resume_from_seconds: Option<u64>,
    ) -> Result<()> {
        self.is_paused = false;
        self.current_position_seconds = resume_from_seconds.unwrap_or(0);
        Ok(())
    }
}
