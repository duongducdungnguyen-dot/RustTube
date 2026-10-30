use crate::mpv::{MpvConfig, MpvInstance};
use anyhow::Result;

pub struct PlayerController {
    mpv: MpvInstance,
}

impl PlayerController {
    pub fn new(hwdec: String, volume: u8, speed: f64) -> Result<Self> {
        let mpv = MpvInstance::new(MpvConfig {
            hwdec,
            volume,
            speed,
        })?;
        Ok(Self { mpv })
    }

    pub fn play_stream(
        &mut self,
        video_url: &str,
        audio_url: Option<&str>,
        resume_from_seconds: Option<u64>,
    ) -> Result<()> {
        self.mpv
            .load_dual_stream(video_url, audio_url, resume_from_seconds)
    }

    pub fn toggle_pause(&mut self) {
        self.mpv.is_paused = !self.mpv.is_paused;
    }

    pub fn seek_relative(&mut self, delta_seconds: i64) {
        let next = (self.mpv.current_position_seconds as i64 + delta_seconds).max(0) as u64;
        self.mpv.current_position_seconds = next;
    }
}
