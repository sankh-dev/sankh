//! In-memory petstore API used by the integration tests and by
//! `cargo run --example petstore_mock` for trying `examples/petstore`.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub const PASSWORD: &str = "demo-password";
pub const TOKEN: &str = "tok-5f2b9c1e7a";

#[derive(Default)]
struct Store {
    next_id: u64,
    pets: BTreeMap<u64, Value>,
}

type Shared = Arc<Mutex<Store>>;

fn authorized(headers: &HeaderMap) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {TOKEN}"))
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "unauthorized" })),
    )
        .into_response()
}

async fn login(Json(body): Json<Value>) -> Response {
    if body["password"] == PASSWORD {
        Json(json!({ "token": TOKEN, "user": body["username"] })).into_response()
    } else {
        unauthorized()
    }
}

async fn list(State(s): State<Shared>, headers: HeaderMap) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let items: Vec<Value> = s.lock().unwrap().pets.values().cloned().collect();
    Json(json!({ "items": items })).into_response()
}

async fn create(State(s): State<Shared>, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let Some(name) = body["name"].as_str() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "name required" })),
        )
            .into_response();
    };
    let mut store = s.lock().unwrap();
    store.next_id += 1;
    let id = store.next_id;
    let pet =
        json!({ "id": id, "name": name, "status": "available", "tags": body["tags"].clone() });
    store.pets.insert(id, pet.clone());
    (
        StatusCode::CREATED,
        [("x-request-id", format!("req-{id}"))],
        Json(pet),
    )
        .into_response()
}

async fn get_pet(State(s): State<Shared>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let pet = id
        .parse::<u64>()
        .ok()
        .and_then(|id| s.lock().unwrap().pets.get(&id).cloned());
    match pet {
        Some(p) => Json(p).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))).into_response(),
    }
}

async fn delete_pet(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    if !authorized(&headers) {
        return unauthorized();
    }
    let removed = id
        .parse::<u64>()
        .ok()
        .and_then(|id| s.lock().unwrap().pets.remove(&id));
    match removed {
        Some(_) => StatusCode::NO_CONTENT.into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" }))).into_response(),
    }
}

/// Echoes the request back, for quoting tests.
async fn echo(headers: HeaderMap, body: String) -> Json<Value> {
    let h: BTreeMap<String, String> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                String::from_utf8_lossy(v.as_bytes()).into_owned(),
            )
        })
        .collect();
    Json(json!({ "headers": h, "body": body }))
}

pub fn router() -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/pets", get(list).post(create))
        .route("/pets/{id}", get(get_pet).delete(delete_pet))
        .route("/echo", post(echo))
        .with_state(Shared::default())
}

/// Binds on an ephemeral port in a background thread; returns the base URL.
pub fn spawn() -> String {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            tx.send(listener.local_addr().unwrap()).unwrap();
            axum::serve(listener, router()).await.unwrap();
        });
    });
    format!("http://{}", rx.recv().unwrap())
}
