//! `sankh serve`: a small HTTP server with the embedded web UI. The browser
//! is only a client; requests execute on the machine running the server.

mod api;
mod assets;
mod runs;
mod security;
mod watch;

use anyhow::{Result, bail};
use axum::Router;
use axum::routing::{get, post};
use sankh_core::Collection;
use sankh_core::env::Vars;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

pub use security::is_loopback;

pub struct ServeConfig {
    pub collection: Collection,
    pub listen: String,
    pub port: u16,
    pub token: Option<String>,
    pub allow_hosts: Vec<String>,
}

pub struct AppState {
    pub collection: Collection,
    pub token: Option<String>,
    /// Enforce the Host allow-list (always on for loopback binds).
    pub check_host: bool,
    pub allowed_hosts: Vec<String>,
    pub runs: runs::Runs,
    /// Captured values per environment, so single-request runs in the UI can
    /// chain (log in once, then use the token).
    pub captures: Mutex<HashMap<String, Vars>>,
    pub changes: broadcast::Sender<()>,
}

impl AppState {
    pub fn new(
        collection: Collection,
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
        let (changes, _) = broadcast::channel(16);
        AppState {
            collection,
            token,
            check_host,
            allowed_hosts,
            runs: runs::Runs::default(),
            captures: Mutex::new(HashMap::new()),
            changes,
        }
    }
}

pub fn app(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/info", get(api::info))
        .route("/tree", get(api::tree))
        .route(
            "/request/{*path}",
            get(api::get_request)
                .put(api::put_request)
                .delete(api::delete_request),
        )
        .route("/parse", post(api::parse))
        .route("/render", post(api::render))
        .route("/import", post(api::import))
        .route("/envs", get(api::envs))
        .route("/envs/{name}", get(api::env_vars))
        .route("/captures", axum::routing::delete(api::clear_captures))
        .route("/trust", get(api::get_trust).post(api::post_trust))
        .route("/run", post(api::start_run))
        .route("/runs/{id}/events", get(api::run_events))
        .route("/events", get(api::change_events));
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
    let state = Arc::new(AppState::new(
        cfg.collection,
        cfg.token.clone(),
        &cfg.listen,
        cfg.allow_hosts,
    ));
    let _watcher = watch::start(&state.collection.root, state.changes.clone());

    let host = if cfg.listen.contains(':') && !cfg.listen.starts_with('[') {
        format!("[{}]", cfg.listen)
    } else {
        cfg.listen.clone()
    };
    let listener = tokio::net::TcpListener::bind(format!("{host}:{}", cfg.port)).await?;
    let shown_host = if is_loopback(&cfg.listen) {
        "localhost".to_string()
    } else {
        host
    };
    let url = format!("http://{shown_host}:{}/", cfg.port);
    println!("sankh serving {} ", state.collection.root.display());
    match &cfg.token {
        Some(t) => println!("open {url}#token={t}"),
        None => println!("open {url}"),
    }
    axum::serve(listener, app(state)).await?;
    Ok(())
}
