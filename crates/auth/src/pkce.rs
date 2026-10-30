use crate::keyring_store::{KeyringTokenStore, StoredTokenSet};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::Utc;
use rand::RngCore;
use reqwest::Client;
use rusttube_core::{ClassifiedError, SyncErrorKind};
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub const YOUTUBE_SCOPES: &str =
    "https://www.googleapis.com/auth/youtube https://www.googleapis.com/auth/youtube.force-ssl";

#[derive(Debug, Clone)]
pub struct PkceChallenge {
    pub verifier: String,
    pub challenge: String,
    pub state: String,
}

impl PkceChallenge {
    pub fn generate() -> Self {
        let mut verifier_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut verifier_bytes);
        let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

        let mut state_bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut state_bytes);
        let state = URL_SAFE_NO_PAD.encode(state_bytes);

        Self {
            verifier,
            challenge,
            state,
        }
    }

    pub fn authorization_url(&self, client_id: &str, redirect_port: u16) -> String {
        let redirect_uri = format!("http://127.0.0.1:{}/callback", redirect_port);
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}&access_type=offline&prompt=select_account%20consent",
            urlencoding(client_id),
            urlencoding(&redirect_uri),
            urlencoding(YOUTUBE_SCOPES),
            urlencoding(&self.challenge),
            urlencoding(&self.state),
        )
    }
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
    scope: Option<String>,
}

pub struct OAuthSessionManager {
    client: Client,
    client_id: String,
    store: KeyringTokenStore,
}

impl OAuthSessionManager {
    pub fn new(client_id: String, keyring_service: String) -> Self {
        Self {
            client: Client::new(),
            client_id,
            store: KeyringTokenStore::new(keyring_service),
        }
    }

    /// Restores session at startup from Windows Credential Manager and refreshes expired access tokens.
    pub async fn restore_or_refresh_session(&self) -> std::result::Result<Option<StoredTokenSet>, ClassifiedError> {
        let existing = self.store.load_tokens().map_err(|e| ClassifiedError {
            kind: SyncErrorKind::Authentication,
            message: format!("Failed to read Windows Credential Manager: {}", e),
            occurred_at: Utc::now().timestamp(),
        })?;

        let Some(tokens) = existing else {
            return Ok(None);
        };

        let now = Utc::now().timestamp();
        if !tokens.is_expired(now) {
            return Ok(Some(tokens));
        }

        // Refresh expired access token while preserving the refresh_token in Windows Credential Manager
        let refreshed = self.refresh_access_token(&tokens.refresh_token).await?;
        Ok(Some(refreshed))
    }

    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> std::result::Result<StoredTokenSet, ClassifiedError> {
        let now = Utc::now().timestamp();
        let params = [
            ("client_id", self.client_id.as_str()),
            ("refresh_token", refresh_token),
            ("grant_type", "refresh_token"),
        ];

        let resp = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Network error refreshing OAuth token: {}", e),
                occurred_at: now,
            })?;

        if !resp.status().is_success() {
            return Err(ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: format!(
                    "OAuth token refresh rejected with HTTP {}",
                    resp.status().as_u16()
                ),
                occurred_at: now,
            });
        }

        let body: GoogleTokenResponse = resp.json().await.map_err(|e| ClassifiedError {
            kind: SyncErrorKind::Parsing,
            message: format!("Invalid token refresh response format: {}", e),
            occurred_at: now,
        })?;

        let updated = StoredTokenSet {
            access_token: body.access_token,
            refresh_token: body
                .refresh_token
                .unwrap_or_else(|| refresh_token.to_string()),
            expires_at_unix: now + body.expires_in,
            scope: body.scope.unwrap_or_else(|| YOUTUBE_SCOPES.to_string()),
        };

        self.store.save_tokens(&updated).map_err(|e| ClassifiedError {
            kind: SyncErrorKind::Authentication,
            message: format!("Failed to persist refreshed token in Credential Manager: {}", e),
            occurred_at: now,
        })?;

        Ok(updated)
    }

    pub fn logout(&self) -> Result<()> {
        self.store.clear_tokens().context("Failed to clear credentials")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_s256_challenge_and_redaction() {
        let pkce = PkceChallenge::generate();
        assert!(!pkce.verifier.is_empty());
        assert!(!pkce.challenge.is_empty());
        assert_ne!(pkce.verifier, pkce.challenge);

        let token = StoredTokenSet {
            access_token: "ya29.secret_access_token".to_string(),
            refresh_token: "1//secret_refresh_token".to_string(),
            expires_at_unix: 1000,
            scope: YOUTUBE_SCOPES.to_string(),
        };
        let debug_str = format!("{:?}", token);
        assert!(!debug_str.contains("ya29.secret_access_token"));
        assert!(!debug_str.contains("1//secret_refresh_token"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(token.is_expired(950));
        assert!(!token.is_expired(800));
    }
}
