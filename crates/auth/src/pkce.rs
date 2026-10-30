use crate::keyring_store::SecureTokenStore;
use anyhow::{anyhow, Result};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

pub struct GoogleAuthManager {
    client_id: String,
    redirect_port: u16,
    token_store: SecureTokenStore,
    current_token: Arc<Mutex<Option<CachedToken>>>,
}

impl GoogleAuthManager {
    pub fn new(client_id: &str, redirect_port: u16, service_name: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            redirect_port,
            token_store: SecureTokenStore::new(service_name, "personal_account"),
            current_token: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.client_id.trim().is_empty()
    }

    pub async fn get_access_token(&self) -> Result<Option<String>> {
        {
            let current = self.current_token.lock().await;
            if let Some(ref cached) = *current {
                if cached.expires_at > Instant::now() + Duration::from_secs(60) {
                    return Ok(Some(cached.access_token.clone()));
                }
            }
        }

        if let Ok(Some(refresh_token)) = self.token_store.get_refresh_token() {
            match self.refresh_access_token(&refresh_token).await {
                Ok((new_token, expires_in)) => {
                    let mut current = self.current_token.lock().await;
                    *current = Some(CachedToken {
                        access_token: new_token.clone(),
                        expires_at: Instant::now() + Duration::from_secs(expires_in),
                    });
                    return Ok(Some(new_token));
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("invalid_grant") || err_str.contains("revoked") {
                        warn!("Refresh token revoked or invalid: {}", err_str);
                        let _ = self.token_store.delete_refresh_token();
                        let mut current = self.current_token.lock().await;
                        *current = None;
                        return Err(anyhow!("Google session expired. Please sign in again."));
                    } else {
                        // Temporary network error: do not erase stored credentials
                        return Err(anyhow!("Network error refreshing Google token: {}", e));
                    }
                }
            }
        }

        Ok(None)
    }

    pub async fn start_oauth_flow(&self) -> Result<String> {
        if self.client_id.trim().is_empty() {
            return Err(anyhow!(
                "Google Client ID is not configured. Please open config.toml and set google_client_id."
            ));
        }

        let redirect_uri = format!("http://127.0.0.1:{}/callback", self.redirect_port);
        let client = BasicClient::new(
            ClientId::new(self.client_id.clone()),
            None,
            AuthUrl::new(GOOGLE_AUTH_URL.to_string())?,
            Some(TokenUrl::new(GOOGLE_TOKEN_URL.to_string())?),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_uri)?);

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let (auth_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("https://www.googleapis.com/auth/youtube.readonly".to_string()))
            .add_scope(Scope::new("https://www.googleapis.com/auth/youtube".to_string()))
            .add_extra_param("access_type", "offline")
            .add_extra_param("prompt", "consent")
            .set_pkce_challenge(pkce_challenge)
            .url();

        if let Err(e) = open::that(auth_url.as_str()) {
            return Err(anyhow!("Failed to open system web browser: {}", e));
        }

        let code = match tokio::time::timeout(Duration::from_secs(120), self.listen_for_callback(csrf_token.secret())).await {
            Ok(res) => res?,
            Err(_) => return Err(anyhow!("Authentication timed out (120 seconds exceeded). Please try again.")),
        };

        let token_resp = self.exchange_code(&code, pkce_verifier).await?;
        let access_token = token_resp.access_token().secret().to_string();
        let expires_in = token_resp.expires_in().map(|d| d.as_secs()).unwrap_or(3600);

        if let Some(refresh_token) = token_resp.refresh_token() {
            self.token_store
                .save_refresh_token(refresh_token.secret())
                .map_err(|e| anyhow!("Failed to securely save refresh token in Windows Credential Manager: {}", e))?;
        }

        let mut current = self.current_token.lock().await;
        *current = Some(CachedToken {
            access_token: access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(expires_in),
        });

        Ok(access_token)
    }

    async fn listen_for_callback(&self, expected_state: &str) -> Result<String> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.redirect_port));
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            anyhow!(
                "Failed to bind local callback port {}: {}. Another instance or service may be using it.",
                self.redirect_port,
                e
            )
        })?;

        let (mut socket, _) = listener.accept().await?;
        let mut buffer = [0u8; 4096];
        let bytes_read = socket.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..bytes_read]);

        let mut auth_code = None;
        let mut oauth_error = None;
        let mut state_matches = false;

        if let Some(first_line) = request.lines().next() {
            if let Some(path) = first_line.split_whitespace().nth(1) {
                if let Ok(parsed_url) = url::Url::parse(&format!("http://127.0.0.1{}", path)) {
                    for (k, v) in parsed_url.query_pairs() {
                        if k == "code" {
                            auth_code = Some(v.to_string());
                        } else if k == "error" {
                            oauth_error = Some(v.to_string());
                        } else if k == "state" && v == expected_state {
                            state_matches = true;
                        }
                    }
                }
            }
        }

        if let Some(err) = oauth_error {
            let html_err = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='font-family:sans-serif;background:#0f0f14;color:#ff6b81;text-align:center;padding:50px;'><h2>Authorization Denied</h2><p>{}</p><p>You can close this tab and return to RustTube.</p></body></html>",
                err
            );
            let _ = socket.write_all(html_err.as_bytes()).await;
            let _ = socket.flush().await;
            return Err(anyhow!("Google authorization was denied: {}", err));
        }

        let html_response = if auth_code.is_some() && state_matches {
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='font-family:sans-serif;background:#0f0f14;color:#ffffff;text-align:center;padding:50px;'><h2>Authentication Successful</h2><p>You can close this browser tab and return to RustTube.</p></body></html>"
        } else {
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='font-family:sans-serif;background:#0f0f14;color:#ff6b81;text-align:center;padding:50px;'><h2>Authentication Error</h2><p>Invalid code or state parameter mismatch.</p></body></html>"
        };

        let _ = socket.write_all(html_response.as_bytes()).await;
        let _ = socket.flush().await;

        if !state_matches {
            return Err(anyhow!("OAuth state parameter mismatch. Authorization rejected."));
        }

        auth_code.ok_or_else(|| anyhow!("No authorization code received in callback URL."))
    }

    async fn exchange_code(
        &self,
        code: &str,
        pkce_verifier: PkceCodeVerifier,
    ) -> Result<oauth2::basic::BasicTokenResponse> {
        let redirect_uri = format!("http://127.0.0.1:{}/callback", self.redirect_port);
        let client = BasicClient::new(
            ClientId::new(self.client_id.clone()),
            None,
            AuthUrl::new(GOOGLE_AUTH_URL.to_string())?,
            Some(TokenUrl::new(GOOGLE_TOKEN_URL.to_string())?),
        )
        .set_redirect_uri(RedirectUrl::new(redirect_uri)?);

        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(15))
            .build()?;

        let token_result = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http_client)
            .await
            .map_err(|e| anyhow!("Token exchange failed: {}", e))?;

        Ok(token_result)
    }

    async fn refresh_access_token(&self, refresh_token: &str) -> Result<(String, u64)> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;
        let params = [
            ("client_id", self.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];

        let resp = client
            .post(GOOGLE_TOKEN_URL)
            .form(&params)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("Failed to refresh token (status {}): {}", status, body));
        }

        let body: serde_json::Value = resp.json().await?;
        let access_token = body["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access_token in refresh response"))?
            .to_string();
        let expires_in = body["expires_in"].as_u64().unwrap_or(3600);

        Ok((access_token, expires_in))
    }

    pub async fn logout(&self) -> Result<()> {
        let mut current = self.current_token.lock().await;
        *current = None;
        self.token_store
            .delete_refresh_token()
            .map_err(|e| anyhow!("Failed to delete token from Windows Credential Manager: {}", e))?;
        Ok(())
    }
}
