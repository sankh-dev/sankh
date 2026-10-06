//! JSON API handlers.

use crate::AppState;
use crate::registry::Slot;
use crate::runs::RunHandle;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use sankh_core::Collection;
use sankh_core::collection::{CONFIG_FILE, CollectionError, ENV_DIR};
use sankh_core::env::{self, Env, EnvEditError};
use sankh_core::format::{self, RequestForm};
use sankh_core::import::{self, ImportError};
use sankh_core::redact::{Redactor, is_secret_name};
use sankh_core::report::Summary;
use sankh_core::select::{self, Filters};
use sankh_core::trust::{self, TrustStore};
use sankh_core::workspace::WorkspaceError;
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

fn not_found(msg: impl Into<String>) -> ApiError {
    ApiError(StatusCode::NOT_FOUND, msg.into(), None)
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

impl From<WorkspaceError> for ApiError {
    fn from(e: WorkspaceError) -> Self {
        let status = match &e {
            WorkspaceError::Collection(CollectionError::NotFound(_)) => StatusCode::NOT_FOUND,
            WorkspaceError::Collection(_) | WorkspaceError::Scratch => StatusCode::BAD_REQUEST,
            WorkspaceError::AlreadyAdded { .. } | WorkspaceError::Overlaps { .. } => {
                StatusCode::CONFLICT
            }
            WorkspaceError::Unknown(_) => StatusCode::NOT_FOUND,
            WorkspaceError::Store { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError(status, e.to_string(), None)
    }
}

type ApiResult = Result<Json<Value>, ApiError>;

/// The open collection with id `cid`.
fn collection(state: &AppState, cid: &str) -> Result<Collection, ApiError> {
    let registry = state.registry.read().unwrap();
    let slot = registry
        .slot(cid)
        .ok_or_else(|| not_found(format!("no collection `{cid}` in the workspace")))?;
    slot.collection
        .clone()
        .map_err(|e| not_found(format!("collection `{cid}` is unavailable: {e}")))
}

fn trust_json(collection: &Collection) -> Value {
    let store = TrustStore::load().unwrap_or_default();
    serde_json::to_value(store.status(&collection.root)).unwrap_or(Value::Null)
}

fn slot_json(slot: &Slot) -> Value {
    match &slot.collection {
        Ok(c) => json!({
            "id": slot.id,
            "name": c.name(),
            "root": c.root.display().to_string(),
            "scratch": slot.is_scratch(),
            "missing": false,
            "error": null,
            "trust": trust_json(c),
            "default_env": c.config.default_env,
        }),
        Err(e) => json!({
            "id": slot.id,
            "name": slot.id,
            "root": slot.path.display().to_string(),
            "scratch": slot.is_scratch(),
            "missing": true,
            "error": e,
            "trust": null,
            "default_env": null,
        }),
    }
}

pub async fn info(State(state): State<Arc<AppState>>) -> ApiResult {
    let registry = state.registry.read().unwrap();
    let collections: Vec<Value> = registry.slots().iter().map(slot_json).collect();
    Ok(Json(json!({
        "version": sankh_core::version(),
        "saved": registry.persist(),
        "collections": collections,
    })))
}

#[derive(Deserialize)]
pub struct AddBody {
    path: String,
}

pub async fn add_collection(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddBody>,
) -> ApiResult {
    let path = expand_home(body.path.trim());
    if path.as_os_str().is_empty() {
        return Err(bad("path is required"));
    }
    let slot = {
        let mut registry = state.registry.write().unwrap();
        let id = registry.add(&path)?;
        slot_json(registry.slot(&id).unwrap())
    };
    state.sync_watch();
    Ok(Json(slot))
}

pub async fn remove_collection(
    State(state): State<Arc<AppState>>,
    Path(cid): Path<String>,
) -> ApiResult {
    state.registry.write().unwrap().remove(&cid)?;
    state.captures.lock().unwrap().retain(|(c, _), _| *c != cid);
    state.sync_watch();
    Ok(Json(json!({ "unlinked": cid })))
}

fn expand_home(path: &str) -> std::path::PathBuf {
    match (path.strip_prefix('~'), dirs::home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            home.join(rest.trim_start_matches('/'))
        }
        _ => path.into(),
    }
}

#[derive(Deserialize)]
pub struct DirsQuery {
    #[serde(default)]
    path: Option<String>,
}

const MAX_DIRS: usize = 1000;

/// Subdirectories of a folder, for the "Add folder" picker. Only names are
/// returned, never file contents.
pub async fn list_dirs(Query(q): Query<DirsQuery>) -> ApiResult {
    let start = q
        .path
        .filter(|p| !p.trim().is_empty())
        .map(|p| expand_home(p.trim()))
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| "/".into());
    let dir = start
        .canonicalize()
        .map_err(|_| not_found(format!("{} not found", start.display())))?;
    if !dir.is_dir() {
        return Err(bad(format!("{} is not a folder", dir.display())));
    }
    let mut dirs: Vec<Value> = std::fs::read_dir(&dir)?
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter(|e| e.path().is_dir())
        .take(MAX_DIRS)
        .map(|e| {
            let p = e.path();
            json!({
                "name": e.file_name().to_string_lossy(),
                "path": p.display().to_string(),
                "collection": is_collection_dir(&p),
            })
        })
        .collect();
    dirs.sort_by_key(|d| d["name"].as_str().unwrap_or("").to_lowercase());
    Ok(Json(json!({
        "path": dir.display().to_string(),
        "parent": dir.parent().map(|p| p.display().to_string()),
        "collection": is_collection_dir(&dir),
        "dirs": dirs,
    })))
}

fn is_collection_dir(p: &std::path::Path) -> bool {
    p.join(CONFIG_FILE).is_file() || p.join(ENV_DIR).is_dir()
}

pub async fn tree(State(state): State<Arc<AppState>>, Path(cid): Path<String>) -> ApiResult {
    let tree = collection(&state, &cid)?.tree()?;
    Ok(Json(serde_json::to_value(tree).unwrap()))
}

fn request_file(c: &Collection, path: &str) -> Result<std::path::PathBuf, ApiError> {
    if !path.ends_with(".sh") {
        return Err(bad("request files must end in .sh"));
    }
    Ok(c.resolve(path)?)
}

fn request_json(c: &Collection, path: &str, content: String) -> Value {
    let abs = c.root.join(path);
    let req = c.parse_file(&abs, &content);
    let form = RequestForm::from_request(&req);
    json!({ "path": path, "content": content, "request": req, "form": form })
}

pub async fn get_request(
    State(state): State<Arc<AppState>>,
    Path((cid, path)): Path<(String, String)>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    let abs = request_file(&c, &path)?;
    let content = std::fs::read_to_string(&abs)?;
    Ok(Json(request_json(&c, &path, content)))
}

#[derive(Deserialize)]
pub struct SaveBody {
    content: String,
}

/// Creates parent folders of `path` inside `c`, re-checking afterwards in
/// case a symlink points outside the collection.
fn prepare_parent(c: &Collection, path: &str, abs: &std::path::Path) -> Result<(), ApiError> {
    if let Some(dir) = abs.parent() {
        std::fs::create_dir_all(dir)?;
        c.resolve(path)?;
    }
    Ok(())
}

pub async fn put_request(
    State(state): State<Arc<AppState>>,
    Path((cid, path)): Path<(String, String)>,
    Json(body): Json<SaveBody>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    let abs = request_file(&c, &path)?;
    prepare_parent(&c, &path, &abs)?;
    std::fs::write(&abs, &body.content)?;
    Ok(Json(request_json(&c, &path, body.content)))
}

pub async fn delete_request(
    State(state): State<Arc<AppState>>,
    Path((cid, path)): Path<(String, String)>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    let abs = request_file(&c, &path)?;
    std::fs::remove_file(&abs)?;
    Ok(Json(json!({ "deleted": path })))
}

#[derive(Deserialize)]
pub struct CopyBody {
    path: String,
    to: String,
    #[serde(default)]
    to_path: Option<String>,
}

/// Copies a request file into another collection; never overwrites.
pub async fn copy_request(
    State(state): State<Arc<AppState>>,
    Path(cid): Path<String>,
    Json(body): Json<CopyBody>,
) -> ApiResult {
    let from = collection(&state, &cid)?;
    let to = collection(&state, &body.to)?;
    let src = request_file(&from, &body.path)?;
    let content = std::fs::read_to_string(&src)?;
    let dest_rel = body
        .to_path
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| body.path.clone());
    let dest = request_file(&to, &dest_rel)?;
    if dest.exists() {
        return Err(ApiError(
            StatusCode::CONFLICT,
            format!("{dest_rel} already exists in `{}`", body.to),
            None,
        ));
    }
    prepare_parent(&to, &dest_rel, &dest)?;
    std::fs::write(&dest, &content)?;
    Ok(Json(json!({ "collection": body.to, "path": dest_rel })))
}

#[derive(Deserialize)]
pub struct ParseBody {
    content: String,
    #[serde(default)]
    path: Option<String>,
}

pub async fn parse(
    State(state): State<Arc<AppState>>,
    Path(cid): Path<String>,
    Json(body): Json<ParseBody>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    let path = body.path.unwrap_or_else(|| "request.sh".into());
    Ok(Json(request_json(&c, &path, body.content)))
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

#[derive(Deserialize)]
pub struct PostmanBody {
    /// Text of the exported collection.
    collection: String,
    /// Texts of exported Postman environments.
    #[serde(default)]
    envs: Vec<String>,
    /// Output folder; defaults to a free `~/sankh-collections/<collection-slug>` folder.
    #[serde(default)]
    dir: Option<String>,
    #[serde(default)]
    force: bool,
    /// Write the files and add the folder; otherwise only preview.
    #[serde(default)]
    write: bool,
}

/// Converts a Postman export. Without `write` it returns the report and the
/// files it would create; with `write` it writes them and adds the folder to
/// the workspace.
pub async fn import_postman(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PostmanBody>,
) -> ApiResult {
    let envs: Vec<&str> = body.envs.iter().map(String::as_str).collect();
    let imported =
        import::postman::read(&body.collection, &envs).map_err(|e| bad(e.to_string()))?;
    let mut rendered = import::render(&imported);
    let dir = match body.dir.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        Some(d) => expand_home(d),
        None => default_import_dir(&import::slug(&imported.name)),
    };
    let nonempty = dir_nonempty(&dir);
    if !body.write {
        let files: Vec<&str> = rendered.files.iter().map(|(p, _)| p.as_str()).collect();
        return Ok(Json(json!({
            "dir": dir.display().to_string(),
            "nonempty": nonempty,
            "files": files,
            "report": rendered.report,
        })));
    }

    if nonempty && !body.force {
        return Err(ApiError(
            StatusCode::CONFLICT,
            format!(
                "{} is not empty; choose another folder or allow overwriting",
                dir.display()
            ),
            None,
        ));
    }
    check_import_target(&state, &dir)?;
    import::write(&mut rendered, &dir, body.force).map_err(|e| match e {
        ImportError::NotEmpty(_) => ApiError(StatusCode::CONFLICT, e.to_string(), None),
        ImportError::Invalid(m) => bad(m),
        ImportError::Io(e) => e.into(),
    })?;
    let slot = {
        let mut registry = state.registry.write().unwrap();
        let id = registry.add(&dir)?;
        slot_json(registry.slot(&id).unwrap())
    };
    state.sync_watch();
    Ok(Json(json!({
        "dir": dir.display().to_string(),
        "collection": slot,
        "report": rendered.report,
    })))
}

fn dir_nonempty(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut d| d.next().is_some())
}

/// Parent folder for imports when none is given; created on the first import.
const IMPORT_HOME: &str = "sankh-collections";

/// `~/sankh-collections/<slug>`, or `<slug>-2`, `<slug>-3`, ... when that
/// folder is in use.
fn default_import_dir(slug: &str) -> std::path::PathBuf {
    let base = dirs::home_dir()
        .unwrap_or_else(|| ".".into())
        .join(IMPORT_HOME);
    let mut dir = base.join(slug);
    let mut n = 2;
    while dir_nonempty(&dir) {
        dir = base.join(format!("{slug}-{n}"));
        n += 1;
    }
    dir
}

/// Refuses an output folder inside or around an open collection before any
/// file is written, since the workspace would reject it afterwards.
fn check_import_target(state: &AppState, dir: &std::path::Path) -> Result<(), ApiError> {
    let target = resolve(dir);
    for (id, root) in state.registry.read().unwrap().roots() {
        if target.starts_with(&root) || root.starts_with(&target) {
            return Err(ApiError(
                StatusCode::CONFLICT,
                format!(
                    "{} overlaps `{id}` ({}); collections cannot be nested",
                    dir.display(),
                    root.display()
                ),
                None,
            ));
        }
    }
    Ok(())
}

/// Canonical form of a path that may not exist yet: the deepest existing
/// ancestor is canonicalized and the rest appended.
fn resolve(path: &std::path::Path) -> std::path::PathBuf {
    let mut existing = path.to_path_buf();
    let mut rest = Vec::new();
    while !existing.exists() {
        match (existing.file_name(), existing.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name.to_os_string());
                existing = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
    let mut out = existing.canonicalize().unwrap_or(existing);
    out.extend(rest.iter().rev());
    out
}

pub async fn envs(State(state): State<Arc<AppState>>, Path(cid): Path<String>) -> ApiResult {
    let c = collection(&state, &cid)?;
    Ok(Json(json!({
        "envs": env::list_envs(&c),
        "default": c.config.default_env,
    })))
}

/// Variables of an environment, for editor autocomplete. Secret values are masked.
pub async fn env_vars(
    State(state): State<Arc<AppState>>,
    Path((cid, name)): Path<(String, String)>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    let env_name = (name != "_").then_some(name.as_str());
    let env = Env::load(&c, env_name).map_err(|e| bad(e.to_string()))?;
    let redactor = Redactor::from_vars(&env.file_vars);
    let mut vars: Vec<Value> = env
        .file_vars
        .iter()
        .map(|(k, v)| json!({ "name": k, "value": redactor.display_var(k, v), "source": "file" }))
        .collect();
    let captures = state.captures.lock().unwrap();
    if let Some(caps) = captures.get(&(cid, name)) {
        for (k, v) in caps {
            vars.push(
                json!({ "name": k, "value": redactor.display_var(k, v), "source": "capture" }),
            );
        }
    }
    Ok(Json(json!({ "vars": vars })))
}

impl From<EnvEditError> for ApiError {
    fn from(e: EnvEditError) -> Self {
        let status = match &e {
            EnvEditError::Exists(_) => StatusCode::CONFLICT,
            EnvEditError::Missing(_) => StatusCode::NOT_FOUND,
            EnvEditError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_REQUEST,
        };
        ApiError(status, e.to_string(), None)
    }
}

/// Picks up `sankh.toml` changes and tells open views the collection changed.
fn env_changed(state: &AppState, cid: &str) {
    state.registry.write().unwrap().reload(cid);
    let _ = state.changes.send(cid.to_string());
}

fn file_vars_json(path: &std::path::Path) -> ApiResult {
    let vars: Vec<Value> = env::read_env_raw(path)?
        .into_iter()
        .map(|(k, v)| json!({ "secret": is_secret_name(&k), "name": k, "value": v }))
        .collect();
    Ok(Json(json!({ "vars": vars })))
}

#[derive(Deserialize)]
pub struct EnvVarBody {
    name: String,
    value: String,
}

#[derive(Deserialize)]
pub struct EnvFileBody {
    vars: Vec<EnvVarBody>,
}

impl EnvFileBody {
    fn pairs(self) -> Vec<(String, String)> {
        self.vars.into_iter().map(|v| (v.name, v.value)).collect()
    }
}

#[derive(Deserialize)]
pub struct CreateEnvBody {
    name: String,
    #[serde(default)]
    copy_from: Option<String>,
}

pub async fn create_env(
    State(state): State<Arc<AppState>>,
    Path(cid): Path<String>,
    Json(body): Json<CreateEnvBody>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    env::create_env(&c, body.name.trim(), body.copy_from.as_deref())?;
    env_changed(&state, &cid);
    Ok(Json(json!({ "name": body.name.trim() })))
}

#[derive(Deserialize)]
pub struct RenameEnvBody {
    name: String,
}

pub async fn rename_env(
    State(state): State<Arc<AppState>>,
    Path((cid, name)): Path<(String, String)>,
    Json(body): Json<RenameEnvBody>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    let to = body.name.trim();
    env::rename_env(&c, &name, to)?;
    let mut captures = state.captures.lock().unwrap();
    if let Some(caps) = captures.remove(&(cid.clone(), name)) {
        captures.insert((cid.clone(), to.to_string()), caps);
    }
    drop(captures);
    env_changed(&state, &cid);
    Ok(Json(json!({ "name": to })))
}

pub async fn delete_env(
    State(state): State<Arc<AppState>>,
    Path((cid, name)): Path<(String, String)>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    env::delete_env(&c, &name)?;
    state
        .captures
        .lock()
        .unwrap()
        .remove(&(cid.clone(), name.clone()));
    env_changed(&state, &cid);
    Ok(Json(json!({ "deleted": name })))
}

#[derive(Deserialize)]
pub struct DefaultEnvBody {
    name: Option<String>,
}

pub async fn put_default_env(
    State(state): State<Arc<AppState>>,
    Path(cid): Path<String>,
    Json(body): Json<DefaultEnvBody>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    let name = body.name.as_deref().filter(|n| !n.is_empty());
    env::set_default_env(&c, name)?;
    env_changed(&state, &cid);
    Ok(Json(json!({ "default": name })))
}

/// Variables of one environment file as written, unmasked; `secret` marks
/// names the UI should mask by default.
pub async fn get_env_file(
    State(state): State<Arc<AppState>>,
    Path((cid, name)): Path<(String, String)>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    file_vars_json(&env::existing_env(&c, &name)?)
}

pub async fn put_env_file(
    State(state): State<Arc<AppState>>,
    Path((cid, name)): Path<(String, String)>,
    Json(body): Json<EnvFileBody>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    let path = env::existing_env(&c, &name)?;
    env::write_env_file(&path, &body.pairs())?;
    env_changed(&state, &cid);
    file_vars_json(&path)
}

pub async fn get_env_local(
    State(state): State<Arc<AppState>>,
    Path(cid): Path<String>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    file_vars_json(&c.root.join(env::ENV_LOCAL))
}

pub async fn put_env_local(
    State(state): State<Arc<AppState>>,
    Path(cid): Path<String>,
    Json(body): Json<EnvFileBody>,
) -> ApiResult {
    let c = collection(&state, &cid)?;
    let path = c.root.join(env::ENV_LOCAL);
    env::write_env_file(&path, &body.pairs())?;
    env_changed(&state, &cid);
    file_vars_json(&path)
}

#[derive(Deserialize)]
pub struct EnvQuery {
    #[serde(default)]
    env: Option<String>,
}

pub async fn clear_captures(
    State(state): State<Arc<AppState>>,
    Path(cid): Path<String>,
    Query(q): Query<EnvQuery>,
) -> ApiResult {
    collection(&state, &cid)?;
    let key = q.env.unwrap_or_else(|| "_".into());
    state.captures.lock().unwrap().remove(&(cid, key.clone()));
    Ok(Json(json!({ "cleared": key })))
}

pub async fn get_trust(State(state): State<Arc<AppState>>, Path(cid): Path<String>) -> ApiResult {
    Ok(Json(trust_json(&collection(&state, &cid)?)))
}

pub async fn post_trust(State(state): State<Arc<AppState>>, Path(cid): Path<String>) -> ApiResult {
    let c = collection(&state, &cid)?;
    let mut store = TrustStore::load().map_err(|e| bad(e.to_string()))?;
    store.trust(&c.root).map_err(|e| bad(e.to_string()))?;
    store.save().map_err(|e| bad(e.to_string()))?;
    Ok(Json(trust_json(&c)))
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

pub async fn start_run(
    State(state): State<Arc<AppState>>,
    Path(cid): Path<String>,
    Json(body): Json<RunBody>,
) -> ApiResult {
    let collection = collection(&state, &cid)?;
    let trusted = trust::ensure(&collection, false).map_err(|e| {
        ApiError(
            StatusCode::FORBIDDEN,
            e.to_string(),
            Some(trust_json(&collection)),
        )
    })?;
    let target = collection.resolve(&body.path)?;
    if !target.exists() {
        return Err(not_found(format!("{} not found", body.path)));
    }
    let env_name = body.env.filter(|e| !e.is_empty());
    let mut env = Env::load(&collection, env_name.as_deref()).map_err(|e| bad(e.to_string()))?;
    let capture_key = (cid.clone(), env_name.clone().unwrap_or_else(|| "_".into()));
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
    handle
        .push(json!({ "type": "start", "collection": cid, "total": files.len(), "paths": paths }));

    let state2 = state.clone();
    tokio::task::spawn_blocking(move || {
        let mut options = RunOptions::for_collection(&collection);
        options.cancel = handle.cancel.clone();
        let mut ctx = RunContext::new(env, options);
        let mut results = Vec::new();
        for file in &files {
            if handle.cancelled() {
                break;
            }
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
        if handle.cancelled() {
            let skipped: Vec<String> = files[results.len()..]
                .iter()
                .map(|f| collection.rel(f))
                .collect();
            handle.push(json!({ "type": "cancelled", "skipped": skipped }));
        }
        handle.push(json!({ "type": "done", "summary": Summary::of(&results) }));
        handle.finish();
    });

    Ok(Json(json!({ "id": id })))
}

/// Stops a run: kills the request in flight and skips the rest.
pub async fn cancel_run(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult {
    let handle = state
        .runs
        .get(&id)
        .ok_or_else(|| not_found("unknown run"))?;
    let was_running = !handle.is_done();
    handle.cancel();
    Ok(Json(json!({ "cancelled": was_running })))
}

pub async fn run_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let handle = state
        .runs
        .get(&id)
        .ok_or_else(|| not_found("unknown run"))?;
    Ok(Sse::new(handle.stream()).keep_alive(KeepAlive::default()))
}

/// Server-sent `{"changed": "<collection id>"}` whenever files change;
/// `null` (after missed events) means every collection.
pub async fn change_events(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    use axum::response::sse::Event;
    use futures::StreamExt;
    let rx = state.changes.subscribe();
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx).map(|res| {
        Ok::<_, std::convert::Infallible>(
            Event::default().data(json!({ "changed": res.ok() }).to_string()),
        )
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
