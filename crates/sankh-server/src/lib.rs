//! `sankh serve`: a small HTTP server with the embedded web UI. The browser
//! is only a client; requests execute on the machine running the server.
//!
//! The server hosts a workspace of independent collections side by side.
//! Per-collection routes live under `/api/c/{id}/...`.

mod api;
mod assets;
mod registry;
mod runs;
mod security;
mod watch;

use anyhow::{Result, bail};
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use sankh_core::env::Vars;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

pub use registry::{Registry, Slot};

/// Postman exports with many saved responses easily exceed axum's 2 MB default.
const IMPORT_BODY_LIMIT: usize = 32 * 1024 * 1024;
pub use security::is_loopback;

/// Which collections to serve.
pub enum WorkspaceSource {
    /// The saved workspace (`~/.config/sankh/workspace.toml`) plus Scratch.
    Saved,
    /// These folders for this session only, plus Scratch.
    Session(Vec<PathBuf>),
}

pub struct ServeConfig {
    pub workspace: WorkspaceSource,
    pub listen: String,
    pub port: u16,
    pub token: Option<String>,
    pub allow_hosts: Vec<String>,
}

pub struct AppState {
    pub registry: RwLock<Registry>,
    pub token: Option<String>,
    /// Enforce the Host allow-list (always on for loopback binds).
    pub check_host: bool,
    pub allowed_hosts: Vec<String>,
    pub runs: runs::Runs,
    /// Captured values per (collection id, environment), so single-request
    /// runs in the UI can chain (log in once, then use the token).
    pub captures: Mutex<HashMap<(String, String), Vars>>,
    /// Ids of collections whose files changed.
    pub changes: broadcast::Sender<String>,
    watcher: Mutex<Option<watch::Watcher>>,
}

impl AppState {
    pub fn new(
        registry: Registry,
        token: Option<String>,
        listen: &str,
        allow_hosts: Vec<String>,
    ) -> AppState {
        let mut allowed_hosts: Vec<String> = ["localhost", "127.0.0.1", "[::1]"]
            .into_iter()
            .map(String::from)
            .collect();
        allowed_hosts.extend(allow_hosts.iter().map(|h| h.to_ascii_lowercase()));
        let check_host = is_loopback(listen) || !allow_hosts.is_empty();
        let (changes, _) = broadcast::channel(64);
        AppState {
            registry: RwLock::new(registry),
            token,
            check_host,
            allowed_hosts,
            runs: runs::Runs::default(),
            captures: Mutex::new(HashMap::new()),
            changes,
            watcher: Mutex::new(None),
        }
    }

    /// Starts watching every collection root for changes.
    pub fn start_watching(&self) {
        let mut guard = self.watcher.lock().unwrap();
        if guard.is_none() {
            *guard = watch::Watcher::start(self.changes.clone());
        }
        drop(guard);
        self.sync_watch();
    }

    /// Points the watcher (if running) at the current collection roots.
    fn sync_watch(&self) {
        let roots = self.registry.read().unwrap().roots();
        if let Some(w) = self.watcher.lock().unwrap().as_mut() {
            w.set_roots(roots);
        }
    }
}

pub fn app(state: Arc<AppState>) -> Router {
    let collection = Router::new()
        .route("/tree", get(api::tree))
        .route(
            "/request/{*path}",
            get(api::get_request)
                .put(api::put_request)
                .delete(api::delete_request),
        )
        .route("/copy", post(api::copy_request))
        .route("/parse", post(api::parse))
        .route("/envs", get(api::envs).post(api::create_env))
        .route(
            "/envs/{name}",
            get(api::env_vars)
                .patch(api::rename_env)
                .delete(api::delete_env),
        )
        .route(
            "/envs/{name}/file",
            get(api::get_env_file).put(api::put_env_file),
        )
        .route(
            "/env-local",
            get(api::get_env_local).put(api::put_env_local),
        )
        .route("/default-env", put(api::put_default_env))
        .route("/captures", delete(api::clear_captures))
        .route("/trust", get(api::get_trust).post(api::post_trust))
        .route("/run", post(api::start_run));
    let api = Router::new()
        .route("/info", get(api::info))
        .route("/collections", post(api::add_collection))
        .route("/collections/{cid}", delete(api::remove_collection))
        .route("/fs/dirs", get(api::list_dirs))
        .route("/render", post(api::render))
        .route("/import", post(api::import))
        .route(
            "/import/postman",
            post(api::import_postman).layer(DefaultBodyLimit::max(IMPORT_BODY_LIMIT)),
        )
        .route("/runs/{id}/events", get(api::run_events))
        .route("/events", get(api::change_events))
        .nest("/c/{cid}", collection);
    Router::new()
        .nest("/api", api)
        .fallback(assets::serve)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security::guard,
        ))
        .with_state(state)
}

pub async fn serve(cfg: ServeConfig) -> Result<()> {
    if !is_loopback(&cfg.listen) && cfg.token.is_none() {
        bail!(
            "listening on {} exposes request execution to the network; pass --token (or SANKH_TOKEN). Prefer an SSH tunnel or a TLS reverse proxy",
            cfg.listen
        );
    }
    let registry = match &cfg.workspace {
        WorkspaceSource::Saved => Registry::saved()?,
        WorkspaceSource::Session(paths) => Registry::session(paths, true)?,
    };
    println!("sankh serving");
    for slot in registry.slots() {
        match &slot.collection {
            Ok(c) => println!("  {:<16} {}", slot.id, c.root.display()),
            Err(e) => println!(
                "  {:<16} {} (unavailable: {e})",
                slot.id,
                slot.path.display()
            ),
        }
    }
    if !registry.persist() {
        println!("  (session only: changes to this list are not saved)");
    }
    let state = Arc::new(AppState::new(
        registry,
        cfg.token.clone(),
        &cfg.listen,
        cfg.allow_hosts,
    ));

    let host = if cfg.listen.contains(':') && !cfg.listen.starts_with('[') {
        format!("[{}]", cfg.listen)
    } else {
        cfg.listen.clone()
    };
    let listener = TcpListener::bind(format!("{host}:{}", cfg.port)).await?;
    let shown_host = if is_loopback(&cfg.listen) {
        "localhost".to_string()
    } else {
        host
    };
    let url = format!("http://{shown_host}:{}/", listener.local_addr()?.port());
    match &cfg.token {
        Some(t) => println!("open {url}#token={t}"),
        None => println!("open {url}"),
    }
    serve_on(listener, state).await
}

/// Serves `state` on a listener the caller bound (e.g. an embedding desktop
/// app on a random loopback port).
pub async fn serve_on(listener: TcpListener, state: Arc<AppState>) -> Result<()> {
    state.start_watching();
    axum::serve(listener, app(state)).await?;
    Ok(())
}
