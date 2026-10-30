use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fmt;

const DEFAULT_USERNAME: &str = "personal_youtube_account";

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
            Ok(raw) => {
                let parsed: StoredTokenSet = serde_json::from_str(&raw)
                    .context("Stored credential JSON in keyring is malformed")?;
                Ok(Some(parsed))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Keyring read error: {}", e)),
        }
    }

    pub fn clear_tokens(&self) -> Result<()> {
        let entry = Entry::new(&self.service_name, DEFAULT_USERNAME)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Failed to clear keyring credential: {}", e)),
        }
    }
}
