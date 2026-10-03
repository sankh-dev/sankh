//! The Sankh desktop app: runs the `sankh serve` server in-process on a random
//! loopback port, guarded by a per-launch token, and shows its UI in a native
//! window. Requests still execute in Rust on this machine, as with the CLI.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod env_path;

use anyhow::{Context, Result};
use sankh_server::{AppState, Registry, serve_on};
use std::sync::Arc;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tokio::net::TcpListener;

const LISTEN: &str = "127.0.0.1";

fn main() {
    env_path::fix();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let (url, warning) = start_server()?;
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Sankh")
                .inner_size(1280.0, 800.0)
                .min_inner_size(720.0, 480.0)
                .build()?;
            if let Some(msg) = warning {
                app.dialog()
                    .message(msg)
                    .title("Sankh")
                    .kind(MessageDialogKind::Warning)
                    .show(|_| {});
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start the Sankh desktop app");
}

/// Starts the server and returns the URL to load, plus a warning to show when
/// the saved workspace could not be opened (the app then opens Scratch only).
fn start_server() -> Result<(tauri::Url, Option<String>)> {
    let (registry, warning) = match Registry::saved() {
        Ok(r) => (r, None),
        Err(e) => (
            Registry::session(&[], true).context("cannot open the Scratch collection")?,
            Some(format!(
                "Could not open your saved workspace: {e}\n\nOnly Scratch is open, and folders you add are kept for this session only."
            )),
        ),
    };

    let token = uuid::Uuid::new_v4().simple().to_string();
    let listener = tauri::async_runtime::block_on(TcpListener::bind((LISTEN, 0)))
        .context("cannot bind a loopback port")?;
    let port = listener.local_addr()?.port();
    let state = Arc::new(AppState::new(registry, Some(token.clone()), LISTEN, vec![]));

    tauri::async_runtime::spawn(async move {
        if let Err(e) = serve_on(listener, state).await {
            eprintln!("sankh server stopped: {e:#}");
        }
    });

    let url = format!("http://{LISTEN}:{port}/#token={token}").parse()?;
    Ok((url, warning))
}
