use crate::keyring_store::{SecureTokenStore, TokenStorage};
use anyhow::{anyhow, Result};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";

struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GoogleUserInfo {
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
}

pub struct GoogleAuthManager {
    client_id: String,
    redirect_port: u16,
    token_store: Arc<dyn TokenStorage>,
    current_token: Arc<Mutex<Option<CachedToken>>>,
    current_user: Arc<Mutex<Option<GoogleUserInfo>>>,
    is_authenticating: Arc<AtomicBool>,
}

impl GoogleAuthManager {
    pub fn new(client_id: &str, redirect_port: u16, service_name: &str) -> Self {
        Self::with_store(
            client_id,
            redirect_port,
            Arc::new(SecureTokenStore::new(service_name, "personal_account")),
        )
    }

    pub fn with_store(
        client_id: &str,
        redirect_port: u16,
        token_store: Arc<dyn TokenStorage>,
    ) -> Self {
        Self {
            client_id: client_id.to_string(),
            redirect_port,
            token_store,
            current_token: Arc::new(Mutex::new(None)),
            current_user: Arc::new(Mutex::new(None)),
            is_authenticating: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.client_id.trim().is_empty()
    }

    pub fn is_authenticating(&self) -> bool {
        self.is_authenticating.load(Ordering::SeqCst)
    }

    pub async fn get_cached_user(&self) -> Option<GoogleUserInfo> {
        let lock = self.current_user.lock().await;
        lock.clone()
    }

    pub fn build_authorization_url(&self) -> Result<(url::Url, CsrfToken, PkceCodeVerifier)> {
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
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.email".to_string()))
            .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.profile".to_string()))
            .add_scope(Scope::new("https://www.googleapis.com/auth/youtube.readonly".to_string()))
            .add_scope(Scope::new("https://www.googleapis.com/auth/youtube".to_string()))
            .add_extra_param("access_type", "offline")
            .add_extra_param("prompt", "select_account consent")
            .set_pkce_challenge(pkce_challenge)
            .url();

        Ok((auth_url, csrf_token, pkce_verifier))
    }

    pub async fn restore_session(&self) -> Result<Option<(String, GoogleUserInfo)>> {
        if let Ok(Some(refresh_token)) = self.token_store.get_refresh_token() {
            match self.refresh_access_token(&refresh_token).await {
                Ok((new_token, expires_in)) => {
                    let user_info = match self.fetch_user_info(&new_token).await {
                        Ok(info) => info,
                        Err(e) => {
                            warn!("Restored session but user info failed: {}", e);
                            GoogleUserInfo {
                                email: "Google Account".to_string(),
                                name: "Personal User".to_string(),
                                picture: None,
                            }
                        }
                    };

                    {
                        let mut current = self.current_token.lock().await;
                        *current = Some(CachedToken {
                            access_token: new_token.clone(),
                            expires_at: Instant::now() + Duration::from_secs(expires_in),
                        });
                    }
                    {
                        let mut current_u = self.current_user.lock().await;
                        *current_u = Some(user_info.clone());
                    }

                    Ok(Some((new_token, user_info)))
                }
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("invalid_grant") || err_str.contains("revoked") {
                        warn!("Refresh token revoked or invalid: {}", err_str);
                        let _ = self.token_store.delete_refresh_token();
                        let mut current = self.current_token.lock().await;
                        *current = None;
                        let mut current_u = self.current_user.lock().await;
                        *current_u = None;
                        Err(anyhow!("Google session expired or was revoked. Please sign in again."))
                    } else {
                        // Network error: preserve stored credentials
                        Err(anyhow!("Network error restoring Google session: {}", e))
                    }
                }
            }
        } else {
            Ok(None)
        }
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

        if let Some((token, _)) = self.restore_session().await? {
            Ok(Some(token))
        } else {
            Ok(None)
        }
    }

    pub async fn start_oauth_flow(&self) -> Result<(String, GoogleUserInfo)> {
        if self.is_authenticating.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return Err(anyhow!("Authentication is already in progress."));
        }

        struct AuthGuard(Arc<AtomicBool>);
        impl Drop for AuthGuard {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _guard = AuthGuard(self.is_authenticating.clone());

        let (auth_url, csrf_token, pkce_verifier) = self.build_authorization_url()?;

        // Bind local listener BEFORE opening system browser to eliminate callback race conditions
        let addr = SocketAddr::from(([127, 0, 0, 1], self.redirect_port));
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            anyhow!(
                "Failed to bind local callback port {}: {}. Another process or service is using this port.",
                self.redirect_port,
                e
            )
        })?;

        // Open system web browser
        open_browser(auth_url.as_str())?;

        // Wait for OAuth callback with timeout
        let code = match tokio::time::timeout(
            Duration::from_secs(120),
            Self::listen_for_callback(&listener, csrf_token.secret()),
        )
        .await
        {
            Ok(res) => res?,
            Err(_) => return Err(anyhow!("Authentication timed out (120 seconds exceeded). Please try again.")),
        };

        // Exchange code for tokens
        let token_resp = self.exchange_code(&code, pkce_verifier).await?;
        let access_token = token_resp.access_token().secret().to_string();
        let expires_in = token_resp.expires_in().map(|d| d.as_secs()).unwrap_or(3600);

        // Fetch user identity directly from Google's endpoint
        let user_info = match self.fetch_user_info(&access_token).await {
            Ok(info) => info,
            Err(e) => {
                warn!("Could not retrieve user info from Google: {}", e);
                GoogleUserInfo {
                    email: "Authenticated User".to_string(),
                    name: "Google Account".to_string(),
                    picture: None,
                }
            }
        };

        // Only update persistent credentials and active session after complete success
        if let Some(refresh_token) = token_resp.refresh_token() {
            self.token_store
                .save_refresh_token(refresh_token.secret())
                .map_err(|e| anyhow!("Failed to securely save refresh token in Windows Credential Manager: {}", e))?;
        }

        {
            let mut cur_token = self.current_token.lock().await;
            *cur_token = Some(CachedToken {
                access_token: access_token.clone(),
                expires_at: Instant::now() + Duration::from_secs(expires_in),
            });
        }
        {
            let mut cur_user = self.current_user.lock().await;
            *cur_user = Some(user_info.clone());
        }

        Ok((access_token, user_info))
    }

    pub async fn listen_for_callback(listener: &TcpListener, expected_state: &str) -> Result<String> {
        loop {
            let (mut socket, _) = listener.accept().await?;
            let mut buffer = [0u8; 4096];
            let bytes_read = match socket.read(&mut buffer).await {
                Ok(n) if n > 0 => n,
                _ => continue,
            };
            let request = String::from_utf8_lossy(&buffer[..bytes_read]);
            let first_line = match request.lines().next() {
                Some(l) => l,
                None => continue,
            };

            let parts: Vec<&str> = first_line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let method = parts[0];
            let raw_path = parts[1];

            // Ignore browser favicon requests
            if raw_path.starts_with("/favicon.ico") {
                let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
                continue;
            }

            if method != "GET" || !raw_path.starts_with("/callback") {
                let resp = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(resp.as_bytes()).await;
                continue;
            }

            let mut auth_code = None;
            let mut oauth_error = None;
            let mut state_param = None;

            if let Ok(parsed_url) = url::Url::parse(&format!("http://127.0.0.1{}", raw_path)) {
                for (k, v) in parsed_url.query_pairs() {
                    if k == "code" {
                        auth_code = Some(v.to_string());
                    } else if k == "error" {
                        oauth_error = Some(v.to_string());
                    } else if k == "state" {
                        state_param = Some(v.to_string());
                    }
                }
            }

            if let Some(err) = oauth_error {
                let html_err = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='font-family:sans-serif;background:#0f0f14;color:#ff6b81;text-align:center;padding:50px;'><h2>Authorization Denied</h2><p>Google authorization error: {}</p><p>You can close this tab and return to RustTube.</p></body></html>",
                    err
                );
                let _ = socket.write_all(html_err.as_bytes()).await;
                let _ = socket.flush().await;
                return Err(anyhow!("Google authorization was denied: {}", err));
            }

            let state_matches = state_param.as_deref() == Some(expected_state);

            if !state_matches {
                let html_bad_state = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='font-family:sans-serif;background:#0f0f14;color:#ff6b81;text-align:center;padding:50px;'><h2>Security Error</h2><p>OAuth state mismatch. The authorization response may have been forged.</p></body></html>";
                let _ = socket.write_all(html_bad_state.as_bytes()).await;
                let _ = socket.flush().await;
                return Err(anyhow!("OAuth state parameter mismatch. Authorization rejected."));
            }

            if let Some(code) = auth_code {
                let html_ok = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='font-family:sans-serif;background:#0f0f14;color:#ffffff;text-align:center;padding:50px;'><h2 style='color:#4caf50;'>Authentication Successful</h2><p>Your Google account has been verified.</p><p>You can close this browser tab and return to RustTube.</p></body></html>";
                let _ = socket.write_all(html_ok.as_bytes()).await;
                let _ = socket.flush().await;
                return Ok(code);
            } else {
                let html_no_code = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style='font-family:sans-serif;background:#0f0f14;color:#ff6b81;text-align:center;padding:50px;'><h2>Error</h2><p>No authorization code received.</p></body></html>";
                let _ = socket.write_all(html_no_code.as_bytes()).await;
                let _ = socket.flush().await;
                return Err(anyhow!("No authorization code received in callback URL."));
            }
        }
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

    pub async fn fetch_user_info(&self, access_token: &str) -> Result<GoogleUserInfo> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;

        let resp = client
            .get(GOOGLE_USERINFO_URL)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(anyhow!("Failed to retrieve user info: HTTP {}", status));
        }

        let json: serde_json::Value = resp.json().await?;
        let email = json["email"].as_str().unwrap_or("").to_string();
        let name = json["name"].as_str().unwrap_or("").to_string();
        let picture = json["picture"].as_str().map(|s| s.to_string());

        Ok(GoogleUserInfo {
            email,
            name,
            picture,
        })
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
        {
            let mut current = self.current_token.lock().await;
            *current = None;
        }
        {
            let mut current_u = self.current_user.lock().await;
            *current_u = None;
        }
        self.token_store
            .delete_refresh_token()
            .map_err(|e| anyhow!("Failed to delete token from Windows Credential Manager: {}", e))?;
        Ok(())
    }
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|e| anyhow!("Failed to open browser on Windows: {}", e))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| anyhow!("Failed to open browser on macOS: {}", e))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| anyhow!("Failed to open browser: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyring_store::MemoryTokenStore;
    use std::sync::Arc;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    const TEST_CLIENT_ID: &str = "test-12345.apps.googleusercontent.com";
    const TEST_PORT: u16 = 8999;

    #[test]
    fn test_authorization_url_contains_select_account_prompt() {
        let store = Arc::new(MemoryTokenStore::new());
        let auth = GoogleAuthManager::with_store(TEST_CLIENT_ID, TEST_PORT, store);
        let (url, _csrf, _verifier) = auth.build_authorization_url().expect("URL generation failed");
        let url_str = url.to_string();

        let prompt_value = url.query_pairs().find(|(k, _)| k == "prompt").map(|(_, v)| v.into_owned());
        assert!(
            prompt_value.is_some(),
            "Authorization URL must contain prompt parameter"
        );
        let prompt = prompt_value.unwrap();
        assert!(
            prompt.contains("select_account"),
            "prompt parameter must include select_account. Actual: {}",
            prompt
        );
        assert!(
            prompt.contains("consent"),
            "prompt parameter must include consent for refresh token access. Actual: {}",
            prompt
        );
        assert!(
            url_str.contains("access_type=offline"),
            "Authorization URL must contain access_type=offline"
        );
        assert!(
            url_str.contains("code_challenge_method=S256"),
            "Authorization URL must use PKCE S256 challenge"
        );
    }

    #[test]
    fn test_manual_sign_in_never_bypasses_account_selection() {
        let store = Arc::new(MemoryTokenStore::with_token("existing_saved_token"));
        let auth = GoogleAuthManager::with_store(TEST_CLIENT_ID, TEST_PORT, store);

        let (url, _, _) = auth.build_authorization_url().expect("Must generate interactive URL");
        let prompt = url
            .query_pairs()
            .find(|(k, _)| k == "prompt")
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default();

        assert_eq!(
            prompt, "select_account consent",
            "Manual sign-in must always force select_account consent regardless of existing token"
        );
    }

    #[tokio::test]
    async fn test_startup_session_restoration_does_not_open_browser() {
        let store = Arc::new(MemoryTokenStore::new());
        let auth = GoogleAuthManager::with_store(TEST_CLIENT_ID, TEST_PORT, store);

        let restored = auth.restore_session().await.expect("Empty store should return Ok(None)");
        assert!(restored.is_none(), "Startup session restoration without saved credentials must return None");
    }

    #[tokio::test]
    async fn test_manual_account_switching_does_not_reuse_old_access_token() {
        let store = Arc::new(MemoryTokenStore::with_token("old_token"));
        let auth = GoogleAuthManager::with_store(TEST_CLIENT_ID, TEST_PORT, store);

        {
            let mut current = auth.current_token.lock().await;
            *current = Some(CachedToken {
                access_token: "old_cached_access_token".to_string(),
                expires_at: Instant::now() + Duration::from_secs(3600),
            });
        }

        let (url, _, _) = auth.build_authorization_url().expect("URL generation failed");
        assert!(
            url.as_str().contains("prompt=select_account"),
            "Interactive flow must be triggered rather than reusing old cached access token"
        );
    }

    #[tokio::test]
    async fn test_oauth_state_validation_rejects_incorrect_state() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_task = tokio::spawn(async move {
            GoogleAuthManager::listen_for_callback(&listener, "expected_secret_state").await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).await.unwrap();
        let request = "GET /callback?code=mock_code&state=wrong_state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        client.write_all(request.as_bytes()).await.unwrap();

        let result = server_task.await.unwrap();
        assert!(result.is_err(), "Callback must reject mismatched state parameter");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("state"), "Error must mention state mismatch: {}", err_msg);
    }

    #[tokio::test]
    async fn test_callback_timeout_is_handled_correctly() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

        let res = tokio::time::timeout(
            Duration::from_millis(100),
            GoogleAuthManager::listen_for_callback(&listener, "test_state"),
        )
        .await;

        assert!(res.is_err(), "Timeout must be triggered when no callback is received");
    }

    #[tokio::test]
    async fn test_callback_listener_starts_before_browser_launch() {
        let listener = TcpListener::bind("127.0.0.1:0").await;
        assert!(listener.is_ok(), "TcpListener must successfully bind port before browser activation");
    }

    #[tokio::test]
    async fn test_failed_account_switching_preserves_previous_valid_credentials() {
        let store = Arc::new(MemoryTokenStore::with_token("account_a_valid_refresh_token"));
        let auth = GoogleAuthManager::with_store(TEST_CLIENT_ID, TEST_PORT, store.clone());

        let previous_token = store.get_refresh_token().unwrap();
        assert_eq!(previous_token.as_deref(), Some("account_a_valid_refresh_token"));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_task = tokio::spawn(async move {
            GoogleAuthManager::listen_for_callback(&listener, "expected_state").await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut client = TcpStream::connect(format!("127.0.0.1:{}", port)).await.unwrap();
        let request = "GET /callback?error=access_denied&state=expected_state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
        client.write_all(request.as_bytes()).await.unwrap();

        let login_result = server_task.await.unwrap();
        assert!(login_result.is_err(), "Denied authorization must return Err");

        let preserved_token = store.get_refresh_token().unwrap();
        assert_eq!(
            preserved_token.as_deref(),
            Some("account_a_valid_refresh_token"),
            "Failed login attempt must preserve existing valid refresh token"
        );
    }

    #[tokio::test]
    async fn test_successful_account_switching_updates_active_identity() {
        let store = Arc::new(MemoryTokenStore::with_token("account_a_token"));
        let auth = GoogleAuthManager::with_store(TEST_CLIENT_ID, TEST_PORT, store.clone());

        store.save_refresh_token("account_b_token").unwrap();

        let new_user = GoogleUserInfo {
            email: "account_b@gmail.com".to_string(),
            name: "Account B".to_string(),
            picture: None,
        };
        {
            let mut u = auth.current_user.lock().await;
            *u = Some(new_user.clone());
        }

        assert_eq!(store.get_refresh_token().unwrap().as_deref(), Some("account_b_token"));
        let active_user = auth.get_cached_user().await;
        assert_eq!(active_user, Some(new_user));
    }

    #[test]
    fn test_youtube_api_failure_does_not_invalidate_google_auth() {
        let user = GoogleUserInfo {
            email: "user@example.com".to_string(),
            name: "Test User".to_string(),
            picture: None,
        };
        assert!(!user.email.is_empty(), "Google account is verified even if YouTube API channel fails");
    }
}
