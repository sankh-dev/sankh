//! Choosing which requests a run includes.

use crate::collection::{Collection, CollectionError};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Filters {
    /// Folder names or relative paths; a request matches if it is inside any.
    pub folders: Vec<String>,
    /// A request matches if it has any of these tags.
    pub tags: Vec<String>,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty() && self.tags.is_empty()
    }
}

pub fn select(
    collection: &Collection,
    target: &Path,
    filters: &Filters,
) -> Result<Vec<PathBuf>, CollectionError> {
    let files = collection.request_files(target)?;
    if filters.is_empty() {
        return Ok(files);
    }
    Ok(files
        .into_iter()
        .filter(|f| {
            let rel = collection.rel(f);
            let folder_ok = filters.folders.is_empty()
                || filters.folders.iter().any(|want| in_folder(&rel, want));
            let tag_ok = filters.tags.is_empty() || {
                let content = std::fs::read_to_string(f).unwrap_or_default();
                let req = collection.parse_file(f, &content);
                filters.tags.iter().any(|t| req.tags.contains(t))
            };
            folder_ok && tag_ok
        })
        .collect())
}

fn in_folder(rel: &str, want: &str) -> bool {
    let want = want.trim_matches('/');
    if want.is_empty() {
        return true;
    }
    let dir = match rel.rsplit_once('/') {
        Some((d, _)) => d,
        None => return false,
    };
    let padded = format!("/{dir}/");
    dir == want || dir.starts_with(&format!("{want}/")) || padded.contains(&format!("/{want}/"))
}

#[cfg(test)]
mod tests {
    use super::in_folder;

    #[test]
    fn folder_matching() {
        assert!(in_folder("users/01-list.sh", "users"));
        assert!(in_folder("api/users/01-list.sh", "users"));
        assert!(in_folder("api/users/01-list.sh", "api/users"));
        assert!(!in_folder("users2/01-list.sh", "users"));
        assert!(!in_folder("top.sh", "users"));
    }
}
