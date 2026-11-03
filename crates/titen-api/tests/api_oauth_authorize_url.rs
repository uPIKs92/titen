mod common;

use axum::body::Body;
use common::{body_to_json, create_test_account, send, test_app, test_pool, test_state};
use serde_json::{Value, json};

fn post_json(body: Value) -> axum::http::Request<Body> {
    axum::http::Request::builder()
        .method("POST")
        .uri("/api/oauth/authorize-url")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn put_json(uri: &str, body: Value) -> axum::http::Request<Body> {
    axum::http::Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

// One test: these cases share a process-wide env var (redirect URI).
#[tokio::test]
async fn authorize_url_uses_per_account_credentials() {
    let pool = test_pool().await;
    let state = test_state(pool.clone());
    let store = state.store.clone();
    let app = test_app(state);

    // No credentials anywhere -> MISSING_APP_ID.
    let resp = send(post_json(json!({})), &app).await;
    assert_eq!(resp.status(), 400);
    assert_eq!(body_to_json(resp).await["code"], "MISSING_APP_ID");

    // SAFETY: only this test touches the variable in this test binary.
    unsafe {
        std::env::set_var(
            "TITEN_OAUTH_REDIRECT_URI",
            "https://titen.example/auth/callback",
        );
    }

    // App id without a secret -> MISSING_APP_SECRET.
    let resp = send(post_json(json!({ "app_id": "111" })), &app).await;
    assert_eq!(resp.status(), 400);
    assert_eq!(body_to_json(resp).await["code"], "MISSING_APP_SECRET");

    // Explicit credentials: URL uses that app, and the state carries the pair.
    let resp = send(
        post_json(json!({ "app_id": "111", "app_secret": "sec-111" })),
        &app,
    )
    .await;
    assert_eq!(resp.status(), 200);
    let data = body_to_json(resp).await["data"].clone();
    let url = data["authorize_url"].as_str().unwrap();
    let st = data["state"].as_str().unwrap();
    assert!(url.starts_with("https://threads.net/oauth/authorize?client_id=111&"));
    assert!(url.contains("redirect_uri=https%3A%2F%2Ftiten.example%2Fauth%2Fcallback"));
    assert!(url.ends_with(&format!("&state={st}")));
    assert_eq!(
        store.get_oauth_state_app(st).await.unwrap(),
        Some(("111".to_string(), "sec-111".to_string()))
    );
    assert!(store.consume_oauth_state(st, "dev").await.unwrap());
    assert_eq!(store.get_oauth_state_app(st).await.unwrap(), None);

    // Two accounts, each with its OWN app, set via PUT /api/accounts/{id}.
    let acc_a = create_test_account(&app, &pool).await;
    let id_a = acc_a["id"].as_str().unwrap().to_string();
    assert_eq!(acc_a["app_secret_set"], false);
    let resp = send(
        put_json(
            &format!("/api/accounts/{id_a}"),
            json!({ "app_id": "AAA", "app_secret": "sec-AAA" }),
        ),
        &app,
    )
    .await;
    assert_eq!(resp.status(), 200);
    let upd = body_to_json(resp).await["data"].clone();
    assert_eq!(upd["app_id"], "AAA");
    assert_eq!(upd["app_secret_set"], true);
    assert!(upd.get("app_secret").is_none(), "secret must never be returned");

    let (acc_b, _) = store
        .upsert_account(&titen_core::models::CreateAccount {
            username: Some("second".to_string()),
            user_id: Some("user_456".to_string()),
            access_token: "tok".to_string(),
            expires_at: "2099-12-31T00:00:00Z".to_string(),
            app_id: Some("BBB".to_string()),
            app_secret: Some("sec-BBB".to_string()),
        })
        .await
        .unwrap();

    // Reconnect each account: its own app is used.
    for (id, app_id, secret) in [(&id_a, "AAA", "sec-AAA"), (&acc_b.id, "BBB", "sec-BBB")] {
        let resp = send(post_json(json!({ "account_id": id })), &app).await;
        assert_eq!(resp.status(), 200);
        let data = body_to_json(resp).await["data"].clone();
        assert_eq!(data["app_id"], app_id);
        let st = data["state"].as_str().unwrap();
        assert_eq!(
            store.get_oauth_state_app(st).await.unwrap(),
            Some((app_id.to_string(), secret.to_string()))
        );
    }

    // A different app_id never inherits another account's secret.
    let resp = send(
        post_json(json!({ "account_id": id_a, "app_id": "CCC" })),
        &app,
    )
    .await;
    assert_eq!(resp.status(), 400);
    assert_eq!(body_to_json(resp).await["code"], "MISSING_APP_SECRET");

    // Unknown account -> 404.
    let resp = send(post_json(json!({ "account_id": "nope" })), &app).await;
    assert_eq!(resp.status(), 404);
}
