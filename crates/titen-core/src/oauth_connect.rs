//! Starting the Threads OAuth connect flow (shared by `POST /api/oauth/authorize-url`
//! and the MCP `connect_account` tool).
//!
//! Each account is connected through its own Threads app. Credentials come
//! from, in order: the call (`app_id` + `app_secret`), the account named by
//! `account_id` (reconnect / "reuse that app"), then the legacy global settings.
//! The returned URL is opened by a human; Threads redirects to the web UI's
//! `/auth/callback`, which completes the exchange.

use crate::{Store, publisher::PostFailure};

/// OAuth state tokens live 10 minutes.
pub const STATE_TTL_SECS: i64 = 600;

pub type ConnectError = PostFailure;

#[derive(Debug, Clone)]
pub struct ConnectStart {
    pub authorize_url: String,
    pub state: String,
    pub redirect_uri: String,
    pub app_id: String,
    pub expires_in: i64,
}

pub fn build_authorize_url(app_id: &str, redirect_uri: &str) -> String {
    format!(
        "https://threads.net/oauth/authorize?client_id={}&redirect_uri={}&scope=threads_basic,threads_content_publish,threads_manage_replies,threads_manage_mentions,threads_keyword_search,threads_profile_discovery,threads_share_to_instagram,threads_location_tagging&response_type=code",
        urlencode(app_id),
        urlencode(redirect_uri),
    )
}

/// Minimal percent-encoder for OAuth URL parameters.
pub fn urlencode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                result.push(byte as char);
            }
            _ => result.push_str(&format!("%{byte:02X}")),
        }
    }
    result
}

/// The redirect URI configured on the server (`TITEN_OAUTH_REDIRECT_URI`, else
/// `APP_URL` + `/auth/callback`). Empty when neither is usable. The API layer
/// additionally supports an allow-listed Host header.
pub fn configured_redirect_uri() -> String {
    if let Ok(uri) = std::env::var("TITEN_OAUTH_REDIRECT_URI") {
        if !uri.is_empty() {
            return uri;
        }
    }
    if let Ok(app_url) = std::env::var("APP_URL") {
        let app_url = app_url.trim_end_matches('/');
        let is_localhost =
            app_url.starts_with("http://localhost") || app_url.starts_with("http://127.0.0.1");
        if !app_url.is_empty() && (app_url.starts_with("https://") || is_localhost) {
            return format!("{app_url}/auth/callback");
        }
    }
    String::new()
}

/// Resolve credentials, issue a one-time state bound to `caller`, and build
/// the authorize URL. `redirect_uri` may be empty → `MISSING_REDIRECT_URI`.
pub async fn start_connect(
    store: &Store,
    caller: &str,
    redirect_uri: &str,
    app_id: Option<String>,
    app_secret: Option<String>,
    account_id: Option<String>,
) -> Result<ConnectStart, ConnectError> {
    let non_empty = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let mut app_id = non_empty(app_id);
    let mut app_secret = app_secret.filter(|s| !s.is_empty());

    // Inherit from an existing account. A different app_id never inherits that
    // account's secret (an id/secret pair from two apps can't work).
    if let Some(account_id) = non_empty(account_id) {
        let acc = store.get_account(&account_id).await.map_err(|_| {
            ConnectError::new(
                404,
                "ACCOUNT_NOT_FOUND",
                "No such account to take Threads app credentials from.",
            )
        })?;
        if app_id.is_none() || app_id == acc.app_id {
            if app_id.is_none() {
                app_id = acc.app_id.clone().filter(|v| !v.is_empty());
            }
            if app_secret.is_none() {
                app_secret = acc.app_secret.clone().filter(|v| !v.is_empty());
            }
        }
    }

    // Legacy fallback: the instance-wide pair, only when nothing else was given.
    if app_id.is_none() && app_secret.is_none() {
        if let Ok(settings) = store.get_app_settings().await {
            app_id = settings.threads_app_id.filter(|v| !v.is_empty());
        }
        if app_id.is_some() {
            app_secret = store
                .get_threads_app_secret()
                .await
                .ok()
                .flatten()
                .filter(|v| !v.is_empty());
        }
    }

    let (app_id, app_secret) = match (app_id, app_secret) {
        (Some(id), Some(secret)) => (id, secret),
        (None, _) => {
            return Err(ConnectError::new(
                400,
                "MISSING_APP_ID",
                "Threads App ID is required for this account.",
            ));
        }
        (Some(_), None) => {
            return Err(ConnectError::new(
                400,
                "MISSING_APP_SECRET",
                "Threads App Secret is required for this account.",
            ));
        }
    };

    if redirect_uri.is_empty() {
        return Err(ConnectError::new(
            400,
            "MISSING_REDIRECT_URI",
            "Cannot derive the OAuth redirect URI. Set APP_URL or TITEN_OAUTH_REDIRECT_URI.",
        ));
    }

    let mut buf = [0u8; 32]; // 256 bits
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut buf);
    let state = hex::encode(buf);
    let expires_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        + STATE_TTL_SECS;

    store
        .insert_oauth_state_with_app(&state, caller, expires_at, &app_id, &app_secret)
        .await
        .map_err(|e| {
            tracing::warn!(target: "titen::oauth", "OAUTH_STATE_FAIL store: {e}");
            ConnectError::new(500, "STATE_STORE_FAILED", "Failed to persist OAuth state token.")
        })?;
    let _ = store.prune_expired_oauth_states().await;

    let authorize_url = format!(
        "{}&state={}",
        build_authorize_url(&app_id, redirect_uri),
        urlencode(&state)
    );
    Ok(ConnectStart {
        authorize_url,
        state,
        redirect_uri: redirect_uri.to_string(),
        app_id,
        expires_in: STATE_TTL_SECS,
    })
}
