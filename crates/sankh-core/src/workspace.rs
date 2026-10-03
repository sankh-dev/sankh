//! The workspace: independent collections shown side by side in the UI.
//!
//! The saved list lives at `~/.config/sankh/workspace.toml` (or
//! `$SANKH_CONFIG_DIR`). Collections never share variables or captures; each
//! keeps its own environments and trust. The built-in Scratch collection is
//! always present and is never written to the file.

use crate::collection::{Collection, CollectionError};
use crate::scratch;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(default)]
    pub collections: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub path: PathBuf,
}

/// A workspace entry after trying to open it.
#[derive(Debug)]
pub struct Opened {
    pub id: String,
    pub path: PathBuf,
    pub collection: Result<Collection, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Collection(#[from] CollectionError),
    #[error("{path} is already in the workspace as `{id}`")]
    AlreadyAdded { id: String, path: String },
    #[error("{path} overlaps `{id}` ({other}); collections cannot be nested")]
    Overlaps {
        id: String,
        path: String,
        other: String,
    },
    #[error("Scratch is built in and cannot be added or unlinked")]
    Scratch,
    #[error("no collection `{0}` in the workspace")]
    Unknown(String),
    #[error("workspace file {path}: {message}")]
    Store { path: String, message: String },
}

impl Workspace {
    pub fn file() -> PathBuf {
        crate::paths::config_dir().join("workspace.toml")
    }

    pub fn load() -> Result<Workspace, WorkspaceError> {
        Self::load_from(&Self::file())
    }

    pub fn load_from(path: &Path) -> Result<Workspace, WorkspaceError> {
        if !path.is_file() {
            return Ok(Workspace::default());
        }
        let text = std::fs::read_to_string(path).map_err(|e| store_err(path, e))?;
        toml::from_str(&text).map_err(|e| store_err(path, e))
    }

    pub fn save(&self) -> Result<(), WorkspaceError> {
        self.save_to(&Self::file())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), WorkspaceError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| store_err(path, e))?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| store_err(path, e))?;
        std::fs::write(path, text).map_err(|e| store_err(path, e))
    }

    /// Adds the collection containing `path` and returns its entry.
    pub fn add(&mut self, path: &Path) -> Result<Entry, WorkspaceError> {
        self.add_reserving(path, &scratch::dir())
    }

    /// Like [`Workspace::add`], with the Scratch location given explicitly.
    pub fn add_reserving(
        &mut self,
        path: &Path,
        scratch_dir: &Path,
    ) -> Result<Entry, WorkspaceError> {
        let (collection, _) = Collection::discover(path)?;
        let root = collection.root.clone();
        let scratch_dir = scratch_dir
            .canonicalize()
            .unwrap_or_else(|_| scratch_dir.to_path_buf());
        if root.starts_with(&scratch_dir) {
            return Err(WorkspaceError::Scratch);
        }
        if scratch_dir.starts_with(&root) {
            return Err(WorkspaceError::Overlaps {
                id: scratch::ID.to_string(),
                path: root.display().to_string(),
                other: scratch_dir.display().to_string(),
            });
        }
        for e in &self.collections {
            if e.path == root {
                return Err(WorkspaceError::AlreadyAdded {
                    id: e.id.clone(),
                    path: root.display().to_string(),
                });
            }
            if root.starts_with(&e.path) || e.path.starts_with(&root) {
                return Err(WorkspaceError::Overlaps {
                    id: e.id.clone(),
                    path: root.display().to_string(),
                    other: e.path.display().to_string(),
                });
            }
        }
        let id = unique_id(&collection.name(), |id| {
            id == scratch::ID || self.collections.iter().any(|e| e.id == id)
        });
        let entry = Entry { id, path: root };
        self.collections.push(entry.clone());
        Ok(entry)
    }

    /// Finds an entry by id or by (canonical) folder path.
    pub fn find(&self, key: &str) -> Option<&Entry> {
        let canon = Path::new(key).canonicalize().ok();
        self.collections
            .iter()
            .find(|e| e.id == key || Some(&e.path) == canon.as_ref() || e.path == Path::new(key))
    }

    /// Unlinks an entry by id or path. Files on disk are untouched.
    pub fn remove(&mut self, key: &str) -> Result<Entry, WorkspaceError> {
        if key == scratch::ID {
            return Err(WorkspaceError::Scratch);
        }
        let id = self
            .find(key)
            .map(|e| e.id.clone())
            .ok_or_else(|| WorkspaceError::Unknown(key.to_string()))?;
        let pos = self.collections.iter().position(|e| e.id == id).unwrap();
        Ok(self.collections.remove(pos))
    }

    /// Opens Scratch (first) and every entry. Folders that cannot be opened
    /// are reported, not fatal.
    pub fn open_all(&self) -> Vec<Opened> {
        let mut out = vec![Opened {
            id: scratch::ID.to_string(),
            path: scratch::dir(),
            collection: scratch::ensure().map_err(|e| e.to_string()),
        }];
        out.extend(self.open_entries());
        out
    }

    /// Opens every entry, without Scratch.
    pub fn open_entries(&self) -> Vec<Opened> {
        self.collections
            .iter()
            .map(|e| Opened {
                id: e.id.clone(),
                path: e.path.clone(),
                collection: Collection::open(&e.path).map_err(|err| err.to_string()),
            })
            .collect()
    }
}

/// Lowercase ASCII slug: `Users API` becomes `users-api`.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "collection".into()
    } else {
        out
    }
}

/// `slug(name)`, with `-2`, `-3`, ... appended while `taken` says so.
pub fn unique_id(name: &str, taken: impl Fn(&str) -> bool) -> String {
    let base = slug(name);
    if !taken(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| !taken(id))
        .unwrap()
}

fn store_err(path: &Path, e: impl std::fmt::Display) -> WorkspaceError {
    WorkspaceError::Store {
        path: path.display().to_string(),
        message: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collection(root: &Path, rel: &str, name: Option<&str>) -> PathBuf {
        let dir = root.join(rel);
        std::fs::create_dir_all(dir.join("environments")).unwrap();
        if let Some(n) = name {
            std::fs::write(dir.join("sankh.toml"), format!("name = \"{n}\"\n")).unwrap();
        }
        dir
    }

    #[test]
    fn slugs_and_unique_ids() {
        assert_eq!(slug("Users API"), "users-api");
        assert_eq!(slug("  --Payments!! v2 "), "payments-v2");
        assert_eq!(slug("शंख"), "collection");
        let taken = ["api", "api-2"];
        assert_eq!(unique_id("API", |id| taken.contains(&id)), "api-3");
    }

    #[test]
    fn adds_finds_removes_and_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let scratch = root.join("data/scratch");
        let users = collection(root, "work/users", Some("API"));
        let payments = collection(root, "other/payments", Some("API"));
        let scratch_named = collection(root, "work/scratchy", Some("Scratch"));

        let mut ws = Workspace::default();
        let a = ws
            .add_reserving(&users.join("environments"), &scratch)
            .unwrap();
        assert_eq!(a.id, "api");
        assert_eq!(a.path, users.canonicalize().unwrap());
        let b = ws.add_reserving(&payments, &scratch).unwrap();
        assert_eq!(b.id, "api-2");
        let c = ws.add_reserving(&scratch_named, &scratch).unwrap();
        assert_eq!(c.id, "scratch-2");

        assert!(matches!(
            ws.add_reserving(&users, &scratch),
            Err(WorkspaceError::AlreadyAdded { .. })
        ));
        let nested = collection(root, "work/users/inner", None);
        assert!(matches!(
            ws.add_reserving(&nested, &scratch),
            Err(WorkspaceError::Overlaps { .. })
        ));
        collection(root, "data/scratch", None);
        assert!(matches!(
            ws.add_reserving(&scratch, &scratch),
            Err(WorkspaceError::Scratch)
        ));
        assert!(matches!(
            ws.add_reserving(&root.join("data"), &scratch),
            Err(WorkspaceError::Overlaps { id, .. }) if id == "scratch"
        ));

        let file = root.join("cfg/workspace.toml");
        ws.save_to(&file).unwrap();
        let mut back = Workspace::load_from(&file).unwrap();
        assert_eq!(back.collections, ws.collections);

        assert_eq!(back.find(users.to_str().unwrap()).unwrap().id, "api");
        assert!(matches!(
            back.remove("scratch"),
            Err(WorkspaceError::Scratch)
        ));
        assert!(matches!(
            back.remove("nope"),
            Err(WorkspaceError::Unknown(_))
        ));
        assert_eq!(back.remove("api-2").unwrap().id, "api-2");
        assert_eq!(back.remove(users.to_str().unwrap()).unwrap().id, "api");
        assert_eq!(back.collections.len(), 1);
        assert!(users.exists());
    }

    #[test]
    fn missing_folders_are_reported_not_fatal() {
        let tmp = tempfile::tempdir().unwrap();
        let users = collection(tmp.path(), "users", None);
        let mut ws = Workspace::default();
        ws.add_reserving(&users, &tmp.path().join("scratch"))
            .unwrap();
        std::fs::remove_dir_all(&users).unwrap();
        let opened = ws.open_entries();
        assert_eq!(opened.len(), 1);
        assert!(opened[0].collection.is_err());
    }
}
