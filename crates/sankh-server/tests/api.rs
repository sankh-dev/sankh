use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use sankh_server::{AppState, Registry, app};
use serde_json::{Value, json};
use std::sync::{Arc, Once};
use tower::ServiceExt;

static CONFIG: Once = Once::new();

/// All tests share one isolated trust store, workspace file and data dir.
fn isolate_config() {
    CONFIG.call_once(|| {
        let dir = tempfile::tempdir().unwrap().keep();
        // SAFETY: set once, before any test reads them.
        unsafe {
            std::env::set_var("SANKH_CONFIG_DIR", dir.join("config"));
            std::env::set_var("SANKH_DATA_DIR", dir.join("data"));
        }
    });
}

fn folder(files: &[(&str, &str)]) -> tempfile::TempDir {
    isolate_config();
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        let p = dir.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }
    dir
}

/// A session state over `dirs` (no Scratch); returns the collection ids in order.
fn state_of(dirs: &[&tempfile::TempDir], token: Option<&str>) -> (Arc<AppState>, Vec<String>) {
    let paths: Vec<_> = dirs.iter().map(|d| d.path().to_path_buf()).collect();
    let registry = Registry::session(&paths, false).unwrap();
    let ids = registry.slots().iter().map(|s| s.id.clone()).collect();
    let state = AppState::new(registry, token.map(String::from), "127.0.0.1", vec![]);
    (Arc::new(state), ids)
}

fn state(dir: &tempfile::TempDir, token: Option<&str>) -> (Arc<AppState>, String) {
    let (s, ids) = state_of(&[dir], token);
    (s, ids[0].clone())
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

/// Starts a run and returns its parsed events.
async fn run(s: &Arc<AppState>, cid: &str, body: Value) -> (StatusCode, Vec<Value>, Value) {
    let (status, started) = call(s, "POST", &format!("/api/c/{cid}/run"), &[], Some(body)).await;
    if status != StatusCode::OK {
        return (status, vec![], started);
    }
    let id = started["id"].as_str().unwrap();
    let (_, events) = call(s, "GET", &format!("/api/runs/{id}/events"), &[], None).await;
    let events = events
        .as_str()
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    (status, events, started)
}

#[tokio::test]
async fn token_is_required_when_configured() {
    let d = folder(&[("a.sh", "curl x\n")]);
    let (s, cid) = state(&d, Some("s3cret"));
    let tree = format!("/api/c/{cid}/tree");
    assert_eq!(
        call(&s, "GET", &tree, &[], None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let bad = [("authorization", "Bearer nope")];
    assert_eq!(
        call(&s, "GET", &tree, &bad, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let good = [("authorization", "Bearer s3cret")];
    assert_eq!(call(&s, "GET", &tree, &good, None).await.0, StatusCode::OK);
    assert_eq!(
        call(&s, "GET", "/api/info", &[], None).await.0,
        StatusCode::UNAUTHORIZED
    );
    // Static UI does not need the token.
    assert_eq!(call(&s, "GET", "/", &[], None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn rejects_foreign_hosts_and_origins() {
    let d = folder(&[("a.sh", "curl x\n")]);
    let (s, cid) = state(&d, None);
    let tree = format!("/api/c/{cid}/tree");
    let mut req = Request::builder()
        .uri(&tree)
        .header(header::HOST, "evil.example:4747")
        .body(Body::empty())
        .unwrap();
    let resp = app(s.clone()).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "DNS rebinding host");

    let origin = [("origin", "https://evil.example")];
    assert_eq!(
        call(&s, "GET", &tree, &origin, None).await.0,
        StatusCode::FORBIDDEN
    );
    let same = [("origin", "http://localhost:4747")];
    assert_eq!(call(&s, "GET", &tree, &same, None).await.0, StatusCode::OK);

    req = Request::builder()
        .uri(&tree)
        .header(header::HOST, "127.0.0.1:4747")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app(s).oneshot(req).await.unwrap().status(), StatusCode::OK);
}

#[tokio::test]
async fn blocks_path_traversal_in_every_collection() {
    let a = folder(&[("a.sh", "curl x\n")]);
    let b = folder(&[("b.sh", "curl x\n")]);
    let (s, ids) = state_of(&[&a, &b], None);
    for cid in &ids {
        for rel in [
            "..%2F..%2Fetc%2Fevil.sh",
            "%2Fetc%2Fevil.sh",
            "sub/..%2F..%2Fx.sh",
        ] {
            let uri = format!("/api/c/{cid}/request/{rel}");
            let (status, _) = call(&s, "PUT", &uri, &[], Some(json!({ "content": "x" }))).await;
            assert!(
                status == StatusCode::FORBIDDEN || status == StatusCode::BAD_REQUEST,
                "{uri}: {status}"
            );
        }
        let (status, _) = call(&s, "GET", &format!("/api/c/{cid}/request/a.txt"), &[], None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    // A file from one collection cannot be read through another.
    let (status, _) = call(
        &s,
        "GET",
        &format!("/api/c/{}/request/a.sh", ids[1]),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&s, "GET", "/api/c/nope/tree", &[], None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reads_writes_and_renders_requests() {
    let dir = folder(&[(
        "users/01-list.sh",
        "# @name List\n# @expect status 200\ncurl -sS \"$BASE_URL/users\"\n",
    )]);
    let (s, cid) = state(&dir, None);
    let (status, body) = call(
        &s,
        "GET",
        &format!("/api/c/{cid}/request/users/01-list.sh"),
        &[],
        None,
    )
    .await;
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
        &format!("/api/c/{cid}/request/users/02-new.sh"),
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

    let (_, tree) = call(&s, "GET", &format!("/api/c/{cid}/tree"), &[], None).await;
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
    let env = format!("BASE_URL={}\n", server.base_url());
    let me =
        "# @expect json .id == 1\ncurl -sS \"$BASE_URL/me\" -H \"Authorization: Bearer $TOKEN\"\n";
    let users = folder(&[
        ("environments/dev.env", &env),
        (
            "01-login.sh",
            "# @capture TOKEN=.token\ncurl -sS \"$BASE_URL/login\"\n",
        ),
        ("02-me.sh", me),
    ]);
    let payments = folder(&[("environments/dev.env", &env), ("02-me.sh", me)]);
    let (s, ids) = state_of(&[&users, &payments], None);
    let (a, b) = (&ids[0], &ids[1]);

    let (status, _, body) = run(&s, a, json!({ "path": "", "env": "dev" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["details"]["state"], "untrusted");

    for cid in [a, b] {
        let (status, _) = call(&s, "POST", &format!("/api/c/{cid}/trust"), &[], None).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, events, _) = run(&s, a, json!({ "path": "", "env": "dev" })).await;
    assert_eq!(status, StatusCode::OK);
    let kinds: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["start", "running", "result", "running", "result", "done"]
    );
    assert_eq!(events[0]["collection"], a.as_str());
    assert_eq!(events[5]["summary"]["passed"], 2);
    let all = serde_json::to_string(&events).unwrap();
    assert!(!all.contains("abcd-1234-token"), "captured token leaked");

    // Captures persist for the session: running 02 alone still has TOKEN.
    let (_, events, _) = run(&s, a, json!({ "path": "02-me.sh", "env": "dev" })).await;
    assert_eq!(events.last().unwrap()["summary"]["passed"], 1);

    // ...but only within that collection.
    let (_, events, _) = run(&s, b, json!({ "path": "02-me.sh", "env": "dev" })).await;
    assert_eq!(events.last().unwrap()["summary"]["passed"], 0);
    let (_, vars) = call(&s, "GET", &format!("/api/c/{b}/envs/dev"), &[], None).await;
    assert!(!serde_json::to_string(&vars).unwrap().contains("capture"));

    // Unlinking forgets the collection's captures.
    let (status, _) = call(&s, "DELETE", &format!("/api/collections/{a}"), &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(s.captures.lock().unwrap().keys().all(|(c, _)| c != a));
    drop((users, payments));
}

#[tokio::test]
async fn adds_unlinks_and_copies_between_collections() {
    let a = folder(&[
        ("sankh.toml", "name = \"Users API\"\n"),
        ("x/a.sh", "curl a\n"),
    ]);
    let b = folder(&[("sankh.toml", "name = \"Payments\"\n")]);
    let (s, ids) = state_of(&[&a], None);
    assert_eq!(ids, ["users-api"]);

    let (status, added) = call(
        &s,
        "POST",
        "/api/collections",
        &[],
        Some(json!({ "path": b.path().display().to_string() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(added["id"], "payments");
    assert_eq!(added["trust"]["state"], "untrusted");

    let (status, _) = call(
        &s,
        "POST",
        "/api/collections",
        &[],
        Some(json!({ "path": b.path().display().to_string() })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = call(
        &s,
        "POST",
        "/api/collections",
        &[],
        Some(json!({ "path": "/definitely/not/here" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, info) = call(&s, "GET", "/api/info", &[], None).await;
    assert_eq!(info["saved"], false);
    let listed: Vec<&str> = info["collections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(listed, ["users-api", "payments"]);

    let copy = json!({ "path": "x/a.sh", "to": "payments" });
    let (status, body) = call(&s, "POST", "/api/c/users-api/copy", &[], Some(copy.clone())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        std::fs::read_to_string(b.path().join("x/a.sh")).unwrap(),
        "curl a\n"
    );
    let (status, _) = call(&s, "POST", "/api/c/users-api/copy", &[], Some(copy)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let escape = json!({ "path": "x/a.sh", "to": "payments", "to_path": "../evil.sh" });
    let (status, _) = call(&s, "POST", "/api/c/users-api/copy", &[], Some(escape)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = call(&s, "DELETE", "/api/collections/payments", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        b.path().join("x/a.sh").is_file(),
        "unlink must not delete files"
    );
    let (status, _) = call(&s, "GET", "/api/c/payments/tree", &[], None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&s, "DELETE", "/api/collections/payments", &[], None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, dirs) = call(
        &s,
        "GET",
        &format!("/api/fs/dirs?path={}", a.path().display()),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dirs["collection"], true);
    assert_eq!(dirs["dirs"][0]["name"], "x");
}

#[tokio::test]
async fn scratch_is_built_in_trusted_and_cannot_be_unlinked() {
    isolate_config();
    let registry = Registry::session(&[], true).unwrap();
    let s = Arc::new(AppState::new(registry, None, "127.0.0.1", vec![]));
    let (_, info) = call(&s, "GET", "/api/info", &[], None).await;
    let scratch = &info["collections"][0];
    assert_eq!(scratch["id"], "scratch");
    assert_eq!(scratch["scratch"], true);
    assert_eq!(scratch["name"], "Scratch");
    assert_eq!(scratch["trust"]["state"], "trusted");
    assert_eq!(scratch["default_env"], "default");

    let (status, _) = call(&s, "DELETE", "/api/collections/scratch", &[], None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &s,
        "POST",
        "/api/collections",
        &[],
        Some(json!({ "path": scratch["root"] })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

fn postman_fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sankh-core/tests/fixtures/postman")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

#[tokio::test]
async fn imports_postman_with_preview_then_write() {
    let existing = folder(&[("a.sh", "curl x\n")]);
    let (s, _) = state(&existing, None);
    let out = tempfile::tempdir().unwrap();
    let dir = out.path().join("petstore");
    let body = json!({
        "collection": postman_fixture("petstore.postman_collection.json"),
        "envs": [postman_fixture("petstore.postman_environment.json")],
        "dir": dir.display().to_string(),
    });

    let (status, preview) = call(&s, "POST", "/api/import/postman", &[], Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    assert_eq!(preview["nonempty"], false);
    assert!(preview["report"]["requests"].as_u64().unwrap() > 0);
    let files = preview["files"].as_array().unwrap();
    assert!(files.iter().any(|f| f == "sankh.toml"));
    assert!(!dir.exists(), "preview must not write");

    let mut write = body.clone();
    write["write"] = json!(true);
    let (status, done) = call(&s, "POST", "/api/import/postman", &[], Some(write.clone())).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert!(dir.join("sankh.toml").is_file());
    let cid = done["collection"]["id"].as_str().unwrap().to_string();
    let (_, info) = call(&s, "GET", "/api/info", &[], None).await;
    assert!(
        info["collections"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["id"] == cid)
    );
    let (status, _) = call(&s, "GET", &format!("/api/c/{cid}/tree"), &[], None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, again) = call(&s, "POST", "/api/import/postman", &[], Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["nonempty"], true);
    let (status, err) = call(&s, "POST", "/api/import/postman", &[], Some(write)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(err["error"].as_str().unwrap().contains("not empty"));
}

#[tokio::test]
async fn postman_import_refuses_nested_targets_and_bad_input() {
    let existing = folder(&[("sankh.toml", ""), ("a.sh", "curl x\n")]);
    let (s, _) = state(&existing, None);
    let inside = existing.path().join("nested");
    let (status, err) = call(
        &s,
        "POST",
        "/api/import/postman",
        &[],
        Some(json!({
            "collection": postman_fixture("petstore.postman_collection.json"),
            "dir": inside.display().to_string(),
            "write": true,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(err["error"].as_str().unwrap().contains("nested"));
    assert!(!inside.exists(), "nothing is written for a rejected target");

    let (status, err) = call(
        &s,
        "POST",
        "/api/import/postman",
        &[],
        Some(json!({ "collection": "{\"values\": []}" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["error"].as_str().unwrap().contains("environment"));
}

#[test]
fn remote_listen_requires_token() {
    isolate_config();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let err = rt
        .block_on(sankh_server::serve(sankh_server::ServeConfig {
            workspace: sankh_server::WorkspaceSource::Session(vec![]),
            listen: "0.0.0.0".into(),
            port: 0,
            token: None,
            allow_hosts: vec![],
        }))
        .unwrap_err();
    assert!(err.to_string().contains("--token"));
}
