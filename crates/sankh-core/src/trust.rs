//! Folder trust: request files are shell scripts, so a cloned collection
//! must never run until the user trusts it.
//!
//! The store lives at `~/.config/sankh/trust.toml` (or `$SANKH_CONFIG_DIR`),
//! keyed by canonical folder path. When the folder is in a git repository the
//! HEAD commit is recorded, and trust lapses when HEAD changes (e.g. after a
//! `git pull`) so new code is reviewed before it runs.

use crate::collection::Collection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrustStore {
    #[serde(default)]
    pub folders: BTreeMap<String, TrustEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustEntry {
    #[serde(default)]
    pub head: Option<String>,
    pub trusted_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum TrustStatus {
    Trusted {
        path: String,
    },
    Untrusted,
    /// Trusted before, but git HEAD has moved since.
    Changed {
        path: String,
        trusted_head: String,
        current_head: String,
    },
}

/// Proof that the collection may execute. Only obtainable through [`ensure`].
#[derive(Debug, Clone)]
pub struct Trusted {
    _private: (),
}

#[derive(Debug, thiserror::Error)]
pub enum TrustError {
    #[error(
        "{0} is not trusted. Request files are shell scripts; review them, then run `sankh trust {0}` (or pass --trust in CI)"
    )]
    Untrusted(String),
    #[error(
        "{path} changed since it was trusted (git HEAD {old} -> {new}). Review the changes, then run `sankh trust {path}`"
    )]
    Changed {
        path: String,
        old: String,
        new: String,
    },
    #[error("trust store {path}: {message}")]
    Store { path: String, message: String },
}

pub fn store_path() -> PathBuf {
    if let Some(dir) = std::env::var_os("SANKH_CONFIG_DIR") {
        return PathBuf::from(dir).join("trust.toml");
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sankh")
        .join("trust.toml")
}

impl TrustStore {
    pub fn load() -> Result<TrustStore, TrustError> {
        let path = store_path();
        if !path.is_file() {
            return Ok(TrustStore::default());
        }
        let text = std::fs::read_to_string(&path).map_err(|e| store_err(&path, e))?;
        toml::from_str(&text).map_err(|e| store_err(&path, e))
    }

    pub fn save(&self) -> Result<(), TrustError> {
        let path = store_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| store_err(&path, e))?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| store_err(&path, e))?;
        std::fs::write(&path, text).map_err(|e| store_err(&path, e))
    }

    pub fn status(&self, folder: &Path) -> TrustStatus {
        let Ok(folder) = folder.canonicalize() else {
            return TrustStatus::Untrusted;
        };
        for dir in folder.ancestors() {
            let key = dir.to_string_lossy().into_owned();
            if let Some(entry) = self.folders.get(&key) {
                let current = git_head(dir);
                return match (&entry.head, current) {
                    (Some(old), Some(new)) if *old != new => TrustStatus::Changed {
                        path: key,
                        trusted_head: old.clone(),
                        current_head: new,
                    },
                    _ => TrustStatus::Trusted { path: key },
                };
            }
        }
        TrustStatus::Untrusted
    }

    pub fn trust(&mut self, folder: &Path) -> Result<String, TrustError> {
        let folder = folder.canonicalize().map_err(|e| store_err(folder, e))?;
        let key = folder.to_string_lossy().into_owned();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.folders.insert(
            key.clone(),
            TrustEntry {
                head: git_head(&folder),
                trusted_at: now,
            },
        );
        Ok(key)
    }

    pub fn revoke(&mut self, folder: &Path) -> bool {
        let key = folder
            .canonicalize()
            .unwrap_or_else(|_| folder.to_path_buf())
            .to_string_lossy()
            .into_owned();
        self.folders.remove(&key).is_some()
    }
}

fn store_err(path: &Path, e: impl std::fmt::Display) -> TrustError {
    TrustError::Store {
        path: path.display().to_string(),
        message: e.to_string(),
    }
}

/// Grants execution for `collection` if it is trusted, or if `override_trust`
/// is set (CI: `--trust` / `SANKH_TRUST=1`).
pub fn ensure(collection: &Collection, override_trust: bool) -> Result<Trusted, TrustError> {
    if override_trust {
        return Ok(Trusted { _private: () });
    }
    let store = TrustStore::load()?;
    let shown = collection.root.display().to_string();
    match store.status(&collection.root) {
        TrustStatus::Trusted { .. } => Ok(Trusted { _private: () }),
        TrustStatus::Untrusted => Err(TrustError::Untrusted(shown)),
        TrustStatus::Changed {
            trusted_head,
            current_head,
            ..
        } => Err(TrustError::Changed {
            path: shown,
            old: short(&trusted_head),
            new: short(&current_head),
        }),
    }
}

fn short(sha: &str) -> String {
    sha.chars().take(8).collect()
}

/// Current git HEAD commit for the repository containing `dir`, if any.
pub fn git_head(dir: &Path) -> Option<String> {
    let git = dir
        .ancestors()
        .map(|d| d.join(".git"))
        .find(|p| p.exists())?;
    let git_dir = if git.is_file() {
        let text = std::fs::read_to_string(&git).ok()?;
        let p = PathBuf::from(text.strip_prefix("gitdir:")?.trim());
        if p.is_absolute() {
            p
        } else {
            git.parent()?.join(p)
        }
    } else {
        git
    };
    let common = std::fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .map(|c| git_dir.join(c.trim()))
        .unwrap_or_else(|| git_dir.clone());
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    let Some(r) = head.strip_prefix("ref:") else {
        return Some(head.to_string());
    };
    let r = r.trim();
    for base in [&git_dir, &common] {
        if let Ok(sha) = std::fs::read_to_string(base.join(r)) {
            return Some(sha.trim().to_string());
        }
    }
    let packed = std::fs::read_to_string(common.join("packed-refs")).ok()?;
    packed.lines().find_map(|l| {
        let (sha, name) = l.split_once(' ')?;
        (name.trim() == r).then(|| sha.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_change_revokes_trust() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
        std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(root.join(".git/refs/heads/main"), "aaaa1111\n").unwrap();
        std::fs::create_dir_all(root.join("api")).unwrap();

        let mut store = TrustStore::default();
        assert_eq!(store.status(&root.join("api")), TrustStatus::Untrusted);
        store.trust(&root.join("api")).unwrap();
        assert!(matches!(
            store.status(&root.join("api")),
            TrustStatus::Trusted { .. }
        ));

        std::fs::write(root.join(".git/refs/heads/main"), "bbbb2222\n").unwrap();
        assert!(matches!(
            store.status(&root.join("api")),
            TrustStatus::Changed { .. }
        ));
        assert!(store.revoke(&root.join("api")));
        assert_eq!(store.status(&root.join("api")), TrustStatus::Untrusted);
    }

    #[test]
    fn reads_packed_refs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::write(
            root.join(".git/packed-refs"),
            "# pack-refs\ncccc3333 refs/heads/main\n",
        )
        .unwrap();
        assert_eq!(git_head(root).as_deref(), Some("cccc3333"));
    }
}
