//! File watching for live tree refresh in the UI.

use notify::{RecursiveMode, Watcher};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// Starts watching `root`; change notifications are debounced. Returns the
/// watcher, which must be kept alive. Watching is best effort.
pub fn start(root: &Path, changes: broadcast::Sender<()>) -> Option<notify::RecommendedWatcher> {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            if !ev.kind.is_access() {
                let _ = tx.send(());
            }
        }
    })
    .ok()?;
    watcher.watch(root, RecursiveMode::Recursive).ok()?;
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            let deadline = Instant::now() + Duration::from_millis(150);
            while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                if rx.recv_timeout(left).is_err() {
                    break;
                }
            }
            let _ = changes.send(());
        }
    });
    Some(watcher)
}
