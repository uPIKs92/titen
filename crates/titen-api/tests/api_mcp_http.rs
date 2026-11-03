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

async fn create_post_err(app: &axum::Router, args: Value) -> String {
    let j = body_to_json(
        send(
            rpc(json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
                "params":{"name":"create_post","arguments":args}})),
            app,
        )
        .await,
    )
    .await;
    assert_eq!(j["result"]["isError"], true, "{j}");
    j["result"]["content"][0]["text"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn create_post_validates_before_publishing() {
    let (app, pool) = app().await;
    let account = create_test_account(&app, &pool).await;
    let id = account["id"].as_str().unwrap();

    // Unknown account -> clear code, not a FK error.
    let t = create_post_err(&app, json!({"account_id":"nope","caption":"hi"})).await;
    assert!(t.contains("ACCOUNT_NOT_FOUND"), "{t}");

    // TEXT needs a caption.
    let t = create_post_err(&app, json!({"account_id":id})).await;
    assert!(t.contains("EMPTY_POST"), "{t}");

    // Caption limit.
    let t = create_post_err(&app, json!({"account_id":id,"caption":"x".repeat(500)})).await;
    assert!(t.contains("CAPTION_TOO_LONG"), "{t}");

    // IMAGE / VIDEO need a public https URL; internal hosts are blocked (SSRF).
    let t = create_post_err(&app, json!({"account_id":id,"media_type":"IMAGE"})).await;
    assert!(t.contains("INVALID_MEDIA_URL"), "{t}");
    let t = create_post_err(&app, json!({"account_id":id,"media_type":"image","image_url":"http://example.com/a.png"})).await;
    assert!(t.contains("INVALID_MEDIA_URL") && t.contains("HTTPS"), "{t}");
    let t = create_post_err(&app, json!({"account_id":id,"media_type":"VIDEO","video_url":"https://127.0.0.1/v.mp4"})).await;
    assert!(t.contains("INVALID_MEDIA_URL") && t.contains("nternal"), "{t}");

    // CAROUSEL needs 2-20 images.
    let t = create_post_err(&app, json!({"account_id":id,"media_type":"CAROUSEL","image_urls":["https://example.com/a.png"]})).await;
    assert!(t.contains("INVALID_CAROUSEL_COUNT"), "{t}");

    // Unknown type.
    let t = create_post_err(&app, json!({"account_id":id,"media_type":"GIF","caption":"x"})).await;
    assert!(t.contains("UNSUPPORTED_MEDIA_TYPE"), "{t}");
}

#[tokio::test]
async fn create_post_schema_advertises_all_media_types() {
    let (app, _) = app().await;
    let j = body_to_json(send(rpc(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})), &app).await).await;
    let t = j["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "create_post").unwrap();
    let props = &t["inputSchema"]["properties"];
    assert_eq!(props["media_type"]["enum"], json!(["TEXT","IMAGE","VIDEO","CAROUSEL"]));
    for k in ["video_url","image_urls","media_ids","reply_to_id"] {
        assert!(props.get(k).is_some(), "missing {k}");
    }
    assert_eq!(t["inputSchema"]["required"], json!(["account_id"]));
}
