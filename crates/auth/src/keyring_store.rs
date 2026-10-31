use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fmt;

// Keep the original Windows Credential Manager target as canonical. Older RustTube
// builds stored the refresh token here as a plain string, so changing this username
// made existing sessions appear signed out after upgrades.
const DEFAULT_USERNAME: &str = "personal_account";
// Read credentials written by intermediate builds that used this newer username.
const LEGACY_USERNAME: &str = "personal_youtube_account";

#[derive(Clone, Serialize, Deserialize)]
pub struct StoredTokenSet {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_unix: i64,
    pub scope: String,
}

// Redact sensitive tokens from Debug output so tokens are never exposed in logs
impl fmt::Debug for StoredTokenSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredTokenSet")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("expires_at_unix", &self.expires_at_unix)
            .field("scope", &self.scope)
            .finish()
    }
}

impl StoredTokenSet {
    pub fn is_expired(&self, now_unix: i64) -> bool {
        // Refresh 60 seconds prior to actual expiration
        now_unix + 60 >= self.expires_at_unix
    }
}

#[derive(Clone)]
pub struct KeyringTokenStore {
    service_name: String,
}

impl KeyringTokenStore {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    pub fn save_tokens(&self, tokens: &StoredTokenSet) -> Result<()> {
        let entry = Entry::new(&self.service_name, DEFAULT_USERNAME)
            .context("Failed to initialize Windows Credential Manager entry")?;
        let payload = serde_json::to_string(tokens)?;
        entry
            .set_password(&payload)
            .context("Failed to store OAuth tokens in Windows Credential Manager")?;
        Ok(())
    }

    pub fn load_tokens(&self) -> Result<Option<StoredTokenSet>> {
        let entry = Entry::new(&self.service_name, DEFAULT_USERNAME)
            .context("Failed to open Windows Credential Manager entry")?;
        match entry.get_password() {
            Ok(raw) => Ok(Some(parse_token_payload(&raw)?)),
            Err(keyring::Error::NoEntry) => {
                let legacy_entry = Entry::new(&self.service_name, LEGACY_USERNAME)
                    .context("Failed to open legacy Windows Credential Manager entry")?;
                match legacy_entry.get_password() {
                    Ok(raw) => {
                        let tokens = parse_token_payload(&raw)?;
                        // Best-effort migration. Keep the legacy value if saving under the
                        // current username fails, so the next read can still recover it.
                        let _ = self.save_tokens(&tokens);
                        Ok(Some(tokens))
                    }
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(e) => Err(anyhow::anyhow!("Legacy keyring read error: {}", e)),
                }
            }
            Err(e) => Err(anyhow::anyhow!("Keyring read error: {}", e)),
        }
    }

    pub fn clear_tokens(&self) -> Result<()> {
        for username in [DEFAULT_USERNAME, LEGACY_USERNAME] {
            let entry = Entry::new(&self.service_name, username)?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Failed to clear keyring credential for {}: {}",
                        username,
                        e
                    ));
                }
            }
        }
        Ok(())
    }
}

fn parse_token_payload(raw: &str) -> Result<StoredTokenSet> {
    if let Ok(tokens) = serde_json::from_str(raw) {
        return Ok(tokens);
    }

    let refresh_token = raw.trim();
    if refresh_token.is_empty() {
        anyhow::bail!("Stored OAuth credential is empty");
    }

    // Before StoredTokenSet was introduced, RustTube stored just the refresh token.
    Ok(StoredTokenSet {
        access_token: String::new(),
        refresh_token: refresh_token.to_string(),
        expires_at_unix: 0,
        scope: String::new(),
    })
}
