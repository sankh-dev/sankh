use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use sankh_core::Collection;
use sankh_server::{AppState, app};
use serde_json::{Value, json};
use std::sync::{Arc, Once};
use tower::ServiceExt;

static CONFIG: Once = Once::new();

/// All tests share one isolated trust store for this process.
fn isolate_config() {
    CONFIG.call_once(|| {
        let dir = tempfile::tempdir().unwrap().keep();
        // SAFETY: set once, before any test reads it.
        unsafe { std::env::set_var("SANKH_CONFIG_DIR", dir) };
    });
}

fn collection(files: &[(&str, &str)]) -> (tempfile::TempDir, Collection) {
    isolate_config();
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        let p = dir.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }
    let c = Collection::open(dir.path()).unwrap();
    (dir, c)
}

fn state(c: Collection, token: Option<&str>) -> Arc<AppState> {
    Arc::new(AppState::new(
        c,
        token.map(String::from),
        "127.0.0.1",
        vec![],
    ))
}

async fn call(
    state: &Arc<AppState>,
    method: &str,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "localhost:4747");
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let resp = app(state.clone()).oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes)
        .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, json)
}

#[tokio::test]
async fn token_is_required_when_configured() {
    let (_d, c) = collection(&[("a.sh", "curl x\n")]);
    let s = state(c, Some("s3cret"));
    assert_eq!(
        call(&s, "GET", "/api/tree", &[], None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let bad = [("authorization", "Bearer nope")];
    assert_eq!(
        call(&s, "GET", "/api/tree", &bad, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let good = [("authorization", "Bearer s3cret")];
    assert_eq!(
        call(&s, "GET", "/api/tree", &good, None).await.0,
        StatusCode::OK
    );
    // Static UI does not need the token.
    assert_eq!(call(&s, "GET", "/", &[], None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn rejects_foreign_hosts_and_origins() {
    let (_d, c) = collection(&[("a.sh", "curl x\n")]);
    let s = state(c, None);
    let mut req = Request::builder()
        .uri("/api/tree")
        .header(header::HOST, "evil.example:4747")
        .body(Body::empty())
        .unwrap();
    let resp = app(s.clone()).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "DNS rebinding host");

    let origin = [("origin", "https://evil.example")];
    assert_eq!(
        call(&s, "GET", "/api/tree", &origin, None).await.0,
        StatusCode::FORBIDDEN
    );
    let same = [("origin", "http://localhost:4747")];
    assert_eq!(
        call(&s, "GET", "/api/tree", &same, None).await.0,
        StatusCode::OK
    );

    req = Request::builder()
        .uri("/api/tree")
        .header(header::HOST, "127.0.0.1:4747")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app(s).oneshot(req).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn blocks_path_traversal() {
    let (_d, c) = collection(&[("a.sh", "curl x\n")]);
    let s = state(c, None);
    for uri in [
        "/api/request/..%2F..%2Fetc%2Fevil.sh",
        "/api/request/%2Fetc%2Fevil.sh",
        "/api/request/sub/..%2F..%2Fx.sh",
    ] {
        let (status, _) = call(&s, "PUT", uri, &[], Some(json!({ "content": "x" }))).await;
        assert!(
            status == StatusCode::FORBIDDEN || status == StatusCode::BAD_REQUEST,
            "{uri}: {status}"
        );
    }
    let (status, _) = call(&s, "GET", "/api/request/a.txt", &[], None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn reads_writes_and_renders_requests() {
    let (dir, c) = collection(&[(
        "users/01-list.sh",
        "# @name List\n# @expect status 200\ncurl -sS \"$BASE_URL/users\"\n",
    )]);
    let s = state(c, None);
    let (status, body) = call(&s, "GET", "/api/request/users/01-list.sh", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["request"]["name"], "List");
    assert_eq!(body["form"]["curl"]["url"], "$BASE_URL/users");

    let mut form = body["form"].clone();
    form["name"] = json!("List users");
    let (_, rendered) = call(
        &s,
        "POST",
        "/api/render",
        &[],
        Some(json!({ "form": form })),
    )
    .await;
    let content = rendered["content"].as_str().unwrap().to_string();
    assert!(content.contains("# @name List users"));

    let (status, _) = call(
        &s,
        "PUT",
        "/api/request/users/02-new.sh",
        &[],
        Some(json!({ "content": content })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(dir.path().join("users/02-new.sh").is_file());

    let (_, imported) = call(
        &s,
        "POST",
        "/api/import",
        &[],
        Some(json!({ "curl": "curl 'https://x.dev/a' -H 'Accept: */*'", "name": "A" })),
    )
    .await;
    assert!(imported["content"].as_str().unwrap().contains("# @name A"));

    let (_, tree) = call(&s, "GET", "/api/tree", &[], None).await;
    assert_eq!(tree["children"][0]["children"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn untrusted_runs_are_refused_then_stream_after_trust() {
    let server = httpmock::MockServer::start();
    server.mock(|when, then| {
        when.path("/login");
        then.status(200)
            .json_body(json!({ "token": "abcd-1234-token" }));
    });
    server.mock(|when, then| {
        when.path("/me")
            .header("authorization", "Bearer abcd-1234-token");
        then.status(200).json_body(json!({ "id": 1 }));
    });
    let (dir, c) = collection(&[
        (
            "environments/dev.env",
            &format!("BASE_URL={}\n", server.base_url()),
        ),
        (
            "01-login.sh",
            "# @capture TOKEN=.token\ncurl -sS \"$BASE_URL/login\"\n",
        ),
        (
            "02-me.sh",
            "# @expect json .id == 1\ncurl -sS \"$BASE_URL/me\" -H \"Authorization: Bearer $TOKEN\"\n",
        ),
    ]);
    let s = state(c, None);
    let run = json!({ "path": "", "env": "dev" });
    let (status, body) = call(&s, "POST", "/api/run", &[], Some(run.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["details"]["state"], "untrusted");

    let (status, _) = call(&s, "POST", "/api/trust", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = call(&s, "POST", "/api/run", &[], Some(run)).await;
    assert_eq!(status, StatusCode::OK);
    let id = body["id"].as_str().unwrap();

    let (status, events) = call(&s, "GET", &format!("/api/runs/{id}/events"), &[], None).await;
    assert_eq!(status, StatusCode::OK);
    let events: Vec<Value> = events
        .as_str()
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let kinds: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["start", "running", "result", "running", "result", "done"]
    );
    assert_eq!(events[5]["summary"]["passed"], 2);
    let all = serde_json::to_string(&events).unwrap();
    assert!(!all.contains("abcd-1234-token"), "captured token leaked");

    // Captures persist for the session: running 02 alone still has TOKEN.
    let (_, body) = call(
        &s,
        "POST",
        "/api/run",
        &[],
        Some(json!({ "path": "02-me.sh", "env": "dev" })),
    )
    .await;
    let id = body["id"].as_str().unwrap();
    let (_, events) = call(&s, "GET", &format!("/api/runs/{id}/events"), &[], None).await;
    assert!(events.as_str().unwrap().contains("\"passed\":1"));
    drop(dir);
}

#[test]
fn remote_listen_requires_token() {
    let (_d, c) = collection(&[("a.sh", "curl x\n")]);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let err = rt
        .block_on(sankh_server::serve(sankh_server::ServeConfig {
            collection: c,
            listen: "0.0.0.0".into(),
            port: 0,
            token: None,
            allow_hosts: vec![],
        }))
        .unwrap_err();
    assert!(err.to_string().contains("--token"));
}
