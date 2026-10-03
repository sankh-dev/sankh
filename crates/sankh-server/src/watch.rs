//! File watching for live tree refresh in the UI. One watcher covers every
//! collection root; roots are added and removed as the workspace changes.

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

type Roots = Arc<RwLock<Vec<(String, PathBuf)>>>;

pub struct Watcher {
    inner: RecommendedWatcher,
    roots: Roots,
}

impl Watcher {
    /// Starts an (initially empty) watcher. Changes are debounced and sent as
    /// the id of the collection they belong to. Watching is best effort.
    pub fn start(changes: broadcast::Sender<String>) -> Option<Watcher> {
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
                if let Some((id, _)) = owner {
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
                    let _ = changes.send(id);
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
