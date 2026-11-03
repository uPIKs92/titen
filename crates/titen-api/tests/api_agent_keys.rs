mod common;

use axum::{Router, body::Body, middleware};
use common::{body_to_json, send, test_app, test_pool, test_state};
use serde_json::{Value, json};

const MASTER: &str = "master-key-for-tests";

/// Router with the real auth middleware and a master key configured.
async fn secured() -> (Router, titen_api::server::AppState) {
    let mut state = test_state(test_pool().await);
    state.api_key = Some(MASTER.to_string());
    let app = test_app(state.clone()).route_layer(middleware::from_fn_with_state(
        state.clone(),
        titen_api::server::api_key_auth,
    ));
    (app, state)
}

fn req(method: &str, uri: &str, key: Option<&str>, body: Option<Value>) -> axum::http::Request<Body> {
    let mut b = axum::http::Request::builder().method(method).uri(uri);
    if let Some(k) = key {
        b = b.header("X-API-Key", k);
    }
    if body.is_some() {
        b = b.header("content-type", "application/json");
    }
    b.body(body.map(|v| Body::from(v.to_string())).unwrap_or_else(Body::empty)).unwrap()
}

async fn make_key(app: &Router, name: &str, scope: &str) -> (String, String) {
    let r = send(req("POST", "/api/agent-keys", Some(MASTER), Some(json!({"name":name,"scope":scope}))), app).await;
    assert_eq!(r.status(), 201);
    let j = body_to_json(r).await;
    (
        j["data"]["api_key"].as_str().unwrap().to_string(),
        j["data"]["key"]["id"].as_str().unwrap().to_string(),
    )
}

fn rpc(key: &str, body: Value) -> axum::http::Request<Body> {
    req("POST", "/api/mcp", Some(key), Some(body))
}

#[tokio::test]
async fn read_key_is_read_only_everywhere() {
    let (app, _) = secured().await;
    let (key, _) = make_key(&app, "analytics-bot", "read").await;
    assert!(key.starts_with("tk_"));

    assert_eq!(send(req("GET", "/api/accounts", Some(&key), None), &app).await.status(), 200);
    assert_eq!(send(req("POST", "/api/accounts", Some(&key), Some(json!({}))), &app).await.status(), 403);
    assert_eq!(send(req("GET", "/api/agent-keys", Some(&key), None), &app).await.status(), 403);
    assert_eq!(send(req("GET", "/api/settings", Some(&key), None), &app).await.status(), 403);

    // MCP: only read-only tools are listed, and write tools are refused.
    let j = body_to_json(send(rpc(&key, json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})), &app).await).await;
    let names: Vec<&str> = j["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"list_posts"));
    for w in ["create_post", "delete_post", "schedule_post", "connect_account"] {
        assert!(!names.contains(&w), "{w} must be hidden from read keys");
    }
    let j = body_to_json(send(rpc(&key, json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_post","arguments":{"account_id":"x","caption":"hi"}}})), &app).await).await;
    assert_eq!(j["result"]["isError"], true);
    assert!(j["result"]["content"][0]["text"].as_str().unwrap().contains("FORBIDDEN"));
}

#[tokio::test]
async fn write_key_gets_all_tools_but_not_admin() {
    let (app, _) = secured().await;
    let (key, _) = make_key(&app, "publisher", "write").await;

    let j = body_to_json(send(rpc(&key, json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})), &app).await).await;
    let names: Vec<&str> = j["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for t in ["create_post", "schedule_post", "connect_account", "list_posts"] {
        assert!(names.contains(&t), "{t} missing for write key");
    }
    assert_eq!(send(req("GET", "/api/agent-keys", Some(&key), None), &app).await.status(), 403);
    assert_eq!(send(req("POST", "/api/agent-keys", Some(&key), Some(json!({"name":"x","scope":"write"}))), &app).await.status(), 403);
    assert_ne!(send(req("POST", "/api/accounts", Some(&key), Some(json!({}))), &app).await.status(), 403);
}

#[tokio::test]
async fn revoked_unknown_and_missing_keys_are_rejected() {
    let (app, state) = secured().await;
    let (key, id) = make_key(&app, "temp", "write").await;
    assert_eq!(send(req("GET", "/api/accounts", Some(&key), None), &app).await.status(), 200);

    // last_used_at recorded
    let k = state.store.get_agent_key(&id).await.unwrap();
    assert!(k.last_used_at.is_some());

    let r = send(req("DELETE", &format!("/api/agent-keys/{id}"), Some(MASTER), None), &app).await;
    assert_eq!(r.status(), 200);
    assert_eq!(send(req("GET", "/api/accounts", Some(&key), None), &app).await.status(), 401);
    // Revoking twice -> 404.
    assert_eq!(send(req("DELETE", &format!("/api/agent-keys/{id}"), Some(MASTER), None), &app).await.status(), 404);

    assert_eq!(send(req("GET", "/api/accounts", Some("tk_deadbeef"), None), &app).await.status(), 401);
    assert_eq!(send(req("GET", "/api/accounts", Some("wrong"), None), &app).await.status(), 401);
    assert_eq!(send(req("GET", "/api/accounts", None, None), &app).await.status(), 401);
}

#[tokio::test]
async fn keys_are_stored_hashed_and_never_relisted() {
    let (app, state) = secured().await;
    let (key, _) = make_key(&app, "hash-check", "read").await;

    let listed = body_to_json(send(req("GET", "/api/agent-keys", Some(MASTER), None), &app).await).await;
    let text = listed.to_string();
    assert!(!text.contains(&key) && !text.contains("api_key") && !text.contains("key_hash"), "{text}");
    assert_eq!(listed["data"][0]["scope"], "read");
    assert!(listed["data"][0]["key_prefix"].as_str().unwrap().starts_with("tk_"));

    let stored: (String,) = sqlx::query_as("SELECT key_hash FROM agent_keys")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    assert_ne!(stored.0, key);
    assert_eq!(stored.0, titen_core::agent_keys::hash_key(&key));

    // Validation.
    let r = send(req("POST", "/api/agent-keys", Some(MASTER), Some(json!({"name":"x","scope":"admin"}))), &app).await;
    assert_eq!(r.status(), 400);
    let r = send(req("POST", "/api/agent-keys", Some(MASTER), Some(json!({"name":"  ","scope":"read"}))), &app).await;
    assert_eq!(r.status(), 400);
}

// Env var is process-wide: keep everything touching it in one test.
#[tokio::test]
async fn connect_account_tool_and_media_schedules() {
    unsafe { std::env::set_var("TITEN_OAUTH_REDIRECT_URI", "https://titen.test/auth/callback") };
    let (app, state) = secured().await;
    let (key, _) = make_key(&app, "connector", "write").await;
    let call = |name: &str, args: Value| {
        rpc(&key, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}))
    };
    let text = |j: &Value| j["result"]["content"][0]["text"].as_str().unwrap().to_string();

    // Missing creds -> clear error.
    let j = body_to_json(send(call("connect_account", json!({})), &app).await).await;
    assert_eq!(j["result"]["isError"], true);
    assert!(text(&j).contains("MISSING_APP_ID"));

    // With creds -> URL with client_id + state; state bound to the MASTER key
    // so the human finishing in the web UI can consume it.
    let j = body_to_json(send(call("connect_account", json!({"app_id":"123","app_secret":"sec"})), &app).await).await;
    assert!(j["result"].get("isError").is_none(), "{j}");
    let data: Value = serde_json::from_str(&text(&j)).unwrap();
    let url = data["authorize_url"].as_str().unwrap();
    assert!(url.contains("client_id=123") && url.contains("redirect_uri=https%3A%2F%2Ftiten.test%2Fauth%2Fcallback"), "{url}");
    let st = url.split("state=").nth(1).unwrap();
    assert!(!text(&j).contains("sec\""), "secret must not be echoed");
    assert!(state.store.consume_oauth_state(st, MASTER).await.unwrap(), "state must be bound to master key");

    // schedule_post: all media types validated like publishing.
    let acct = common::create_test_account(&test_app(state.clone()), state.store.pool()).await;
    let id = acct["id"].as_str().unwrap();
    let ok = |a: Value| async {
        let j = body_to_json(send(call("schedule_post", a), &app).await).await;
        j
    };
    let j = ok(json!({"account_id":id,"scheduled_at":"2030-01-01T09:00:00+07:00","caption":"hello"})).await;
    assert!(j["result"].get("isError").is_none(), "{j}");
    let j = ok(json!({"account_id":id,"scheduled_at":"2030-01-01T09:00:00Z","media_type":"IMAGE","image_url":"https://example.com/a.png"})).await;
    assert!(j["result"].get("isError").is_none(), "{j}");
    let j = ok(json!({"account_id":id,"scheduled_at":"2030-01-01T09:00:00Z","media_type":"VIDEO","video_url":"https://example.com/v.mp4","reply_to_id":"123"})).await;
    assert!(j["result"].get("isError").is_none(), "{j}");
    let j = ok(json!({"account_id":id,"scheduled_at":"2030-01-01T09:00:00Z","media_type":"CAROUSEL","media_urls":["https://e.com/1.png","https://e.com/2.png"]})).await;
    assert!(j["result"].get("isError").is_none(), "{j}");

    for (args, code) in [
        (json!({"account_id":id,"scheduled_at":"tomorrow","caption":"x"}), "INVALID_SCHEDULED_AT"),
        (json!({"account_id":id,"scheduled_at":"2030-01-01T09:00:00Z","media_type":"IMAGE"}), "INVALID_MEDIA_URL"),
        (json!({"account_id":id,"scheduled_at":"2030-01-01T09:00:00Z","media_type":"CAROUSEL","media_urls":["https://e.com/1.png"]}), "INVALID_CAROUSEL_COUNT"),
        (json!({"account_id":id,"scheduled_at":"2030-01-01T09:00:00Z","media_type":"IMAGE","image_url":"https://10.0.0.1/a.png"}), "INVALID_MEDIA_URL"),
        (json!({"account_id":"nope","scheduled_at":"2030-01-01T09:00:00Z","caption":"x"}), "ACCOUNT_NOT_FOUND"),
        (json!({"account_id":id,"scheduled_at":"2030-01-01T09:00:00Z"}), "EMPTY_POST"),
    ] {
        let j = ok(args.clone()).await;
        assert_eq!(j["result"]["isError"], true, "{args}");
        assert!(text(&j).contains(code), "{code}: {}", text(&j));
    }
}
