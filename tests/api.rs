use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::json;
use shortener::{build_router, build_state, Config};
use tower::ServiceExt;

const KEY: &str = "test-key-0123456789abcdef";
const BASE: &str = "https://sho.rt";

async fn test_app() -> (Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        bind: "127.0.0.1:0".parse().unwrap(),
        public_base_url: BASE.to_string(),
        api_keys: vec![KEY.to_string()],
        db_path: dir.path().join("test.db").to_string_lossy().to_string(),
        slug_len: 7,
    };
    let state = build_state(cfg).await.unwrap();
    (build_router(state), dir)
}

async fn send(
    app: &Router,
    req: Request<Body>,
) -> (StatusCode, serde_json::Value, axum::http::HeaderMap) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, value, headers)
}

fn create_req(body: serde_json::Value, key: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/links")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(k) = key {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {k}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

fn get(uri: &str, key: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(k) = key {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {k}"));
    }
    builder.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn creates_link_and_round_trips() {
    let (app, _dir) = test_app().await;

    let (status, body, _) = send(
        &app,
        create_req(json!({"url": "example.com/docs"}), Some(KEY)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["url"], "https://example.com/docs");
    let slug = body["slug"].as_str().unwrap().to_string();
    assert_eq!(slug.len(), 7);
    assert_eq!(body["short_url"], format!("{BASE}/{slug}"));
    assert!(body["created_at"].as_str().unwrap().ends_with('Z'));

    // Follow the short link.
    let res = app
        .clone()
        .oneshot(get(&format!("/{slug}"), None))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FOUND);
    assert_eq!(res.headers()[header::LOCATION], "https://example.com/docs");
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-store");
}

#[tokio::test]
async fn rejects_missing_and_wrong_keys() {
    let (app, _dir) = test_app().await;

    let (status, body, _) = send(&app, create_req(json!({"url": "example.com"}), None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "unauthorized");

    let (status, _, _) = send(
        &app,
        create_req(json!({"url": "example.com"}), Some("wrong-key-000000000")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn accepts_x_api_key_header() {
    let (app, _dir) = test_app().await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/links")
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-api-key", KEY)
        .body(Body::from(
            json!({"url": "https://example.com"}).to_string(),
        ))
        .unwrap();
    let (status, _, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn custom_slug_is_used_and_conflicts() {
    let (app, _dir) = test_app().await;

    let (status, body, _) = send(
        &app,
        create_req(
            json!({"url": "https://example.com/a", "slug": "launch26"}),
            Some(KEY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "body: {body}");
    assert_eq!(body["slug"], "launch26");

    let (status, body, _) = send(
        &app,
        create_req(
            json!({"url": "https://example.com/b", "slug": "launch26"}),
            Some(KEY),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
}

#[tokio::test]
async fn rejects_bad_input() {
    let (app, _dir) = test_app().await;

    for payload in [
        json!({"url": ""}),
        json!({"url": "ftp://example.com"}),
        json!({"url": "javascript:alert(1)"}),
        json!({"url": "https://example.com", "slug": "has space"}),
        json!({"url": "https://example.com", "slug": "api"}),
        json!({"url": "https://example.com", "slug": "-lead"}),
    ] {
        let (status, body, _) = send(&app, create_req(payload.clone(), Some(KEY))).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "payload {payload} -> {body}"
        );
        assert_eq!(body["error"], "invalid_request");
    }
}

#[tokio::test]
async fn missing_slug_is_404() {
    let (app, _dir) = test_app().await;
    let (status, body, _) = send(&app, get("/nope123", None)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn get_link_metadata_requires_auth() {
    let (app, _dir) = test_app().await;
    let (_, body, _) = send(
        &app,
        create_req(
            json!({"url": "https://example.com/x", "slug": "meta1"}),
            Some(KEY),
        ),
    )
    .await;
    assert_eq!(body["slug"], "meta1");

    let (status, body, _) = send(&app, get("/api/links/meta1", Some(KEY))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["url"], "https://example.com/x");
    assert_eq!(body["short_url"], format!("{BASE}/meta1"));

    let (status, _, _) = send(&app, get("/api/links/meta1", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn healthz_reports_ok() {
    let (app, _dir) = test_app().await;
    let (status, body, _) = send(&app, get("/healthz", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["links"], 0);
}

#[tokio::test]
async fn distinct_urls_get_distinct_slugs() {
    let (app, _dir) = test_app().await;
    let mut slugs = std::collections::HashSet::new();
    for i in 0..25 {
        let (status, body, _) = send(
            &app,
            create_req(
                json!({"url": format!("https://example.com/{i}")}),
                Some(KEY),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(
            slugs.insert(body["slug"].as_str().unwrap().to_string()),
            "duplicate slug"
        );
    }
    assert_eq!(slugs.len(), 25);
}
