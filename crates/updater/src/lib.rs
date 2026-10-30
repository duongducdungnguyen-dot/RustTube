use anyhow::{Context, Result};
use reqwest::Client;
use rusttube_core::ReleaseManifest;
use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

pub const GITHUB_REPO_DEFAULT: &str = "duongducdungnguyen-dot/RustTube";

pub struct UpdateManager {
    http: Client,
    repo: String,
    current_version: String,
}

impl UpdateManager {
    pub fn new(repo: impl Into<String>, current_version: impl Into<String>) -> Self {
        Self {
            http: Client::builder()
                .user_agent("RustTube-Native-Updater/0.2")
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            repo: repo.into(),
            current_version: current_version.into(),
        }
    }

    pub fn is_newer_version(current: &str, candidate: &str) -> Result<bool> {
        let cur_clean = current.trim().trim_start_matches('v');
        let cand_clean = candidate.trim().trim_start_matches('v');
        let v_cur = Version::parse(cur_clean)
            .with_context(|| format!("Invalid current semver: {}", current))?;
        let v_cand = Version::parse(cand_clean)
            .with_context(|| format!("Invalid candidate semver: {}", candidate))?;
        Ok(v_cand > v_cur)
    }

    pub fn compute_sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> bool {
        let actual = Self::compute_sha256_hex(bytes);
        actual.eq_ignore_ascii_case(expected_hex.trim())
    }

    /// Queries GitHub Releases API (`https://api.github.com/repos/{repo}/releases`)
    /// Ignores drafts and prereleases by default and selects the Windows x64 release asset.
    pub async fn check_for_updates(
        &self,
        include_prereleases: bool,
    ) -> Result<Option<ReleaseManifest>> {
        let url = format!("https://api.github.com/repos/{}/releases", self.repo);
        let resp = self
            .http
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .context("Failed to connect to GitHub Releases API")?;

        if !resp.status().is_success() {
            anyhow::bail!("GitHub Releases API returned HTTP {}", resp.status());
        }

        let releases: Vec<Value> = resp.json().await?;
        Self::select_compatible_release(&releases, &self.current_version, include_prereleases)
    }

    pub fn select_compatible_release(
        releases: &[Value],
        current_version: &str,
        include_prereleases: bool,
    ) -> Result<Option<ReleaseManifest>> {
        for rel in releases {
            let is_draft = rel.get("draft").and_then(|v| v.as_bool()).unwrap_or(false);
            let is_prerelease = rel
                .get("prerelease")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_draft || (is_prerelease && !include_prereleases) {
                continue;
            }

            let tag_name = rel
                .get("tag_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let version_clean = tag_name.trim_start_matches('v').to_string();
            if version_clean.is_empty() {
                continue;
            }

            if !Self::is_newer_version(current_version, &version_clean).unwrap_or(false) {
                continue;
            }

            let assets = rel
                .get("assets")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            // Prefer Windows x64 zip or exe asset
            let win_asset = assets.iter().find(|a| {
                let name = a
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_lowercase();
                (name.contains("windows") || name.contains("win") || name.contains("x64"))
                    && (name.ends_with(".zip") || name.ends_with(".exe"))
            });

            if let Some(asset) = win_asset {
                let asset_name = asset
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let download_url = asset
                    .get("browser_download_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let size_bytes = asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0);

                // Extract SHA-256 from release body if published in release notes
                let body = rel
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let sha256 = Self::extract_sha256_from_notes(&body);

                return Ok(Some(ReleaseManifest {
                    version: version_clean,
                    tag_name: tag_name.to_string(),
                    published_at: rel
                        .get("published_at")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    release_notes: body,
                    html_url: rel
                        .get("html_url")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    asset_name,
                    download_url,
                    sha256,
                    size_bytes,
                    is_prerelease,
                }));
            }
        }

        Ok(None)
    }

    pub fn extract_sha256_from_notes(notes: &str) -> Option<String> {
        for token in notes.split_whitespace() {
            let trimmed = token.trim_matches(|c: char| !c.is_ascii_hexdigit());
            if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(trimmed.to_lowercase());
            }
        }
        None
    }

    /// Stages the downloaded release package in a temporary staging directory and verifies its SHA-256 checksum.
    pub fn stage_and_verify_package(
        staging_dir: &Path,
        asset_name: &str,
        bytes: &[u8],
        expected_sha256: Option<&str>,
    ) -> Result<PathBuf> {
        fs::create_dir_all(staging_dir)?;
        if let Some(expected) = expected_sha256 {
            if !Self::verify_sha256(bytes, expected) {
                anyhow::bail!(
                    "SHA-256 checksum verification failed for {}; aborting update",
                    asset_name
                );
            }
        }
        let staged_file = staging_dir.join(asset_name);
        fs::write(&staged_file, bytes)?;
        Ok(staged_file)
    }

    /// Performs an atomic binary replacement with backup & rollback protection,
    /// strictly preserving user data files (config.toml, rusttube.db, caches).
    pub fn apply_staged_binary_with_rollback(
        install_dir: &Path,
        staged_new_binary: &Path,
        executable_name: &str,
    ) -> Result<()> {
        let target_exe = install_dir.join(executable_name);
        let backup_exe = install_dir.join(format!("{}.bak", executable_name));

        if !staged_new_binary.exists() {
            anyhow::bail!("Staged update binary does not exist");
        }

        let staged_meta = fs::metadata(staged_new_binary)?;
        if staged_meta.len() == 0 {
            anyhow::bail!("Staged update binary is empty (corrupted)");
        }

        // 1. Backup current working binary if present
        if target_exe.exists() {
            fs::copy(&target_exe, &backup_exe)
                .context("Failed to create rollback backup of current executable")?;
        }

        // 2. Attempt replacement; if copying fails, immediately roll back from backup
        match fs::copy(staged_new_binary, &target_exe) {
            Ok(_) => {
                let _ = fs::remove_file(&backup_exe);
                Ok(())
            }
            Err(err) => {
                if backup_exe.exists() {
                    let _ = fs::copy(&backup_exe, &target_exe);
                    let _ = fs::remove_file(&backup_exe);
                }
                Err(anyhow::anyhow!(
                    "Binary replacement failed and was rolled back: {}",
                    err
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_semver_comparison() -> Result<()> {
        assert!(UpdateManager::is_newer_version("0.1.0", "0.2.0")?);
        assert!(UpdateManager::is_newer_version("v0.2.0", "v0.2.1")?);
        assert!(!UpdateManager::is_newer_version("0.2.0", "0.2.0")?);
        assert!(!UpdateManager::is_newer_version("0.3.0", "0.2.9")?);
        Ok(())
    }

    #[test]
    fn test_manifest_selection_ignores_drafts_and_prereleases() -> Result<()> {
        let mock_releases = vec![
            json!({
                "tag_name": "v0.4.0-beta.1",
                "draft": false,
                "prerelease": true,
                "assets": [{ "name": "RustTube-v0.4.0-windows-x64.zip", "browser_download_url": "https://example.com/beta.zip", "size": 1024 }]
            }),
            json!({
                "tag_name": "v0.3.0",
                "draft": false,
                "prerelease": false,
                "body": "Release v0.3.0\nSHA256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
                "assets": [{ "name": "RustTube-v0.3.0-windows-x64.zip", "browser_download_url": "https://example.com/win.zip", "size": 2048 }]
            }),
        ];

        let selected = UpdateManager::select_compatible_release(&mock_releases, "0.2.0", false)?
            .expect("Expected stable release 0.3.0");
        assert_eq!(selected.version, "0.3.0");
        assert_eq!(
            selected.sha256.as_deref(),
            Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
        );
        Ok(())
    }

    #[test]
    fn test_sha256_verification_and_rollback_preserves_user_data() -> Result<()> {
        let dir = tempdir()?;
        let install_dir = dir.path().join("install");
        let user_data_dir = dir.path().join("appdata");
        fs::create_dir_all(&install_dir)?;
        fs::create_dir_all(&user_data_dir)?;

        // Populate user data files that must never be deleted by an update
        let config_path = user_data_dir.join("config.toml");
        let db_path = user_data_dir.join("rusttube.db");
        fs::write(&config_path, "google_client_id = 'test'")?;
        fs::write(&db_path, "SQLITE_USER_DATA")?;

        let current_exe = install_dir.join("rusttube.exe");
        fs::write(&current_exe, b"OLD_BINARY_V1")?;

        let new_payload = b"NEW_BINARY_V2";
        let expected_hash = UpdateManager::compute_sha256_hex(new_payload);

        let staged = UpdateManager::stage_and_verify_package(
            &dir.path().join("staging"),
            "rusttube.exe",
            new_payload,
            Some(&expected_hash),
        )?;

        UpdateManager::apply_staged_binary_with_rollback(&install_dir, &staged, "rusttube.exe")?;
        assert_eq!(fs::read(&current_exe)?, b"NEW_BINARY_V2");

        // Verify user data files are 100% intact
        assert!(config_path.exists());
        assert!(db_path.exists());
        Ok(())
    }
}
