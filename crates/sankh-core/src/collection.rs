//! Collection discovery: folder tree, ordering and `sankh.toml`.

use crate::parser::{self, strip_order_prefix};
use crate::request::Request;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub const CONFIG_FILE: &str = "sankh.toml";
pub const ENV_DIR: &str = "environments";
const SKIP_DIRS: &[&str] = &[ENV_DIR, "node_modules", "target"];

/// A folder Sankh treats as a collection root: it has `sankh.toml` or `environments/`.
pub fn is_collection_root(dir: &Path) -> bool {
    dir.join(CONFIG_FILE).is_file() || dir.join(ENV_DIR).is_dir()
}

/// Immediate child folders of `dir` that are collection roots, sorted by path.
pub fn child_collection_roots(dir: &Path) -> Result<Vec<PathBuf>, CollectionError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut children = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() && is_collection_root(&path) {
            children.push(path.canonicalize().unwrap_or(path));
        }
    }
    children.sort();
    Ok(children)
}

/// When `path` lies under `home`, the top-level child of `home` that contains
/// it, if that child is a collection root.
pub fn collection_child(home: &Path, path: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(home).ok()?;
    let name = rel.components().next()?.as_os_str();
    if name.to_string_lossy().starts_with('.') {
        return None;
    }
    let child = home.join(name);
    if child.is_dir() && is_collection_root(&child) {
        Some(child)
    } else {
        None
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub name: Option<String>,
    /// Collection format version.
    #[serde(default)]
    pub format: Option<u32>,
    #[serde(default)]
    pub default_env: Option<String>,
    /// Default per-request timeout in seconds (curl `--max-time`).
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Explicit ordering: folder path relative to the root (`""` for the root)
    /// mapped to entry names in the desired order. Unlisted entries follow.
    #[serde(default)]
    pub order: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Collection {
    pub root: PathBuf,
    pub config: Config,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Node {
    Folder {
        name: String,
        /// Path relative to the collection root, `/`-separated.
        path: String,
        children: Vec<Node>,
    },
    Request {
        name: String,
        path: String,
        tags: Vec<String>,
        method: Option<String>,
        raw: bool,
        errors: usize,
    },
}

impl Node {
    pub fn path(&self) -> &str {
        match self {
            Node::Folder { path, .. } | Node::Request { path, .. } => path,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CollectionError {
    #[error("{0} does not exist")]
    NotFound(PathBuf),
    #[error("invalid {CONFIG_FILE}: {0}")]
    Config(String),
    #[error("path `{0}` is outside the collection")]
    OutsideRoot(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Collection {
    /// Opens the collection that contains `path` (a file or folder). The root
    /// is the nearest ancestor with `sankh.toml` or `environments/`, falling
    /// back to the given folder (or the file's folder).
    pub fn discover(path: &Path) -> Result<(Collection, PathBuf), CollectionError> {
        let target = path
            .canonicalize()
            .map_err(|_| CollectionError::NotFound(path.to_path_buf()))?;
        let start = if target.is_dir() {
            target.clone()
        } else {
            target.parent().unwrap_or(&target).to_path_buf()
        };
        let root = start
            .ancestors()
            .find(|d| d.join(CONFIG_FILE).is_file() || d.join(ENV_DIR).is_dir())
            .map(Path::to_path_buf)
            .unwrap_or(start);
        Ok((Collection::open(&root)?, target))
    }

    /// Collection roots implied by `path`.
    ///
    /// One root when `path` is inside a collection. Several when `path` is a
    /// folder of collection roots (a shared zip, for example). Otherwise the
    /// folder itself, which may be a plain directory of `.sh` files.
    pub fn roots_for(path: &Path) -> Result<Vec<PathBuf>, CollectionError> {
        let target = path
            .canonicalize()
            .map_err(|_| CollectionError::NotFound(path.to_path_buf()))?;
        let start = if target.is_dir() {
            target.clone()
        } else {
            target.parent().unwrap_or(&target).to_path_buf()
        };
        if let Some(root) = start.ancestors().find(|d| is_collection_root(d)) {
            return Ok(vec![root.to_path_buf()]);
        }
        if target.is_dir() {
            let children = child_collection_roots(&target)?;
            if !children.is_empty() {
                return Ok(children);
            }
        }
        Ok(vec![start])
    }

    pub fn open(root: &Path) -> Result<Collection, CollectionError> {
        let root = root
            .canonicalize()
            .map_err(|_| CollectionError::NotFound(root.to_path_buf()))?;
        let config_path = root.join(CONFIG_FILE);
        let config = if config_path.is_file() {
            let text = std::fs::read_to_string(&config_path)?;
            toml::from_str(&text).map_err(|e| CollectionError::Config(e.to_string()))?
        } else {
            Config::default()
        };
        Ok(Collection { root, config })
    }

    pub fn name(&self) -> String {
        self.config.name.clone().unwrap_or_else(|| {
            self.root
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "collection".into())
        })
    }

    /// Converts an absolute path inside the collection to a `/`-separated relative path.
    pub fn rel(&self, abs: &Path) -> String {
        abs.strip_prefix(&self.root)
            .unwrap_or(abs)
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Resolves a relative path from a client, rejecting anything that could
    /// escape the root (absolute paths, `..`, symlinks pointing outside).
    pub fn resolve(&self, rel: &str) -> Result<PathBuf, CollectionError> {
        let rel_path = Path::new(rel);
        let mut out = self.root.clone();
        for comp in rel_path.components() {
            match comp {
                Component::Normal(c) => out.push(c),
                Component::CurDir => {}
                _ => return Err(CollectionError::OutsideRoot(rel.into())),
            }
        }
        // Canonicalize the deepest existing ancestor to catch symlink escapes.
        let existing = out
            .ancestors()
            .find(|p| p.exists())
            .unwrap_or(&self.root)
            .to_path_buf();
        let canon = existing.canonicalize()?;
        if !canon.starts_with(&self.root) {
            return Err(CollectionError::OutsideRoot(rel.into()));
        }
        Ok(out)
    }

    /// Builds the folder tree.
    pub fn tree(&self) -> Result<Node, CollectionError> {
        self.folder_node(&self.root)
    }

    fn folder_node(&self, dir: &Path) -> Result<Node, CollectionError> {
        let mut children = Vec::new();
        for entry in self.sorted_entries(dir)? {
            if entry.is_dir() {
                let node = self.folder_node(&entry)?;
                if let Node::Folder { children: c, .. } = &node {
                    if !c.is_empty() {
                        children.push(node);
                    }
                }
            } else {
                let content = std::fs::read_to_string(&entry).unwrap_or_default();
                let req = self.parse_file(&entry, &content);
                children.push(Node::Request {
                    name: req.name.clone(),
                    path: self.rel(&entry),
                    tags: req.tags.clone(),
                    method: req.curl.as_ref().map(|c| c.method.clone()),
                    raw: req.curl.is_none(),
                    errors: req
                        .diagnostics
                        .iter()
                        .filter(|d| d.severity == crate::request::Severity::Error)
                        .count(),
                });
            }
        }
        let name = if dir == self.root {
            self.name()
        } else {
            dir.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        Ok(Node::Folder {
            name,
            path: self.rel(dir),
            children,
        })
    }

    pub fn parse_file(&self, path: &Path, content: &str) -> Request {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        parser::parse(content, &file_name)
    }

    /// Request files and subfolders of `dir`, in run order.
    fn sorted_entries(&self, dir: &Path) -> Result<Vec<PathBuf>, CollectionError> {
        let mut entries: Vec<PathBuf> = Vec::new();
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = e.path();
            let ft = e.file_type()?;
            let is_dir = ft.is_dir() || (ft.is_symlink() && path.is_dir());
            if is_dir {
                if dir == self.root && SKIP_DIRS.contains(&name.as_str()) {
                    continue;
                }
                entries.push(path);
            } else if name.ends_with(".sh") {
                entries.push(path);
            }
        }
        let explicit = self.config.order.get(&self.rel(dir));
        entries.sort_by(|a, b| {
            let an = a.file_name().unwrap().to_string_lossy();
            let bn = b.file_name().unwrap().to_string_lossy();
            let ai = explicit.and_then(|o| o.iter().position(|n| *n == an));
            let bi = explicit.and_then(|o| o.iter().position(|n| *n == bn));
            match (ai, bi) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => order_key(&an).cmp(&order_key(&bn)),
            }
        });
        Ok(entries)
    }

    /// All request files under `target` (a file or folder), in run order.
    pub fn request_files(&self, target: &Path) -> Result<Vec<PathBuf>, CollectionError> {
        if target.is_file() {
            return Ok(vec![target.to_path_buf()]);
        }
        let mut out = Vec::new();
        self.collect(target, &mut out)?;
        Ok(out)
    }

    fn collect(&self, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), CollectionError> {
        for entry in self.sorted_entries(dir)? {
            if entry.is_dir() {
                self.collect(&entry, out)?;
            } else {
                out.push(entry);
            }
        }
        Ok(())
    }
}

/// Sort key: numeric prefix first (numerically), then name, case-insensitive.
fn order_key(name: &str) -> (u8, u64, String) {
    let digits: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
    let rest = strip_order_prefix(name).to_lowercase();
    match digits.parse::<u64>() {
        Ok(n) if strip_order_prefix(name).len() < name.len() => (0, n, rest),
        _ => (1, 0, name.to_lowercase()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    }

    #[test]
    fn orders_and_resolves() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "environments/dev.env", "BASE_URL=x\n");
        write(root, "users/10-delete.sh", "curl x\n");
        write(root, "users/2-create.sh", "curl x\n");
        write(root, "users/01-list.sh", "curl x\n");
        write(root, "users/zz.sh", "curl x\n");
        write(root, "auth/01-login.sh", "# @name Login\ncurl x\n");
        write(root, "README.md", "hi");
        let (c, _) = Collection::discover(&root.join("users")).unwrap();
        assert_eq!(c.root, root.canonicalize().unwrap());
        let files: Vec<String> = c
            .request_files(&c.root)
            .unwrap()
            .iter()
            .map(|p| c.rel(p))
            .collect();
        assert_eq!(
            files,
            vec![
                "auth/01-login.sh",
                "users/01-list.sh",
                "users/2-create.sh",
                "users/10-delete.sh",
                "users/zz.sh"
            ]
        );
        assert!(c.resolve("../etc/passwd").is_err());
        assert!(c.resolve("/etc/passwd").is_err());
        assert!(c.resolve("users/new.sh").is_ok());
    }

    #[test]
    fn explicit_order_wins() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "sankh.toml",
            "[order]\n\"\" = [\"users\", \"auth\"]\n",
        );
        write(root, "auth/a.sh", "curl x\n");
        write(root, "users/b.sh", "curl x\n");
        let c = Collection::open(root).unwrap();
        let files: Vec<String> = c
            .request_files(&c.root)
            .unwrap()
            .iter()
            .map(|p| c.rel(p))
            .collect();
        assert_eq!(files, vec!["users/b.sh", "auth/a.sh"]);
    }
}
