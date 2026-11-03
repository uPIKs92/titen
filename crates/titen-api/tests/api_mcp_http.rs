mod common;

use axum::body::Body;
use common::{body_to_json, create_test_account, send, test_app, test_pool, test_state};
use serde_json::{Value, json};

fn rpc(body: Value) -> axum::http::Request<Body> {
    axum::http::Request::builder()
        .method("POST")
        .uri("/api/mcp")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn app() -> (axum::Router, sqlx::SqlitePool) {
    let pool = test_pool().await;
    (test_app(test_state(pool.clone())), pool)
}

#[tokio::test]
async fn initialize_ping_and_tools_list() {
    let (app, _) = app().await;

    let r = send(
        rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}})),
        &app,
    )
    .await;
    assert_eq!(r.status(), 200);
    let j = body_to_json(r).await;
    assert_eq!(j["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(j["result"]["serverInfo"]["name"], "titen");

    let j = body_to_json(send(rpc(json!({"jsonrpc":"2.0","id":2,"method":"ping"})), &app).await).await;
    assert_eq!(j["result"], json!({}));

    let j = body_to_json(send(rpc(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"})), &app).await).await;
    assert!(j["result"]["tools"].as_array().unwrap().len() >= 29);
}

#[tokio::test]
async fn notification_is_202_and_errors_are_jsonrpc() {
    let (app, _) = app().await;

    let r = send(rpc(json!({"jsonrpc":"2.0","method":"notifications/initialized"})), &app).await;
    assert_eq!(r.status(), 202);

    let j = body_to_json(send(rpc(json!({"jsonrpc":"2.0","id":"x","method":"nope"})), &app).await).await;
    assert_eq!(j["id"], "x");
    assert_eq!(j["error"]["code"], -32601);

    let bad = axum::http::Request::builder()
        .method("POST")
        .uri("/api/mcp")
        .header("content-type", "application/json")
        .body(Body::from("{oops"))
        .unwrap();
    let j = body_to_json(send(bad, &app).await).await;
    assert_eq!(j["error"]["code"], -32700);
    assert_eq!(j["id"], Value::Null);

    let wrong_ct = axum::http::Request::builder()
        .method("POST")
        .uri("/api/mcp")
        .header("content-type", "text/plain")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(send(wrong_ct, &app).await.status(), 415);

    let get = axum::http::Request::builder().method("GET").uri("/api/mcp").body(Body::empty()).unwrap();
    assert_eq!(send(get, &app).await.status(), 405);
}

#[tokio::test]
async fn batch_and_tool_call_use_the_shared_store() {
    let (app, pool) = app().await;
    let account = create_test_account(&app, &pool).await;
    let account_id = account["id"].as_str().unwrap().to_string();

    let r = send(
        rpc(json!([
            {"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_accounts","arguments":{}}},
            {"jsonrpc":"2.0","method":"notifications/initialized"},
            {"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nope","arguments":{}}}
        ])),
        &app,
    )
    .await;
    let arr = body_to_json(r).await;
    let arr = arr.as_array().unwrap();
    assert_eq!(arr.len(), 2, "notification omitted from batch reply");
    let text = arr[0]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains(&account_id) && text.contains("testuser"), "{text}");
    assert!(!text.contains("access_token"), "tokens must not leak: {text}");
    assert_eq!(arr[1]["result"]["isError"], true);

    // Empty and oversized batches are rejected.
    let j = body_to_json(send(rpc(json!([])), &app).await).await;
    assert_eq!(j["error"]["code"], -32600);
}
