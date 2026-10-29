//! The built-in Scratch collection: always available, for trying requests
//! without an existing folder. It lives in the user's data directory.

use crate::collection::{Collection, CollectionError};
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
}

/// Where Scratch lives: `<data_dir>/scratch`.
pub fn dir() -> PathBuf {
    crate::paths::data_dir().join("scratch")
}

/// Creates (if needed) and opens Scratch.
pub fn ensure() -> Result<Collection, ScratchError> {
    create_in(&dir())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_once() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scratch");
        let c = create_in(&dir).unwrap();
        assert_eq!(c.name(), "Scratch");
        assert_eq!(crate::env::list_envs(&c), vec!["default"]);

        std::fs::remove_file(dir.join("environments/default.env")).unwrap();
        create_in(&dir).unwrap();
        assert!(!dir.join("environments/default.env").exists());
    }
}
