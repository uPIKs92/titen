//! Admin management of per-agent API keys (`/api/agent-keys`).
//!
//! Only the master key / admin session may call these (agent keys are barred
//! by the auth middleware). The plaintext key is returned once, on creation.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use titen_core::agent_keys::KeyScope;

use crate::server::AppState;

#[derive(serde::Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    /// `read` or `write`.
    pub scope: String,
}

pub async fn list_keys(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match state.store.list_agent_keys().await {
        Ok(keys) => (StatusCode::OK, Json(serde_json::json!({ "data": keys }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string(), "code": "LIST_FAILED" })),
        ),
    }
}

pub async fn create_key(
    State(state): State<AppState>,
    Json(input): Json<CreateKeyRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let Some(scope) = KeyScope::parse(input.scope.trim()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "scope must be 'read' or 'write'", "code": "INVALID_SCOPE" })),
        );
    };
    match state.store.create_agent_key(&input.name, scope).await {
        Ok((key, plaintext)) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "data": { "key": key, "api_key": plaintext } })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string(), "code": "CREATE_FAILED" })),
        ),
    }
}

pub async fn revoke_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> (StatusCode, Json<serde_json::Value>) {
    match state.store.revoke_agent_key(&id).await {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({ "data": null }))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Key not found or already revoked", "code": "NOT_FOUND" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string(), "code": "REVOKE_FAILED" })),
        ),
    }
}
