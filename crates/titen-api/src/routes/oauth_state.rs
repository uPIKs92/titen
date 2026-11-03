// #237 — OAuth state parameter (CSRF protection)
//
// The Threads OAuth flow previously sent no `state` parameter, so the
// `/auth/callback` page could be driven by an attacker-crafted callback URL
// (login CSRF). This module adds a one-time state token:
//
//   1. `POST /api/oauth/state` (authenticated) — generates a 256-bit random
//      token, stores it bound to the caller's API key / session, and returns
//      `{ state }`. TTL: 10 minutes, single use.
//   2. `POST /api/oauth/exchange` (authenticated) — now requires `state` and
//      validates it: must exist, belong to the caller, be unexpired, and is
//      atomically deleted on success (replay impossible).

use axum::{Json, extract::State, http::HeaderMap, http::StatusCode};
use getrandom::fill as getrandom_fill;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};

use crate::server::AppState;

/// OAuth state tokens live 10 minutes — enough to complete a login redirect,
/// short enough that stale tokens expire quickly.
pub const STATE_TTL_SECS: i64 = 600;

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Resolve the caller identity for state binding from request headers,
/// mirroring `api_key_auth`: raw `X-API-Key` header first, then the session
/// cookie resolved through the session store. Returns `None` when neither is
/// present (the auth middleware would have rejected the request anyway; dev
/// mode with no configured key yields `Some("dev")` at the call site).
pub async fn caller_key_from_headers(_state: &AppState, headers: &HeaderMap) -> Option<String> {
    if let Some(k) = headers
        .get("X-API-Key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
    {
        return Some(k);
    }
    let cookie_token = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .map(|c| c.trim())
                .find(|c| c.starts_with("titen_session="))
                .map(|c| c.trim_start_matches("titen_session=").to_string())
        });
    match cookie_token {
        Some(ref token) => crate::routes::auth::validate_session(token).await,
        None => None,
    }
}

/// Generate and persist a one-time state token bound to the caller.
/// Shared by `POST /api/oauth/state` and `POST /api/oauth/authorize-url`.
async fn issue_state(
    state: &AppState,
    headers: &HeaderMap,
    app: Option<(&str, &str)>,
) -> Result<String, (StatusCode, serde_json::Value)> {
    let required_key = state.api_key.clone().unwrap_or_default();
    let caller = if required_key.is_empty() {
        // Dev mode (no key configured): auth middleware lets everyone through.
        "dev".to_string()
    } else {
        match caller_key_from_headers(state, headers).await {
            Some(k) => k,
            None => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    serde_json::json!({
                        "error": "Invalid or missing API key",
                        "code": "UNAUTHORIZED"
                    }),
                ));
            }
        }
    };

    let mut buf = [0u8; 32]; // 256 bits of entropy
    if let Err(e) = getrandom_fill(&mut buf) {
        warn!(target: "titen::oauth", "OAUTH_STATE_FAIL rng: {e}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            serde_json::json!({
                "error": "Failed to generate OAuth state token.",
                "code": "STATE_GENERATION_FAILED"
            }),
        ));
    }
    let state_value = hex::encode(buf);

    let expires_at = now_epoch() + STATE_TTL_SECS;
    let inserted = match app {
        Some((app_id, app_secret)) => {
            state
                .store
                .insert_oauth_state_with_app(&state_value, &caller, expires_at, app_id, app_secret)
                .await
        }
        None => {
            state
                .store
                .insert_oauth_state(&state_value, &caller, expires_at)
                .await
        }
    };
    if let Err(e) = inserted {
        warn!(target: "titen::oauth", "OAUTH_STATE_FAIL store: {e}");
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            serde_json::json!({
                "error": "Failed to persist OAuth state token.",
                "code": "STATE_STORE_FAILED"
            }),
        ));
    }

    // Opportunistic cleanup of expired states (cheap: indexed column).
    let _ = state.store.prune_expired_oauth_states().await;

    info!(target: "titen::oauth", "OAUTH_STATE_ISSUED");
    Ok(state_value)
}

/// Issue a one-time OAuth state token bound to the caller.
pub async fn create_state(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (StatusCode, Json<serde_json::Value>) {
    match issue_state(&state, &headers, None).await {
        Ok(state_value) => (
            StatusCode::OK,
            Json(serde_json::json!({ "state": state_value })),
        ),
        Err((status, body)) => (status, Json(body)),
    }
}

/// Optional body for `POST /api/oauth/authorize-url`.
///
/// Each account is connected through its own Threads app. Credentials come
/// from, in order: the request (`app_id` + `app_secret`), the account named by
/// `account_id` (reconnect, or "copy from"), then the legacy global settings.
#[derive(serde::Deserialize, Default)]
pub struct AuthorizeUrlRequest {
    pub redirect_uri: Option<String>,
    /// Threads app ID for this connection.
    pub app_id: Option<String>,
    /// Threads app secret for this connection (stored on the account afterwards).
    pub app_secret: Option<String>,
    /// Existing account whose stored app credentials supply any missing values.
    pub account_id: Option<String>,
}

/// `POST /api/oauth/authorize-url` — one call to start the Threads connect flow.
///
/// Resolves the Threads app credentials for this connection, issues a
/// caller-bound state token that carries them across the redirect, and returns
/// the full authorize URL. The user opens it, clicks Authorize, and Threads
/// redirects to the web UI's `/auth/callback`, which exchanges the code with
/// those same credentials and saves them on the account.
pub async fn authorize_url(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<AuthorizeUrlRequest>>,
) -> (StatusCode, Json<serde_json::Value>) {
    let err = |status: StatusCode, code: &str, msg: &str| {
        (
            status,
            Json(serde_json::json!({ "error": msg, "code": code })),
        )
    };
    let req = body.map(|Json(r)| r).unwrap_or_default();
    let non_empty = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());

    let mut app_id = non_empty(req.app_id);
    let mut app_secret = req.app_secret.filter(|s| !s.is_empty());

    // Inherit from an existing account. A different app_id never inherits that
    // account's secret (an id/secret pair from two apps can't work).
    if let Some(account_id) = non_empty(req.account_id) {
        match state.store.get_account(&account_id).await {
            Ok(acc) => {
                let same_app = app_id.is_none() || app_id == acc.app_id;
                if same_app {
                    if app_id.is_none() {
                        app_id = acc.app_id.clone().filter(|v| !v.is_empty());
                    }
                    if app_secret.is_none() {
                        app_secret = acc.app_secret.clone().filter(|v| !v.is_empty());
                    }
                }
            }
            Err(_) => {
                return err(
                    StatusCode::NOT_FOUND,
                    "ACCOUNT_NOT_FOUND",
                    "No such account to take Threads app credentials from.",
                );
            }
        }
    }

    // Legacy fallback: the instance-wide pair, only when nothing else was given.
    if app_id.is_none() && app_secret.is_none() {
        if let Ok(settings) = state.store.get_app_settings().await {
            app_id = settings.threads_app_id.filter(|v| !v.is_empty());
        }
        if app_id.is_some() {
            app_secret = state
                .store
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
            return err(
                StatusCode::BAD_REQUEST,
                "MISSING_APP_ID",
                "Threads App ID is required for this account.",
            );
        }
        (Some(_), None) => {
            return err(
                StatusCode::BAD_REQUEST,
                "MISSING_APP_SECRET",
                "Threads App Secret is required for this account.",
            );
        }
    };

    // Server-derived URI (env-configured) wins. The optional body value is only
    // a fallback for when the server can't derive one; Meta still enforces its
    // own redirect-URI whitelist, and the exchange step already accepts a
    // caller-supplied redirect_uri.
    let mut redirect_uri = super::settings::derive_redirect_uri(&headers);
    if redirect_uri.is_empty() {
        redirect_uri = req.redirect_uri.unwrap_or_default();
    }
    if redirect_uri.is_empty() {
        return err(
            StatusCode::BAD_REQUEST,
            "MISSING_REDIRECT_URI",
            "Cannot derive the OAuth redirect URI. Set APP_URL or TITEN_OAUTH_REDIRECT_URI.",
        );
    }

    let state_value = match issue_state(&state, &headers, Some((&app_id, &app_secret))).await {
        Ok(v) => v,
        Err((status, body)) => return (status, Json(body)),
    };
    let url = format!(
        "{}&state={}",
        super::settings::build_authorize_url(&app_id, &redirect_uri),
        super::settings::urlencode(&state_value)
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "data": {
                "authorize_url": url,
                "state": state_value,
                "redirect_uri": redirect_uri,
                "app_id": app_id,
                "expires_in": STATE_TTL_SECS,
            }
        })),
    )
}

/// Consume a state token: validates owner + expiry and deletes it
/// atomically (single use). Returns Ok(()) when valid.
pub async fn consume_state(
    state: &AppState,
    caller: &str,
    token: &str,
) -> Result<(), (StatusCode, serde_json::Value)> {
    let deleted = state
        .store
        .consume_oauth_state(token, caller)
        .await
        .map_err(|e| {
            warn!(target: "titen::oauth", "OAUTH_STATE_VALIDATE_FAIL store: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({
                    "error": "Failed to validate OAuth state token.",
                    "code": "STATE_STORE_FAILED"
                }),
            )
        })?;
    if deleted {
        Ok(())
    } else {
        warn!(target: "titen::oauth", "OAUTH_STATE_REJECTED invalid/missing/expired state");
        Err((
            StatusCode::BAD_REQUEST,
            serde_json::json!({
                "error": "Invalid, expired, or already-used OAuth state token. Restart the connection flow.",
                "code": "INVALID_OAUTH_STATE"
            }),
        ))
    }
}

/// Validates AND consumes the `state` on an exchange request, atomically.
/// Used for the legacy `{code, app_id, app_secret, redirect_uri}` clients:
/// they cannot know the stored binding, so any state that is unexpired and
/// not yet consumed is accepted exactly once — the DELETE is the authority,
/// so concurrent exchanges race on it and exactly one caller wins. Still
/// unguessable at 256-bit. Returns the error response to send on failure.
pub async fn validate_state_shallow(
    state: &AppState,
    token: &str,
) -> Result<(), (StatusCode, serde_json::Value)> {
    let consumed = state
        .store
        .consume_any_oauth_state(token)
        .await
        .map_err(|e| {
            warn!(target: "titen::oauth", "OAUTH_STATE_VALIDATE_FAIL store: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({
                    "error": "Failed to validate OAuth state token.",
                    "code": "STATE_STORE_FAILED"
                }),
            )
        })?;
    if consumed {
        Ok(())
    } else {
        warn!(target: "titen::oauth", "OAUTH_STATE_REJECTED legacy invalid/missing/expired state");
        Err((
            StatusCode::BAD_REQUEST,
            serde_json::json!({
                "error": "Invalid, expired, or already-used OAuth state token. Restart the connection flow.",
                "code": "INVALID_OAUTH_STATE"
            }),
        ))
    }
}
