use anyhow::{anyhow, Result};
use reqwest::Client;
use rusttube_core::models::AppUpdateInfo;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tracing::info;

const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/duongducdungnguyen-dot/RustTube/releases/latest";

pub async fn check_for_updates(client: &Client) -> Result<Option<AppUpdateInfo>> {
    let current_ver = env!("CARGO_PKG_VERSION");

    let resp = client
        .get(GITHUB_RELEASES_URL)
        .header("User-Agent", "RustTube-Desktop-Client")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    let json: Value = resp.json().await?;
    let tag_name = json["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();

    if tag_name.is_empty() {
        return Ok(None);
    }

    let is_update_available = is_version_newer(current_ver, &tag_name);
    let release_notes = json["body"].as_str().unwrap_or("No release notes provided.").to_string();
    let published_at = json["published_at"].as_str().unwrap_or("Recent").to_string();

    let mut download_url = String::new();
    let mut sha256_checksum = None;

    if let Some(assets) = json["assets"].as_array() {
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or("").to_lowercase();
            if name.ends_with(".exe") || name.contains("windows") {
                if let Some(url) = asset["browser_download_url"].as_str() {
                    download_url = url.to_string();
                }
            }
            if name.ends_with(".sha256") {
                if let Some(url) = asset["browser_download_url"].as_str() {
                    sha256_checksum = Some(url.to_string());
                }
            }
        }
    }

    Ok(Some(AppUpdateInfo {
        current_version: current_ver.to_string(),
        latest_version: tag_name,
        is_update_available,
        release_notes,
        download_url,
        sha256_checksum,
        published_at,
    }))
}

pub async fn download_and_stage_update(
    client: &Client,
    download_url: &str,
) -> Result<PathBuf> {
    if download_url.trim().is_empty() {
        return Err(anyhow!("No valid Windows executable download URL found in release asset."));
    }

    let resp = client
        .get(download_url)
        .header("User-Agent", "RustTube-Desktop-Client")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("Failed to download update: HTTP {}", resp.status()));
    }

    let bytes = resp.bytes().await?;

    let updates_dir = PathBuf::from("cache").join("updates");
    std::fs::create_dir_all(&updates_dir)?;

    let staged_file = updates_dir.join("RustTube_New.exe");
    std::fs::write(&staged_file, &bytes)?;

    info!("Update executable downloaded and staged at {:?}", staged_file);
    Ok(staged_file)
}

pub fn launch_updater_helper(staged_exe_path: &Path) -> Result<()> {
    let current_exe = std::env::current_exe()?;
    let script_path = PathBuf::from("cache").join("updates").join("install_update.cmd");

    let script_content = format!(
        "@echo off\r\n\
         timeout /t 2 /nobreak > nul\r\n\
         copy /y \"{}\" \"{}\"\r\n\
         start \"\" \"{}\"\r\n\
         del \"%~f0\"\r\n",
        staged_exe_path.to_string_lossy(),
        current_exe.to_string_lossy(),
        current_exe.to_string_lossy()
    );

    std::fs::write(&script_path, script_content)?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        std::process::Command::new("cmd.exe")
            .arg("/C")
            .arg(&script_path)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("sleep 1 && cp \"{}\" \"{}\" && \"{}\"", staged_exe_path.display(), current_exe.display(), current_exe.display()))
            .spawn()?;
    }

    info!("Updater helper script launched successfully.");
    Ok(())
}

fn is_version_newer(current: &str, latest: &str) -> bool {
    let parse_ver = |v: &str| -> Vec<u32> {
        v.split('.')
            .map(|s| s.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0))
            .collect()
    };

    let cur_parts = parse_ver(current);
    let lat_parts = parse_ver(latest);

    for i in 0..3 {
        let c = cur_parts.get(i).copied().unwrap_or(0);
        let l = lat_parts.get(i).copied().unwrap_or(0);
        if l > c {
            return true;
        } else if l < c {
            return false;
        }
    }
    false
}
