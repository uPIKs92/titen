//! MCP over Streamable HTTP (`POST /api/mcp`).
//!
//! Lets remote agents (Hermes, Claude, Cursor, …) use the same tools as the
//! stdio `titen-mcp` binary. Protocol and tool logic live in the `titen_mcp`
//! library; this module is only the transport. Auth is the normal API-key
//! middleware (`X-API-Key` header), so the live server's encryption key and
//! per-account Threads apps apply automatically.
//!
//! Supported: single or batched JSON-RPC requests answered as
//! `application/json`; notification-only posts answered `202 Accepted`.
//! Not supported: server-initiated SSE streams (`GET` → 405), sessions.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::server::{AgentAccess, AppState};

/// Hard cap on JSON-RPC batch size so one request can't fan out unboundedly.
const MAX_BATCH: usize = 20;

pub async fn post_mcp(
    State(state): State<AppState>,
    headers: HeaderMap,
    access: Option<axum::Extension<AgentAccess>>,
    body: axum::body::Bytes,
) -> Response {
    let opts = titen_mcp::McpOptions {
        access: access.map(|a| a.0.0).unwrap_or(titen_mcp::Access::Full),
        // Agents only start the flow; a human finishes it in the admin UI with
        // the master key / session, so the state is bound to the master key.
        oauth_caller: match state.api_key.as_deref() {
            Some(k) if !k.is_empty() => k.to_string(),
            _ => "dev".to_string(),
        },
        redirect_uri: crate::routes::settings::derive_redirect_uri(&headers),
    };
    // Spec: clients must accept JSON; we only ever reply with JSON.
    if let Some(ct) = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        if !ct.starts_with("application/json") {
            return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Content-Type must be application/json")
                .into_response();
        }
    }

    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return Json(titen_mcp::rpc_error(Value::Null, -32700, &format!("Parse error: {e}")))
                .into_response();
        }
    };

    match parsed {
        Value::Array(items) => {
            if items.is_empty() || items.len() > MAX_BATCH {
                return Json(titen_mcp::rpc_error(
                    Value::Null,
                    -32600,
                    &format!("Batch must contain 1..={MAX_BATCH} requests"),
                ))
                .into_response();
            }
            let mut out = Vec::new();
            for item in &items {
                if let Some(r) = dispatch(&state, item, &opts).await {
                    out.push(r);
                }
            }
            if out.is_empty() {
                StatusCode::ACCEPTED.into_response()
            } else {
                Json(Value::Array(out)).into_response()
            }
        }
        item @ Value::Object(_) => match dispatch(&state, &item, &opts).await {
            Some(r) => Json(r).into_response(),
            None => StatusCode::ACCEPTED.into_response(),
        },
        _ => Json(titen_mcp::rpc_error(Value::Null, -32600, "Invalid Request")).into_response(),
    }
}

async fn dispatch(state: &AppState, item: &Value, opts: &titen_mcp::McpOptions) -> Option<Value> {
    if !item.is_object() || item.get("method").and_then(|m| m.as_str()).is_none() {
        // Responses/garbage from the client: nothing to answer.
        let id = item.get("id").cloned().unwrap_or(Value::Null);
        return Some(titen_mcp::rpc_error(id, -32600, "Invalid Request"));
    }
    titen_mcp::handle_message(&state.store, &state.threads_client, item, opts).await
}

/// `GET`/`DELETE /api/mcp`: no server-initiated streams and no sessions.
pub async fn method_not_allowed() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "POST")],
        Json(json!({ "error": "This MCP endpoint is POST-only (stateless Streamable HTTP)" })),
    )
        .into_response()
}
