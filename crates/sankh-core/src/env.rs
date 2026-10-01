//! Environment layering.
//!
//! Lowest to highest priority: `environments/<name>.env`, `.env.local`,
//! the process environment, then values captured earlier in the run.

use crate::collection::{Collection, ENV_DIR};
use std::collections::BTreeMap;
use std::path::Path;

pub type Vars = BTreeMap<String, String>;

#[derive(Debug, thiserror::Error)]
pub enum EnvError {
    #[error("environment `{name}` not found (available: {available})")]
    NotFound { name: String, available: String },
    #[error("{path}: {message}")]
    Parse { path: String, message: String },
}

#[derive(Debug, Clone, Default)]
pub struct Env {
    /// Values from the env file and `.env.local`, before the process env.
    pub file_vars: Vars,
    pub process: Vars,
    pub captures: Vars,
}

impl Env {
    /// Loads layers for the named environment (or the configured default, or none).
    pub fn load(collection: &Collection, name: Option<&str>) -> Result<Env, EnvError> {
        let mut file_vars = Vars::new();
        let name = name
            .map(str::to_string)
            .or_else(|| collection.config.default_env.clone());
        if let Some(name) = &name {
            let path = collection.root.join(ENV_DIR).join(format!("{name}.env"));
            if !path.is_file() {
                return Err(EnvError::NotFound {
                    name: name.clone(),
                    available: {
                        let list = list_envs(collection);
                        if list.is_empty() {
                            "none".into()
                        } else {
                            list.join(", ")
                        }
                    },
                });
            }
            file_vars.extend(read_env_file(&path)?);
        }
        let local = collection.root.join(".env.local");
        if local.is_file() {
            file_vars.extend(read_env_file(&local)?);
        }
        Ok(Env {
            file_vars,
            process: std::env::vars().collect(),
            captures: Vars::new(),
        })
    }

    /// Merged view of every layer.
    pub fn resolved(&self) -> Vars {
        let mut v = self.file_vars.clone();
        v.extend(self.process.clone());
        v.extend(self.captures.clone());
        v
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.captures
            .get(key)
            .or_else(|| self.process.get(key))
            .or_else(|| self.file_vars.get(key))
    }

    /// Variables referenced by `text` that are unset or empty in every layer.
    pub fn missing(&self, text: &str) -> Vec<String> {
        crate::shell::referenced_vars(text)
            .into_iter()
            .filter(|v| self.get(v).is_none_or(|s| s.is_empty()))
            .collect()
    }
}

pub fn read_env_file(path: &Path) -> Result<Vars, EnvError> {
    let mut out = Vars::new();
    let iter = dotenvy::from_path_iter(path).map_err(|e| EnvError::Parse {
        path: path.display().to_string(),
        message: e.to_string(),
    })?;
    for item in iter {
        let (k, v) = item.map_err(|e| EnvError::Parse {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        out.insert(k, v);
    }
    Ok(out)
}

/// Names of environments in `environments/*.env`, sorted.
pub fn list_envs(collection: &Collection) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(collection.root.join(ENV_DIR))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".env").map(str::to_string)
        })
        .collect();
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_in_priority_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("environments")).unwrap();
        std::fs::write(
            root.join("environments/dev.env"),
            "A=file\nB=file\nC=file\n",
        )
        .unwrap();
        std::fs::write(root.join(".env.local"), "B=local\nC=local\n").unwrap();
        let c = Collection::open(root).unwrap();
        let mut env = Env::load(&c, Some("dev")).unwrap();
        env.process = Vars::from([("C".to_string(), "process".to_string())]);
        env.captures.insert("D".into(), "cap".into());
        let r = env.resolved();
        assert_eq!(r["A"], "file");
        assert_eq!(r["B"], "local");
        assert_eq!(r["C"], "process");
        assert_eq!(r["D"], "cap");
        assert_eq!(env.missing("$A $NOPE ${D}"), vec!["NOPE"]);
        assert!(matches!(
            Env::load(&c, Some("prod")),
            Err(EnvError::NotFound { .. })
        ));
        assert_eq!(list_envs(&c), vec!["dev"]);
    }
}
