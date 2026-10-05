//! The collections a server hosts: Scratch first, then the workspace entries
//! in order. A saved registry writes changes to `workspace.toml`; a session
//! registry (`sankh serve A B`) keeps them in memory.

use sankh_core::Collection;
use sankh_core::scratch;
use sankh_core::workspace::{Opened, Workspace, WorkspaceError};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Slot {
    pub id: String,
    pub path: PathBuf,
    /// `Err` when the folder is missing or cannot be opened.
    pub collection: Result<Collection, String>,
}

impl Slot {
    pub fn is_scratch(&self) -> bool {
        self.id == scratch::ID
    }

    fn from(o: Opened) -> Slot {
        Slot {
            id: o.id,
            path: o.path,
            collection: o.collection,
        }
    }
}

#[derive(Debug)]
pub struct Registry {
    workspace: Workspace,
    persist: bool,
    slots: Vec<Slot>,
}

impl Registry {
    /// The saved workspace, plus Scratch.
    pub fn saved() -> Result<Registry, WorkspaceError> {
        let workspace = Workspace::load()?;
        Ok(Registry::build(workspace, true, true))
    }

    /// A session-only workspace of `paths` (plus Scratch when `with_scratch`).
    pub fn session(paths: &[PathBuf], with_scratch: bool) -> Result<Registry, WorkspaceError> {
        let mut workspace = Workspace::default();
        for p in paths {
            workspace.add(p)?;
        }
        Ok(Registry::build(workspace, false, with_scratch))
    }

    fn build(workspace: Workspace, persist: bool, with_scratch: bool) -> Registry {
        let slots = if with_scratch {
            workspace.open_all()
        } else {
            workspace.open_entries()
        };
        Registry {
            slots: slots.into_iter().map(Slot::from).collect(),
            workspace,
            persist,
        }
    }

    pub fn persist(&self) -> bool {
        self.persist
    }

    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }

    pub fn slot(&self, id: &str) -> Option<&Slot> {
        self.slots.iter().find(|s| s.id == id)
    }

    /// Roots of the collections that opened, for file watching.
    pub fn roots(&self) -> Vec<(String, PathBuf)> {
        self.slots
            .iter()
            .filter_map(|s| {
                let c = s.collection.as_ref().ok()?;
                Some((s.id.clone(), c.root.clone()))
            })
            .collect()
    }

    /// Re-reads a collection's `sankh.toml` after it was edited.
    pub fn reload(&mut self, id: &str) {
        let Some(slot) = self.slots.iter_mut().find(|s| s.id == id) else {
            return;
        };
        if let Some(fresh) = slot
            .collection
            .as_ref()
            .ok()
            .and_then(|c| Collection::open(&c.root).ok())
        {
            slot.collection = Ok(fresh);
        }
    }

    /// Adds the collection containing `path`; returns its id.
    pub fn add(&mut self, path: &Path) -> Result<String, WorkspaceError> {
        let id = self.mutate(|ws| ws.add(path).map(|e| e.id))?;
        Ok(id)
    }

    /// Unlinks a collection. Files on disk are untouched.
    pub fn remove(&mut self, id: &str) -> Result<(), WorkspaceError> {
        self.mutate(|ws| ws.remove(id).map(|_| ()))
    }

    /// Applies a change to the workspace. A saved workspace is re-read first,
    /// so entries added meanwhile with `sankh workspace add` are kept.
    fn mutate<T>(
        &mut self,
        f: impl FnOnce(&mut Workspace) -> Result<T, WorkspaceError>,
    ) -> Result<T, WorkspaceError> {
        let mut ws = if self.persist {
            Workspace::load()?
        } else {
            self.workspace.clone()
        };
        let out = f(&mut ws)?;
        if self.persist {
            ws.save()?;
        }
        self.workspace = ws;
        let scratch = self.slots.iter().find(|s| s.is_scratch()).cloned();
        self.slots = scratch
            .into_iter()
            .chain(self.workspace.open_entries().into_iter().map(Slot::from))
            .collect();
        Ok(out)
    }
}
