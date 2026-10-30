use crate::mpv::{MpvInstance, MpvLibrary};
use anyhow::Result;
use rusttube_core::events::UiCommand;
use rusttube_core::models::{PlaybackStatus, StreamInfo};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub struct PlayerController {
    mpv: Option<Arc<Mutex<MpvInstance>>>,
    status: Arc<Mutex<PlaybackStatus>>,
    hwdec: String,
    window_id: Option<isize>,
}

impl PlayerController {
    pub fn new(hwdec: &str) -> Self {
        Self {
            mpv: None,
            status: Arc::new(Mutex::new(PlaybackStatus::default())),
            hwdec: hwdec.to_string(),
            window_id: None,
        }
    }

    pub fn set_window_id(&mut self, wid: isize) {
        self.window_id = Some(wid);
        if let Some(mpv) = &self.mpv {
            if let Ok(inst) = mpv.try_lock() {
                let _ = inst.set_property("wid", &wid.to_string());
            }
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        let lib = MpvLibrary::load()?;
        match MpvInstance::new(lib.clone(), &self.hwdec) {
            Ok(inst) => {
                if let Some(wid) = self.window_id {
                    let _ = inst.set_option("wid", &wid.to_string());
                }
                self.mpv = Some(Arc::new(Mutex::new(inst)));
                info!("libmpv initialized with hwdec={}", self.hwdec);
                Ok(())
            }
            Err(e) => {
                warn!("Failed hwdec={}, falling back to software decoding 'no': {}", self.hwdec, e);
                let inst = MpvInstance::new(lib, "no")?;
                if let Some(wid) = self.window_id {
                    let _ = inst.set_option("wid", &wid.to_string());
                }
                self.mpv = Some(Arc::new(Mutex::new(inst)));
                Ok(())
            }
        }
    }

    pub async fn play_stream(&self, stream: &StreamInfo, video_id: &str, title: &str) -> Result<()> {
        if let Some(mpv) = &self.mpv {
            let inst = mpv.lock().await;

            if let Some(ref audio_url) = stream.audio_url {
                inst.command(&["loadfile", &stream.video_url, "replace", &format!("audio-file={}", audio_url)])?;
            } else {
                inst.load_file(&stream.video_url)?;
            }

            inst.play()?;

            let mut status = self.status.lock().await;
            status.video_id = video_id.to_string();
            status.title = title.to_string();
            status.is_playing = true;
            status.is_buffering = false;
            status.quality = stream.resolution.clone();
        }
        Ok(())
    }

    pub async fn handle_command(&self, cmd: UiCommand) -> Result<()> {
        let mpv = match &self.mpv {
            Some(m) => m.clone(),
            None => return Ok(()),
        };

        let inst = mpv.lock().await;
        match cmd {
            UiCommand::TogglePlayPause => {
                inst.toggle_pause()?;
            }
            UiCommand::SeekRelative(seconds) => {
                inst.seek_relative(seconds)?;
            }
            UiCommand::SeekAbsolute(seconds) => {
                inst.seek_absolute(seconds)?;
            }
            UiCommand::SetVolume(vol) => {
                inst.set_volume(vol)?;
            }
            UiCommand::SetSpeed(speed) => {
                inst.set_speed(speed)?;
            }
            UiCommand::ToggleFullscreen => {
                let is_fs = inst.get_property("fullscreen").map(|s| s == "yes").unwrap_or(false);
                inst.set_property("fullscreen", if is_fs { "no" } else { "yes" })?;
            }
            UiCommand::ToggleSubtitles => {
                let sub = inst.get_property("sub-visibility").map(|s| s == "yes").unwrap_or(false);
                inst.set_property("sub-visibility", if sub { "no" } else { "yes" })?;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn seek_absolute(&self, seconds: f64) -> Result<()> {
        if let Some(mpv) = &self.mpv {
            let inst = mpv.lock().await;
            inst.seek_absolute(seconds)?;
        }
        Ok(())
    }

    pub async fn poll_status(&self) -> PlaybackStatus {
        let mut status = self.status.lock().await.clone();
        if let Some(mpv) = &self.mpv {
            let inst = mpv.lock().await;
            if let Some(pos) = inst.get_property("time-pos") {
                if let Ok(p) = pos.parse::<f64>() {
                    status.position_seconds = p;
                }
            }
            if let Some(dur) = inst.get_property("duration") {
                if let Ok(d) = dur.parse::<f64>() {
                    status.duration_seconds = d;
                }
            }
            if let Some(paused) = inst.get_property("pause") {
                status.is_playing = paused != "yes";
            }
            if let Some(vol) = inst.get_property("volume") {
                if let Ok(v) = vol.parse::<f64>() {
                    status.volume = v as u32;
                }
            }
            if let Some(speed) = inst.get_property("speed") {
                if let Ok(s) = speed.parse::<f32>() {
                    status.speed = s;
                }
            }
        }
        status
    }

    pub async fn stop(&self) {
        if let Some(mpv) = &self.mpv {
            let inst = mpv.lock().await;
            let _ = inst.stop();
        }
        let mut status = self.status.lock().await;
        status.is_playing = false;
        status.position_seconds = 0.0;
    }
}
