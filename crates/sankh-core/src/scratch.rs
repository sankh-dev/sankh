//! The built-in Scratch collection: always available, for trying requests
//! without an existing folder. It lives in the user's data directory and is
//! trusted automatically, because only Sankh and the user write to it.

use crate::collection::{Collection, CollectionError};
use crate::trust::{TrustError, TrustStatus, TrustStore};
use std::path::{Path, PathBuf};

pub const ID: &str = "scratch";

const FILES: &[(&str, &str)] = &[
    (
        "sankh.toml",
        "name = \"Scratch\"\nformat = 1\ndefault_env = \"default\"\n",
    ),
    (
        "environments/default.env",
        "# Variables for Scratch requests\nBASE_URL=http://localhost:8080\n",
    ),
    (".gitignore", ".env.local\n"),
];

#[derive(Debug, thiserror::Error)]
pub enum ScratchError {
    #[error("creating Scratch at {0}: {1}")]
    Create(String, std::io::Error),
    #[error(transparent)]
    Collection(#[from] CollectionError),
    #[error(transparent)]
    Trust(#[from] TrustError),
}

/// Where Scratch lives: `<data_dir>/scratch`.
pub fn dir() -> PathBuf {
    crate::paths::data_dir().join("scratch")
}

/// Creates (if needed), trusts and opens Scratch.
pub fn ensure() -> Result<Collection, ScratchError> {
    let collection = create_in(&dir())?;
    let mut store = TrustStore::load()?;
    if trust_in(&mut store, &collection.root)? {
        store.save()?;
    }
    Ok(collection)
}

/// Creates the skeleton when `dir` does not exist yet; an existing folder is
/// left exactly as the user left it.
pub fn create_in(dir: &Path) -> Result<Collection, ScratchError> {
    if !dir.exists() {
        let err = |e| ScratchError::Create(dir.display().to_string(), e);
        for (rel, content) in FILES {
            let p = dir.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(err)?;
            }
            std::fs::write(&p, content).map_err(err)?;
        }
    }
    Ok(Collection::open(dir)?)
}

/// Trusts `root` unless it already is; returns whether the store changed.
pub fn trust_in(store: &mut TrustStore, root: &Path) -> Result<bool, TrustError> {
    if matches!(store.status(root), TrustStatus::Trusted { .. }) {
        return Ok(false);
    }
    store.trust(root)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_once_and_trusts() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scratch");
        let c = create_in(&dir).unwrap();
        assert_eq!(c.name(), "Scratch");
        assert_eq!(crate::env::list_envs(&c), vec!["default"]);

        std::fs::remove_file(dir.join("environments/default.env")).unwrap();
        create_in(&dir).unwrap();
        assert!(!dir.join("environments/default.env").exists());

        let mut store = TrustStore::default();
        assert!(trust_in(&mut store, &c.root).unwrap());
        assert!(!trust_in(&mut store, &c.root).unwrap());
    }
}
