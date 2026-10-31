use crate::keyring_store::{KeyringTokenStore, StoredTokenSet};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::Utc;
use rand::RngCore;
use reqwest::Client;
use rusttube_core::{AuthConfig, ClassifiedError, SyncErrorKind};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::process::Command;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

pub const YOUTUBE_SCOPES: &str =
    "https://www.googleapis.com/auth/youtube https://www.googleapis.com/auth/youtube.force-ssl";

pub const DEFAULT_OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);

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

    pub fn redirect_uri(redirect_port: u16) -> String {
        let port = if redirect_port == 0 {
            8989
        } else {
            redirect_port
        };
        format!("http://127.0.0.1:{}/callback", port)
    }

    pub fn authorization_url(&self, client_id: &str, redirect_port: u16) -> String {
        let redirect_uri = Self::redirect_uri(redirect_port);
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

#[derive(Debug, Deserialize)]
pub(crate) struct GoogleTokenResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleOAuthErrorPayload {
    error: Option<String>,
    error_description: Option<String>,
}

/// Parses an HTTP request line or callback path (e.g. `GET /callback?code=...&state=... HTTP/1.1`),
/// validates the OAuth `state` parameter, checks for provider `error` query parameters,
/// and returns the authorization `code`.
pub fn parse_and_validate_callback_request(
    request_line_or_target: &str,
    expected_state: &str,
) -> std::result::Result<String, ClassifiedError> {
    let now = Utc::now().timestamp();
    let first_line = request_line_or_target.lines().next().unwrap_or("").trim();

    let path_and_query = if first_line.starts_with("GET ") || first_line.starts_with("POST ") {
        first_line
            .split_whitespace()
            .nth(1)
            .unwrap_or(first_line)
    } else {
        first_line
    };

    let full_url = if path_and_query.starts_with("http://") || path_and_query.starts_with("https://")
    {
        path_and_query.to_string()
    } else if path_and_query.starts_with('/') {
        format!("http://127.0.0.1{}", path_and_query)
    } else {
        format!("http://127.0.0.1/{}", path_and_query)
    };

    let parsed = Url::parse(&full_url).map_err(|e| ClassifiedError {
        kind: SyncErrorKind::Authentication,
        message: format!("Malformed OAuth loopback callback URI: {}", e),
        occurred_at: now,
    })?;

    if parsed.path() != "/callback" {
        return Err(ClassifiedError {
            kind: SyncErrorKind::Authentication,
            message: format!(
                "Unexpected OAuth callback path '{}'; expected '/callback'",
                parsed.path()
            ),
            occurred_at: now,
        });
    }

    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    let mut error: Option<String> = None;
    let mut error_description: Option<String> = None;

    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "code" if !v.trim().is_empty() => code = Some(v.into_owned()),
            "state" if !v.trim().is_empty() => state = Some(v.into_owned()),
            "error" if !v.trim().is_empty() => error = Some(v.into_owned()),
            "error_description" if !v.trim().is_empty() => {
                error_description = Some(v.into_owned())
            }
            _ => {}
        }
    }

    if let Some(err_code) = error {
        let detail = error_description
            .map(|d| format!("{}: {}", err_code, d))
            .unwrap_or(err_code);
        return Err(ClassifiedError {
            kind: SyncErrorKind::Authentication,
            message: format!("Google Sign-In was denied or failed ({})", detail),
            occurred_at: now,
        });
    }

    let received_state = state.ok_or_else(|| ClassifiedError {
        kind: SyncErrorKind::Authentication,
        message: "OAuth callback missing required 'state' parameter".to_string(),
        occurred_at: now,
    })?;

    if received_state != expected_state {
        return Err(ClassifiedError {
            kind: SyncErrorKind::Authentication,
            message: "OAuth state mismatch detected; aborting sign-in to prevent CSRF".to_string(),
            occurred_at: now,
        });
    }

    let auth_code = code.ok_or_else(|| ClassifiedError {
        kind: SyncErrorKind::Authentication,
        message: "OAuth callback did not include an authorization code".to_string(),
        occurred_at: now,
    })?;

    Ok(auth_code)
}

/// Builds the URL-encoded form body for the installed-app PKCE authorization code exchange.
/// Does not embed or require a client_secret.
pub fn build_token_exchange_form(
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .append_pair("client_id", client_id)
        .append_pair("code", code)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_verifier", code_verifier)
        .append_pair("grant_type", "authorization_code")
        .finish()
}

/// Merges a token endpoint response with any previously stored token set so that
/// if Google omits `refresh_token` on a subsequent login or refresh, the previously
/// stored `refresh_token` is never lost.
pub(crate) fn merge_token_response(
    body: GoogleTokenResponse,
    previous_tokens: Option<&StoredTokenSet>,
    now_unix: i64,
) -> std::result::Result<StoredTokenSet, ClassifiedError> {
    if body.access_token.trim().is_empty() {
        return Err(ClassifiedError {
            kind: SyncErrorKind::Authentication,
            message: "Google OAuth token response contained an empty access_token".to_string(),
            occurred_at: now_unix,
        });
    }

    let refresh_token = body
        .refresh_token
        .filter(|rt| !rt.trim().is_empty())
        .or_else(|| {
            previous_tokens
                .map(|prev| prev.refresh_token.clone())
                .filter(|rt| !rt.trim().is_empty())
        })
        .unwrap_or_default();

    let scope = body
        .scope
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            previous_tokens
                .map(|prev| prev.scope.clone())
                .filter(|s| !s.trim().is_empty())
        })
        .unwrap_or_else(|| YOUTUBE_SCOPES.to_string());

    Ok(StoredTokenSet {
        access_token: body.access_token,
        refresh_token,
        expires_at_unix: now_unix + body.expires_in.max(60),
        scope,
    })
}

/// Maps HTTP error responses from `https://oauth2.googleapis.com/token` into a `ClassifiedError`
/// that preserves the safe provider error detail (`error` and `error_description`) without
/// exposing tokens or secrets, and provides actionable remediation when a Web application
/// credential (`client_secret is missing.`) is used instead of a Desktop app credential.
pub fn map_token_exchange_error(
    status: u16,
    err_text: &str,
    redirect_uri: &str,
) -> ClassifiedError {
    let now = Utc::now().timestamp();
    let parsed = serde_json::from_str::<GoogleOAuthErrorPayload>(err_text).ok();

    let detail = parsed
        .as_ref()
        .map(|p| {
            let code = p.error.as_deref().unwrap_or("oauth_error");
            match p.error_description.as_deref() {
                Some(desc) if !desc.trim().is_empty() => format!("{} ({})", code, desc.trim()),
                _ => code.to_string(),
            }
        })
        .unwrap_or_else(|| format!("HTTP {}", status));

    let lower_detail = detail.to_ascii_lowercase();
    if lower_detail.contains("client_secret is missing")
        || lower_detail.contains("client_secret")
        || lower_detail.contains("unauthorized_client")
    {
        return ClassifiedError {
            kind: SyncErrorKind::Authentication,
            message: format!(
                "Google OAuth code exchange failed: {}. The configured `google_client_id` belongs to a 'Web application' OAuth credential, which requires a confidential client_secret. RustTube is a native desktop public client using PKCE: in Google Cloud Console -> APIs & Services -> Credentials -> Create Credentials -> OAuth client ID, create an OAuth client with Application type 'Desktop app' (Desktop app clients automatically permit loopback redirect URI '{}' without a client secret), update `google_client_id` in config.toml, and sign in again.",
                detail, redirect_uri
            ),
            occurred_at: now,
        };
    }

    ClassifiedError {
        kind: SyncErrorKind::Authentication,
        message: format!(
            "Google OAuth code exchange failed: {}. Verify that your Google Cloud Console OAuth client type is 'Desktop app' and loopback redirect URI '{}' is permitted.",
            detail, redirect_uri
        ),
        occurred_at: now,
    }
}

/// Waits on the loopback `127.0.0.1` listener for the `/callback` redirect up to `timeout_duration`.
pub async fn wait_for_loopback_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout_duration: Duration,
) -> std::result::Result<String, ClassifiedError> {
    wait_for_loopback_callback_cancellable(listener, expected_state, timeout_duration, None).await
}

/// Waits on the loopback `127.0.0.1` listener for the `/callback` redirect up to `timeout_duration`,
/// or aborts immediately if `cancel_rx` resolves so the loopback port is released right away.
pub async fn wait_for_loopback_callback_cancellable(
    listener: TcpListener,
    expected_state: &str,
    timeout_duration: Duration,
    cancel_rx: Option<tokio::sync::oneshot::Receiver<()>>,
) -> std::result::Result<String, ClassifiedError> {
    let wait_block = async {
        loop {
            let (mut stream, _) = listener.accept().await.map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: format!("Loopback OAuth listener failed to accept connection: {}", e),
                occurred_at: Utc::now().timestamp(),
            })?;

            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                continue;
            }

            let raw_req = String::from_utf8_lossy(&buf[..n]);
            let first_line = raw_req.lines().next().unwrap_or("");

            // Ignore browser preflight/favicon requests while waiting for `/callback`
            if !first_line.contains("/callback") {
                let not_found =
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(not_found.as_bytes()).await;
                let _ = stream.flush().await;
                continue;
            }

            match parse_and_validate_callback_request(first_line, expected_state) {
                Ok(code) => {
                    let html = "<!doctype html><html><head><meta charset=\"utf-8\"><title>RustTube Sign-In Complete</title><style>body{font-family:system-ui,-apple-system,sans-serif;background:#0f1115;color:#f3f4f6;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}.card{background:#181b22;border:1px solid #2d3340;border-radius:10px;padding:28px 36px;max-width:440px;text-align:center}h2{margin:0 0 10px;color:#34d399}p{margin:0;color:#9ca3af;line-height:1.5}</style></head><body><div class=\"card\"><h2>Google Sign-In Successful</h2><p>You can close this browser tab and return to RustTube.</p></div></body></html>";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.flush().await;
                    return Ok(code);
                }
                Err(err) => {
                    let html = "<!doctype html><html><head><meta charset=\"utf-8\"><title>RustTube Sign-In Failed</title><style>body{font-family:system-ui,-apple-system,sans-serif;background:#0f1115;color:#f3f4f6;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}.card{background:#181b22;border:1px solid #ef4444;border-radius:10px;padding:28px 36px;max-width:440px;text-align:center}h2{margin:0 0 10px;color:#f87171}p{margin:0;color:#9ca3af;line-height:1.5}</style></head><body><div class=\"card\"><h2>Google Sign-In Failed</h2><p>Please return to RustTube for details and try signing in again.</p></div></body></html>";
                    let response = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.flush().await;
                    return Err(err);
                }
            }
        }
    };

    let timed_wait = tokio::time::timeout(timeout_duration, wait_block);

    match cancel_rx {
        Some(rx) => {
            tokio::select! {
                res = timed_wait => match res {
                    Ok(inner) => inner,
                    Err(_) => Err(ClassifiedError {
                        kind: SyncErrorKind::Authentication,
                        message: format!(
                            "Google Sign-In timed out after {} seconds waiting for the browser callback.",
                            timeout_duration.as_secs()
                        ),
                        occurred_at: Utc::now().timestamp(),
                    }),
                },
                _ = rx => Err(ClassifiedError {
                    kind: SyncErrorKind::Authentication,
                    message: "Google Sign-In was cancelled.".to_string(),
                    occurred_at: Utc::now().timestamp(),
                }),
            }
        }
        None => match timed_wait.await {
            Ok(result) => result,
            Err(_) => Err(ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: format!(
                    "Google Sign-In timed out after {} seconds waiting for the browser callback.",
                    timeout_duration.as_secs()
                ),
                occurred_at: Utc::now().timestamp(),
            }),
        },
    }
}

/// Opens the given authorization URL in the user's default system browser.
pub fn open_system_browser(url: &str) -> std::result::Result<(), ClassifiedError> {
    let now = Utc::now().timestamp();

    #[cfg(target_os = "windows")]
    let spawn_res = Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(url)
        .spawn();

    #[cfg(target_os = "macos")]
    let spawn_res = Command::new("open").arg(url).spawn();

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let spawn_res = Command::new("xdg-open").arg(url).spawn();

    spawn_res.map(|_| ()).map_err(|e| ClassifiedError {
        kind: SyncErrorKind::Authentication,
        message: format!("Failed to open system browser for Google Sign-In: {}", e),
        occurred_at: now,
    })
}

#[derive(Clone)]
pub struct OAuthSessionManager {
    client: Client,
    client_id: String,
    store: KeyringTokenStore,
}

impl OAuthSessionManager {
    pub fn new(client_id: String, keyring_service: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .unwrap_or_default(),
            client_id,
            store: KeyringTokenStore::new(keyring_service),
        }
    }

    pub fn set_client_id(&mut self, client_id: String) {
        self.client_id = client_id;
    }

    pub fn validate_configured_client_id(&self) -> std::result::Result<String, ClassifiedError> {
        AuthConfig::validate_client_id_str(&self.client_id)
    }

    /// Runs the complete installed-app Google OAuth2 PKCE flow:
    /// 1. Validates the configured `google_client_id`.
    /// 2. Binds a loopback listener strictly to `127.0.0.1:<redirect_port>`.
    /// 3. Opens the Google authorization URL in the system browser.
    /// 4. Waits for the `/callback` redirect with state validation and timeout.
    /// 5. Exchanges the authorization code using PKCE (`code_verifier`, no client secret)
    ///    and persists the resulting tokens in Windows Credential Manager.
    pub async fn start_pkce_login(
        &self,
        redirect_port: u16,
    ) -> std::result::Result<StoredTokenSet, ClassifiedError> {
        self.start_pkce_login_cancellable(redirect_port, None).await
    }

    /// Runs the installed-app Google OAuth2 PKCE flow with optional cancellation support so a
    /// pending callback wait can be abandoned immediately without keeping the loopback port bound.
    pub async fn start_pkce_login_cancellable(
        &self,
        redirect_port: u16,
        cancel_rx: Option<tokio::sync::oneshot::Receiver<()>>,
    ) -> std::result::Result<StoredTokenSet, ClassifiedError> {
        let valid_client_id = self.validate_configured_client_id()?;
        let requested_port = if redirect_port == 0 {
            8989
        } else {
            redirect_port
        };

        let listener = TcpListener::bind(("127.0.0.1", requested_port))
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: format!(
                    "Failed to bind loopback OAuth callback listener on 127.0.0.1:{}: {}",
                    requested_port, e
                ),
                occurred_at: Utc::now().timestamp(),
            })?;

        let bound_port = listener
            .local_addr()
            .map(|a| a.port())
            .unwrap_or(requested_port);

        let pkce = PkceChallenge::generate();
        let redirect_uri = PkceChallenge::redirect_uri(bound_port);
        let auth_url = pkce.authorization_url(&valid_client_id, bound_port);

        open_system_browser(&auth_url)?;

        let code = wait_for_loopback_callback_cancellable(
            listener,
            &pkce.state,
            DEFAULT_OAUTH_CALLBACK_TIMEOUT,
            cancel_rx,
        )
        .await?;

        self.exchange_code_for_tokens_with_client_id(
            &valid_client_id,
            &code,
            &pkce.verifier,
            &redirect_uri,
        )
        .await
    }

    /// Exchanges an authorization code for access and refresh tokens using PKCE (`code_verifier`),
    /// without requiring or sending a `client_secret`.
    pub async fn exchange_code_for_tokens(
        &self,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> std::result::Result<StoredTokenSet, ClassifiedError> {
        let valid_client_id = self.validate_configured_client_id()?;
        self.exchange_code_for_tokens_with_client_id(
            &valid_client_id,
            code,
            code_verifier,
            redirect_uri,
        )
        .await
    }

    async fn exchange_code_for_tokens_with_client_id(
        &self,
        valid_client_id: &str,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> std::result::Result<StoredTokenSet, ClassifiedError> {
        let now = Utc::now().timestamp();
        let form_body =
            build_token_exchange_form(valid_client_id, code, redirect_uri, code_verifier);

        let resp = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body)
            .send()
            .await
            .map_err(|e| ClassifiedError {
                kind: SyncErrorKind::Network,
                message: format!("Network error exchanging OAuth authorization code: {}", e),
                occurred_at: now,
            })?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(map_token_exchange_error(status, &err_text, redirect_uri));
        }

        let body: GoogleTokenResponse = resp.json().await.map_err(|e| ClassifiedError {
            kind: SyncErrorKind::Parsing,
            message: format!("Invalid Google OAuth token response format: {}", e),
            occurred_at: now,
        })?;

        let previous_tokens = self.store.load_tokens().ok().flatten();
        let stored = merge_token_response(body, previous_tokens.as_ref(), now)?;

        self.store.save_tokens(&stored).map_err(|e| ClassifiedError {
            kind: SyncErrorKind::Authentication,
            message: format!(
                "Failed to store OAuth tokens in Windows Credential Manager: {}",
                e
            ),
            occurred_at: now,
        })?;

        Ok(stored)
    }

    /// Restores session at startup from Windows Credential Manager and refreshes expired access tokens.
    pub async fn restore_or_refresh_session(
        &self,
    ) -> std::result::Result<Option<StoredTokenSet>, ClassifiedError> {
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

        if tokens.refresh_token.trim().is_empty() {
            return Err(ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: "Stored Google OAuth session expired and no refresh token is available. Please sign in again.".to_string(),
                occurred_at: now,
            });
        }

        // Refresh expired access token while preserving the refresh_token in Windows Credential Manager
        let refreshed = self.refresh_access_token(&tokens.refresh_token).await?;
        Ok(Some(refreshed))
    }

    pub async fn refresh_access_token(
        &self,
        refresh_token: &str,
    ) -> std::result::Result<StoredTokenSet, ClassifiedError> {
        let valid_client_id = self.validate_configured_client_id()?;
        let now = Utc::now().timestamp();
        if refresh_token.trim().is_empty() {
            return Err(ClassifiedError {
                kind: SyncErrorKind::Authentication,
                message: "Cannot refresh Google session: refresh token is empty. Please sign in again.".to_string(),
                occurred_at: now,
            });
        }

        let form_body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("client_id", &valid_client_id)
            .append_pair("refresh_token", refresh_token)
            .append_pair("grant_type", "refresh_token")
            .finish();

        let resp = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_body)
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

        let previous = StoredTokenSet {
            access_token: String::new(),
            refresh_token: refresh_token.to_string(),
            expires_at_unix: now,
            scope: YOUTUBE_SCOPES.to_string(),
        };
        let updated = merge_token_response(body, Some(&previous), now)?;

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
    use std::collections::HashMap;

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

    #[test]
    fn test_callback_state_validation_and_error_handling() {
        let code = parse_and_validate_callback_request(
            "GET /callback?code=4/0AeanS0xyz&state=expected_state_123 HTTP/1.1",
            "expected_state_123",
        )
        .expect("valid callback should return code");
        assert_eq!(code, "4/0AeanS0xyz");

        let mismatch_err = parse_and_validate_callback_request(
            "GET /callback?code=4/0AeanS0xyz&state=wrong_state HTTP/1.1",
            "expected_state_123",
        )
        .expect_err("state mismatch must fail");
        assert_eq!(mismatch_err.kind, SyncErrorKind::Authentication);
        assert!(mismatch_err.message.contains("state mismatch"));

        let provider_err = parse_and_validate_callback_request(
            "GET /callback?error=access_denied&error_description=User+denied+access&state=expected_state_123 HTTP/1.1",
            "expected_state_123",
        )
        .expect_err("provider error must fail");
        assert_eq!(provider_err.kind, SyncErrorKind::Authentication);
        assert!(provider_err.message.contains("access_denied"));
    }

    #[tokio::test]
    async fn test_loopback_callback_timeout_cancellation_and_success() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let timeout_res =
            wait_for_loopback_callback(listener, "state_abc", Duration::from_millis(40)).await;
        assert!(timeout_res.is_err());
        assert!(timeout_res.unwrap_err().message.contains("timed out"));

        // Verify cancellation immediately releases the bound port for a retry
        let listener_cancel = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let bound_port = listener_cancel.local_addr().unwrap().port();
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let cancel_handle = tokio::spawn(async move {
            wait_for_loopback_callback_cancellable(
                listener_cancel,
                "state_cancel",
                Duration::from_secs(60),
                Some(cancel_rx),
            )
            .await
        });
        let _ = cancel_tx.send(());
        let cancel_res = cancel_handle.await.unwrap();
        assert!(cancel_res.is_err());
        assert!(cancel_res.unwrap_err().message.contains("cancelled"));

        // Port must be free to rebind immediately for a retry
        let listener2 = TcpListener::bind(("127.0.0.1", bound_port)).await.unwrap();
        let addr = listener2.local_addr().unwrap();

        let client_task = tokio::spawn(async move {
            let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
            let req = "GET /callback?code=auth_code_999&state=state_ok HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
            stream.write_all(req.as_bytes()).await.unwrap();
        });

        let code = wait_for_loopback_callback(listener2, "state_ok", Duration::from_secs(2))
            .await
            .expect("should receive loopback auth code");
        let _ = client_task.await;
        assert_eq!(code, "auth_code_999");
    }

    #[test]
    fn test_token_exchange_request_fields_and_refresh_token_preservation() {
        let previous = StoredTokenSet {
            access_token: "old_access".to_string(),
            refresh_token: "existing_refresh_token_keep_me".to_string(),
            expires_at_unix: 1000,
            scope: YOUTUBE_SCOPES.to_string(),
        };

        let response_without_refresh = GoogleTokenResponse {
            access_token: "new_access_token".to_string(),
            expires_in: 3600,
            refresh_token: None,
            scope: None,
        };

        let merged = merge_token_response(response_without_refresh, Some(&previous), 2000)
            .expect("merge should succeed");
        assert_eq!(merged.access_token, "new_access_token");
        assert_eq!(merged.refresh_token, "existing_refresh_token_keep_me");
        assert_eq!(merged.expires_at_unix, 5600);

        let form = build_token_exchange_form(
            "client_123.apps.googleusercontent.com",
            "4/0AeanS0_auth_code",
            "http://127.0.0.1:8989/callback",
            "pkce_verifier_xyz_123",
        );
        let parsed_fields: HashMap<String, String> =
            url::form_urlencoded::parse(form.as_bytes())
                .into_owned()
                .collect();

        assert_eq!(
            parsed_fields.get("client_id").map(String::as_str),
            Some("client_123.apps.googleusercontent.com")
        );
        assert_eq!(
            parsed_fields.get("code").map(String::as_str),
            Some("4/0AeanS0_auth_code")
        );
        assert_eq!(
            parsed_fields.get("redirect_uri").map(String::as_str),
            Some("http://127.0.0.1:8989/callback")
        );
        assert_eq!(
            parsed_fields.get("code_verifier").map(String::as_str),
            Some("pkce_verifier_xyz_123")
        );
        assert_eq!(
            parsed_fields.get("grant_type").map(String::as_str),
            Some("authorization_code")
        );
        assert!(!parsed_fields.contains_key("client_secret"));
    }

    #[test]
    fn test_map_token_exchange_error_client_secret_missing() {
        let google_error_json = r#"{
            "error": "invalid_request",
            "error_description": "client_secret is missing."
        }"#;
        let err = map_token_exchange_error(
            400,
            google_error_json,
            "http://127.0.0.1:8989/callback",
        );
        assert_eq!(err.kind, SyncErrorKind::Authentication);
        assert!(err
            .message
            .contains("invalid_request (client_secret is missing.)"));
        assert!(err.message.contains("Web application"));
        assert!(err.message.contains("Desktop app"));
        assert!(err.message.contains("http://127.0.0.1:8989/callback"));
    }
}
