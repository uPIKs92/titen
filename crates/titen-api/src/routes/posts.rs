use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::server::AppState;
use titen_core::models::*;

#[utoipa::path(
    get,
    path = "/api/posts",
    tag = "posts",
    params(("filter" = Option<PostFilter>, Query, description = "Post filter")),
    responses(
        (status = 200, description = "List of posts", body = serde_json::Value),
    ),
    security(("api_key" = [])),
)]
pub async fn list_posts(
    State(state): State<AppState>,
    Query(filter): Query<PostFilter>,
) -> Json<serde_json::Value> {
    match state.store.list_posts(&filter).await {
        Ok(posts) => Json(serde_json::json!({ "data": posts })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "code": "LIST_FAILED" })),
    }
}

#[utoipa::path(
    get,
    path = "/api/posts/{id}",
    tag = "posts",
    params(("id" = String, Path, description = "Post ID")),
    responses(
        (status = 200, description = "Post details", body = serde_json::Value),
        (status = 404, description = "Not found", body = serde_json::Value),
    ),
    security(("api_key" = [])),
)]
pub async fn get_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    match state.store.get_post(&id).await {
        Ok(post) => Json(serde_json::json!({ "data": post })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "code": "NOT_FOUND" })),
    }
}

#[utoipa::path(
    post,
    path = "/api/posts",
    tag = "posts",
    request_body = CreatePost,
    responses(
        (status = 201, description = "Post created", body = serde_json::Value),
        (status = 400, description = "Bad request", body = serde_json::Value),
        (status = 404, description = "Account not found", body = serde_json::Value),
        (status = 429, description = "Rate limited", body = serde_json::Value),
        (status = 502, description = "Threads API error", body = serde_json::Value),
    ),
    security(("api_key" = [])),
)]
pub async fn create_post(
    State(state): State<AppState>,
    Json(input): Json<CreatePost>,
) -> (StatusCode, Json<serde_json::Value>) {
    match titen_core::publisher::create_and_publish(&state.store, &state.threads_client, input)
        .await
    {
        Ok(post) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "data": post })),
        ),
        Err(f) => (
            StatusCode::from_u16(f.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(serde_json::json!({ "error": f.message, "code": f.code })),
        ),
    }
}

#[utoipa::path(
    delete,
    path = "/api/posts/{id}",
    tag = "posts",
    params(("id" = String, Path, description = "Post ID")),
    responses(
        (status = 200, description = "Post deleted", body = serde_json::Value),
        (status = 404, description = "Not found", body = serde_json::Value),
    ),
    security(("api_key" = [])),
)]
pub async fn delete_post(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let post = match state.store.get_post(&id).await {
        Ok(p) => p,
        Err(e) => return Json(serde_json::json!({ "error": e.to_string(), "code": "NOT_FOUND" })),
    };

    if let Some(threads_post_id) = &post.threads_post_id {
        if let Ok(account) = state.store.get_account(&post.account_id).await {
            if let Err(e) = state
                .threads_client
                .delete_post(&account, threads_post_id)
                .await
            {
                tracing::warn!(
                    "Failed to delete Threads post {threads_post_id} for post {id}: {e}"
                );
            }
        }
    }

    match state.store.delete_post(&id).await {
        Ok(()) => Json(serde_json::json!({ "data": null })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "code": "DELETE_FAILED" })),
    }
}

#[utoipa::path(
    get,
    path = "/api/posts/{id}/insights",
    tag = "posts",
    params(("id" = String, Path, description = "Post ID")),
    responses(
        (status = 200, description = "Post insights", body = serde_json::Value),
        (status = 404, description = "Not found", body = serde_json::Value),
    ),
    security(("api_key" = [])),
)]
pub async fn get_insights(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let post = match state.store.get_post(&id).await {
        Ok(p) => p,
        Err(e) => return Json(serde_json::json!({ "error": e.to_string(), "code": "NOT_FOUND" })),
    };

    let threads_post_id = match &post.threads_post_id {
        Some(id) => id,
        None => {
            return Json(
                serde_json::json!({ "error": "Post not yet published to Threads", "code": "NOT_PUBLISHED" }),
            );
        }
    };

    match state.store.get_account(&post.account_id).await {
        Ok(account) => {
            match state
                .threads_client
                .fetch_insights(&account, threads_post_id, None)
                .await
            {
                Ok(insights) => {
                    // Store snapshot
                    let snap_id = Uuid::now_v7().to_string();
                    let insights_model: titen_core::models::Insights = insights.into();
                    if let Err(e) = state
                        .store
                        .insert_analytics_snap(&snap_id, &id, &insights_model)
                        .await
                    {
                        tracing::warn!("Failed to store analytics snapshot for post {id}: {e}");
                    }
                    Json(serde_json::json!({ "data": insights_model }))
                }
                Err(e) => {
                    Json(serde_json::json!({ "error": e.to_string(), "code": "INSIGHTS_FAILED" }))
                }
            }
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string(), "code": "ACCOUNT_NOT_FOUND" })),
    }
}
