//! JSON API handlers.

use crate::AppState;
use crate::runs::RunHandle;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use sankh_core::collection::CollectionError;
use sankh_core::env::{self, Env};
use sankh_core::format::{self, RequestForm};
use sankh_core::redact::Redactor;
use sankh_core::report::Summary;
use sankh_core::select::{self, Filters};
use sankh_core::trust::{self, TrustStore};
use sankh_core::{RunContext, RunOptions, runner};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

pub struct ApiError(StatusCode, String, Option<Value>);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = json!({ "error": self.1 });
        if let Some(extra) = self.2 {
            body["details"] = extra;
        }
        (self.0, Json(body)).into_response()
    }
}

fn bad(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into(), None)
}

impl From<CollectionError> for ApiError {
    fn from(e: CollectionError) -> Self {
        let status = match e {
            CollectionError::OutsideRoot(_) => StatusCode::FORBIDDEN,
            CollectionError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError(status, e.to_string(), None)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        let status = if e.kind() == std::io::ErrorKind::NotFound {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        ApiError(status, e.to_string(), None)
    }
}

type ApiResult = Result<Json<Value>, ApiError>;

fn trust_json(state: &AppState) -> Value {
    let store = TrustStore::load().unwrap_or_default();
    serde_json::to_value(store.status(&state.collection.root)).unwrap_or(Value::Null)
}

pub async fn info(State(state): State<Arc<AppState>>) -> ApiResult {
    Ok(Json(json!({
        "name": state.collection.name(),
        "root": state.collection.root.display().to_string(),
        "version": sankh_core::version(),
        "trust": trust_json(&state),
        "default_env": state.collection.config.default_env,
    })))
}

pub async fn tree(State(state): State<Arc<AppState>>) -> ApiResult {
    let tree = state.collection.tree()?;
    Ok(Json(serde_json::to_value(tree).unwrap()))
}

fn request_file(state: &AppState, path: &str) -> Result<std::path::PathBuf, ApiError> {
    if !path.ends_with(".sh") {
        return Err(bad("request files must end in .sh"));
    }
    Ok(state.collection.resolve(path)?)
}

fn request_json(state: &AppState, path: &str, content: String) -> Value {
    let abs = state.collection.root.join(path);
    let req = state.collection.parse_file(&abs, &content);
    let form = RequestForm::from_request(&req);
    json!({ "path": path, "content": content, "request": req, "form": form })
}

pub async fn get_request(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> ApiResult {
    let abs = request_file(&state, &path)?;
    let content = std::fs::read_to_string(&abs)?;
    Ok(Json(request_json(&state, &path, content)))
}

#[derive(Deserialize)]
pub struct SaveBody {
    content: String,
}

pub async fn put_request(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    Json(body): Json<SaveBody>,
) -> ApiResult {
    let abs = request_file(&state, &path)?;
    if let Some(dir) = abs.parent() {
        std::fs::create_dir_all(dir)?;
        // Re-check after creating directories, in case of symlinks.
        state.collection.resolve(&path)?;
    }
    std::fs::write(&abs, &body.content)?;
    Ok(Json(request_json(&state, &path, body.content)))
}

pub async fn delete_request(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> ApiResult {
    let abs = request_file(&state, &path)?;
    std::fs::remove_file(&abs)?;
    Ok(Json(json!({ "deleted": path })))
}

#[derive(Deserialize)]
pub struct ParseBody {
    content: String,
    #[serde(default)]
    path: Option<String>,
}

pub async fn parse(State(state): State<Arc<AppState>>, Json(body): Json<ParseBody>) -> ApiResult {
    let path = body.path.unwrap_or_else(|| "request.sh".into());
    Ok(Json(request_json(&state, &path, body.content)))
}

#[derive(Deserialize)]
pub struct RenderBody {
    form: RequestForm,
}

pub async fn render(Json(body): Json<RenderBody>) -> ApiResult {
    let content = format::render(&body.form);
    let req = sankh_core::parser::parse(&content, "request.sh");
    Ok(Json(json!({ "content": content, "request": req })))
}

#[derive(Deserialize)]
pub struct ImportBody {
    curl: String,
    #[serde(default)]
    name: Option<String>,
}

pub async fn import(Json(body): Json<ImportBody>) -> ApiResult {
    let name = body.name.unwrap_or_else(|| "Imported request".into());
    let content = format::import_curl(&body.curl, &name).map_err(bad)?;
    Ok(Json(json!({ "content": content })))
}

pub async fn envs(State(state): State<Arc<AppState>>) -> ApiResult {
    Ok(Json(json!({
        "envs": env::list_envs(&state.collection),
        "default": state.collection.config.default_env,
    })))
}

/// Variables of an environment, for editor autocomplete. Secret values are masked.
pub async fn env_vars(State(state): State<Arc<AppState>>, Path(name): Path<String>) -> ApiResult {
    let env_name = (name != "_").then_some(name.as_str());
    let env = Env::load(&state.collection, env_name).map_err(|e| bad(e.to_string()))?;
    let redactor = Redactor::from_vars(&env.file_vars);
    let mut vars: Vec<Value> = env
        .file_vars
        .iter()
        .map(|(k, v)| json!({ "name": k, "value": redactor.display_var(k, v), "source": "file" }))
        .collect();
    let captures = state.captures.lock().unwrap();
    if let Some(c) = captures.get(&name) {
        for (k, v) in c {
            vars.push(
                json!({ "name": k, "value": redactor.display_var(k, v), "source": "capture" }),
            );
        }
    }
    Ok(Json(json!({ "vars": vars })))
}

#[derive(Deserialize)]
pub struct EnvQuery {
    #[serde(default)]
    env: Option<String>,
}

pub async fn clear_captures(
    State(state): State<Arc<AppState>>,
    Query(q): Query<EnvQuery>,
) -> ApiResult {
    let key = q.env.unwrap_or_else(|| "_".into());
    state.captures.lock().unwrap().remove(&key);
    Ok(Json(json!({ "cleared": key })))
}

pub async fn get_trust(State(state): State<Arc<AppState>>) -> ApiResult {
    Ok(Json(trust_json(&state)))
}

pub async fn post_trust(State(state): State<Arc<AppState>>) -> ApiResult {
    let mut store = TrustStore::load().map_err(|e| bad(e.to_string()))?;
    store
        .trust(&state.collection.root)
        .map_err(|e| bad(e.to_string()))?;
    store.save().map_err(|e| bad(e.to_string()))?;
    Ok(Json(trust_json(&state)))
}

#[derive(Deserialize)]
pub struct RunBody {
    /// File or folder relative to the root; empty for the whole collection.
    #[serde(default)]
    path: String,
    #[serde(default)]
    env: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

pub async fn start_run(State(state): State<Arc<AppState>>, Json(body): Json<RunBody>) -> ApiResult {
    let collection = state.collection.clone();
    let trusted = trust::ensure(&collection, false).map_err(|e| {
        ApiError(
            StatusCode::FORBIDDEN,
            e.to_string(),
            Some(trust_json(&state)),
        )
    })?;
    let target = collection.resolve(&body.path)?;
    if !target.exists() {
        return Err(ApiError(
            StatusCode::NOT_FOUND,
            format!("{} not found", body.path),
            None,
        ));
    }
    let env_name = body.env.filter(|e| !e.is_empty());
    let mut env = Env::load(&collection, env_name.as_deref()).map_err(|e| bad(e.to_string()))?;
    let capture_key = env_name.clone().unwrap_or_else(|| "_".into());
    if let Some(c) = state.captures.lock().unwrap().get(&capture_key) {
        env.captures = c.clone();
    }
    let files = select::select(
        &collection,
        &target,
        &Filters {
            folders: vec![],
            tags: body.tags,
        },
    )?;

    let id = uuid::Uuid::new_v4().to_string();
    let handle = RunHandle::new();
    state.runs.insert(id.clone(), handle.clone());
    let paths: Vec<String> = files.iter().map(|f| collection.rel(f)).collect();
    handle.push(json!({ "type": "start", "total": files.len(), "paths": paths }));

    let state2 = state.clone();
    tokio::task::spawn_blocking(move || {
        let options = RunOptions::for_collection(&collection);
        let mut ctx = RunContext::new(env, options);
        let mut results = Vec::new();
        for file in &files {
            let rel = collection.rel(file);
            handle.push(json!({ "type": "running", "path": rel }));
            let result = runner::run_request(&trusted, &collection, file, &mut ctx);
            handle.push(json!({ "type": "result", "result": result }));
            results.push(result);
        }
        state2
            .captures
            .lock()
            .unwrap()
            .insert(capture_key, ctx.env.captures.clone());
        handle.push(json!({ "type": "done", "summary": Summary::of(&results) }));
        handle.finish();
    });

    Ok(Json(json!({ "id": id })))
}

pub async fn run_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let handle = state
        .runs
        .get(&id)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "unknown run".into(), None))?;
    Ok(Sse::new(handle.stream()).keep_alive(KeepAlive::default()))
}

pub async fn change_events(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    use axum::response::sse::Event;
    use futures::StreamExt;
    let rx = state.changes.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .map(|_| Ok::<_, std::convert::Infallible>(Event::default().data("changed")));
    Sse::new(stream).keep_alive(KeepAlive::default())
}
