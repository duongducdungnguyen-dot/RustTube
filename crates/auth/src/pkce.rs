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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in_seconds: u64,
}

pub struct GoogleAuthManager {
    client_id: String,
    redirect_port: u16,
    token_store: SecureTokenStore,
    current_access_token: Arc<Mutex<Option<String>>>,
}

impl GoogleAuthManager {
    pub fn new(client_id: &str, redirect_port: u16, service_name: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            redirect_port,
            token_store: SecureTokenStore::new(service_name, "personal_account"),
            current_access_token: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.client_id.trim().is_empty()
    }

    pub async fn get_access_token(&self) -> Result<Option<String>> {
        {
            let current = self.current_access_token.lock().await;
            if let Some(token) = current.as_ref() {
                return Ok(Some(token.clone()));
            }
        }

        if let Ok(Some(refresh_token)) = self.token_store.get_refresh_token() {
            if let Ok(new_token) = self.refresh_access_token(&refresh_token).await {
                let mut current = self.current_access_token.lock().await;
                *current = Some(new_token.clone());
                return Ok(Some(new_token));
            }
        }

        Ok(None)
    }

    pub async fn start_oauth_flow(&self) -> Result<String> {
        if self.client_id.is_empty() {
            return Err(anyhow!("Google Client ID is not configured in config.toml"));
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
            .set_pkce_challenge(pkce_challenge)
            .url();

        let _ = open::that(auth_url.as_str());

        let code = self.listen_for_callback(csrf_token.secret()).await?;

        let token_resp = self.exchange_code(&code, pkce_verifier).await?;
        let access_token = token_resp.access_token().secret().to_string();

        if let Some(refresh_token) = token_resp.refresh_token() {
            let _ = self.token_store.save_refresh_token(refresh_token.secret());
        }

        let mut current = self.current_access_token.lock().await;
        *current = Some(access_token.clone());

        Ok(access_token)
    }

    async fn listen_for_callback(&self, expected_state: &str) -> Result<String> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.redirect_port));
        let listener = TcpListener::bind(addr).await?;

        let (mut socket, _) = listener.accept().await?;
        let mut buffer = [0u8; 2048];
        let bytes_read = socket.read(&mut buffer).await?;
        let request = String::from_utf8_lossy(&buffer[..bytes_read]);

        let mut auth_code = None;
        let mut state_matches = false;

        if let Some(first_line) = request.lines().next() {
            if let Some(path) = first_line.split_whitespace().nth(1) {
                if let Ok(parsed_url) = url::Url::parse(&format!("http://127.0.0.1{}", path)) {
                    for (k, v) in parsed_url.query_pairs() {
                        if k == "code" {
                            auth_code = Some(v.to_string());
                        } else if k == "state" && v == expected_state {
                            state_matches = true;
                        }
                    }
                }
            }
        }

        let html_response = if auth_code.is_some() && state_matches {
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='font-family:sans-serif;background:#0f0f12;color:#fff;text-align:center;padding:50px;'><h2>Authentication Successful</h2><p>You can close this tab and return to RustTube.</p></body></html>"
        } else {
            "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='font-family:sans-serif;background:#0f0f12;color:#fff;text-align:center;padding:50px;'><h2>Authentication Failed</h2><p>State mismatch or invalid code.</p></body></html>"
        };

        let _ = socket.write_all(html_response.as_bytes()).await;
        let _ = socket.flush().await;

        if !state_matches {
            return Err(anyhow!("OAuth state parameter mismatch"));
        }

        auth_code.ok_or_else(|| anyhow!("No authorization code received in callback"))
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
            .build()?;

        let token_result = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(&http_client)
            .await
            .map_err(|e| anyhow!("Token exchange failed: {}", e))?;

        Ok(token_result)
    }

    async fn refresh_access_token(&self, refresh_token: &str) -> Result<String> {
        let client = reqwest::Client::new();
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
            return Err(anyhow!("Failed to refresh token: status {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await?;
        let access_token = body["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access_token in refresh response"))?
            .to_string();

        Ok(access_token)
    }

    pub async fn logout(&self) -> Result<()> {
        let mut current = self.current_access_token.lock().await;
        *current = None;
        self.token_store.delete_refresh_token()?;
        Ok(())
    }
}
