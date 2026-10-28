//! File watching for live tree refresh in the UI. One watcher covers every
//! collection root; roots are added and removed as the workspace changes.
//!
//! `~/sankh-collections` is watched too, recursively, so a folder copied in
//! becomes a collection once its `sankh.toml` or `environments/` appears.
//! Only an immediate child of that directory is adopted. Events inside a
//! collection that is already open belong to that collection, because the
//! longer root wins.

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use sankh_core::collection;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

type Roots = Arc<RwLock<Vec<(String, PathBuf)>>>;

/// Root id for the import-home watch. Not a collection id.
pub const IMPORT_ID: &str = "\0import";

/// Broadcast when the set of collections changed (a folder was adopted).
pub const COLLECTIONS_CHANGED: &str = "\0collections";

pub struct Watcher {
    inner: RecommendedWatcher,
    roots: Roots,
}

impl Watcher {
    /// Starts an (initially empty) watcher. Collection-file changes are
    /// debounced and sent as the collection id. A new collection folder under
    /// the import home is sent on `imports` instead. Watching is best effort.
    pub fn start(changes: broadcast::Sender<String>, imports: Sender<PathBuf>) -> Option<Watcher> {
        let roots: Roots = Arc::default();
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let lookup = roots.clone();
        let inner = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(ev) = res else { return };
            if ev.kind.is_access() {
                return;
            }
            let roots = lookup.read().unwrap();
            for path in &ev.paths {
                let owner = roots
                    .iter()
                    .filter(|(_, root)| path.starts_with(root))
                    .max_by_key(|(_, root)| root.as_os_str().len());
                let Some((id, root)) = owner else { continue };
                if id == IMPORT_ID {
                    if let Some(child) = collection::collection_child(root, path) {
                        let _ = tx.send(format!("\0{}", child.display()));
                    }
                } else {
                    let _ = tx.send(id.clone());
                }
            }
        })
        .ok()?;
        std::thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut ids = BTreeSet::from([first]);
                let deadline = Instant::now() + Duration::from_millis(150);
                while let Some(left) = deadline.checked_duration_since(Instant::now()) {
                    match rx.recv_timeout(left) {
                        Ok(id) => {
                            ids.insert(id);
                        }
                        Err(_) => break,
                    }
                }
                for id in ids {
                    if let Some(path) = id.strip_prefix('\0') {
                        if !path.is_empty() {
                            let _ = imports.send(PathBuf::from(path));
                        }
                    } else {
                        let _ = changes.send(id);
                    }
                }
            }
        });
        Some(Watcher { inner, roots })
    }

    /// Watches exactly `roots`, adding and removing watches as needed.
    pub fn set_roots(&mut self, new: Vec<(String, PathBuf)>) {
        // The event callback takes the read lock, and (un)watching may wait
        // on the event thread, so the lock must not be held while doing so.
        let old = self.roots.read().unwrap().clone();
        for (_, p) in &old {
            if !new.iter().any(|(_, n)| n == p) {
                let _ = self.inner.unwatch(p);
            }
        }
        for (_, p) in &new {
            if !old.iter().any(|(_, o)| o == p) {
                let _ = self.inner.watch(p, RecursiveMode::Recursive);
            }
        }
        *self.roots.write().unwrap() = new;
    }
}
