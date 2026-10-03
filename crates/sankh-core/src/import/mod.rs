//! Importers from other API clients into Sankh collections.
//!
//! Each reader turns its source format into the [`ImportedCollection`]
//! intermediate model; [`writer`] renders that model into request files with
//! the same serializer the UI uses, and reports everything that could not be
//! converted exactly.

pub mod postman;
mod postman_script;
pub mod writer;

use crate::request::CurlCommand;
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::LazyLock;

pub use writer::{ImportOutput, render, write};

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0} is not empty (use --force to write into it anyway)")]
    NotEmpty(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Source-agnostic collection produced by a reader.
#[derive(Debug, Clone, Default)]
pub struct ImportedCollection {
    pub name: String,
    pub root: ImportedFolder,
    /// Already converted: renamed keys, `${VAR}` references, secrets removed.
    pub environments: Vec<ImportedEnv>,
    pub vars: VarMap,
    /// Variables that must be provided outside the collection (`.env.example`).
    pub placeholders: Vec<Placeholder>,
}

impl ImportedCollection {
    pub fn add_placeholder(&mut self, name: &str, note: &str) {
        if !self.placeholders.iter().any(|p| p.name == name) {
            self.placeholders.push(Placeholder {
                name: name.to_string(),
                note: note.to_string(),
            });
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ImportedFolder {
    pub name: String,
    pub items: Vec<ImportedItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ImportedItem {
    Folder(ImportedFolder),
    Request(Box<ImportedRequest>),
}

#[derive(Debug, Clone, Default)]
pub struct ImportedRequest {
    pub name: String,
    pub description: Option<String>,
    /// Words follow the Sankh convention: `$VAR` expands, `\$` is literal.
    pub curl: CurlCommand,
    /// `@expect` arguments, e.g. `status 201`.
    pub expects: Vec<String>,
    /// `@capture` arguments, e.g. `TOKEN=.token`.
    pub captures: Vec<String>,
    /// Extra header lines, each starting with `#`.
    pub comments: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ImportedEnv {
    pub name: String,
    /// Key and dotenv-ready value (see [`env_value`]).
    pub vars: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Placeholder {
    pub name: String,
    pub note: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ImportReport {
    pub collection: String,
    pub requests: usize,
    pub folders: usize,
    pub environments: Vec<String>,
    pub renamed: Vec<Rename>,
    pub placeholders: Vec<Placeholder>,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rename {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Warning {
    /// Generated file or folder the warning is about (`""` for the collection).
    pub path: String,
    pub message: String,
}

static TEMPLATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap());

/// Piece of source text: literal text or a `{{variable}}` reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Text(String),
    /// Renamed (UPPER_SNAKE) variable name.
    Var(String),
}

/// Maps source variable names (`baseUrl`, `$guid`) to Sankh names
/// (`BASE_URL`, `GUID`), consistently across every file of one import.
#[derive(Debug, Clone, Default)]
pub struct VarMap {
    names: BTreeMap<String, String>,
    used: BTreeSet<String>,
    /// Renamed names of dynamic variables such as `{{$guid}}`.
    pub dynamic: BTreeSet<String>,
}

impl VarMap {
    pub fn rename(&mut self, original: &str) -> String {
        let original = original.trim();
        if let Some(n) = self.names.get(original) {
            return n.clone();
        }
        let dynamic = original.starts_with('$');
        let name = self.fresh(&upper_snake(original.trim_start_matches('$')));
        if dynamic {
            self.dynamic.insert(name.clone());
        }
        self.names.insert(original.to_string(), name.clone());
        name
    }

    /// Reserves `base` (or `base_2`, `base_3`, ...) for a generated variable.
    pub fn fresh(&mut self, base: &str) -> String {
        let mut name = base.to_string();
        let mut n = 2;
        while self.used.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        self.used.insert(name.clone());
        name
    }

    /// Source names that were changed, sorted by source name.
    pub fn renames(&self) -> Vec<Rename> {
        self.names
            .iter()
            .filter(|(from, to)| from != to)
            .map(|(from, to)| Rename {
                from: from.clone(),
                to: to.clone(),
            })
            .collect()
    }

    pub fn segments(&mut self, text: &str) -> Vec<Segment> {
        let mut out = Vec::new();
        let mut last = 0;
        for m in TEMPLATE.captures_iter(text) {
            let whole = m.get(0).unwrap();
            if whole.start() > last {
                out.push(Segment::Text(text[last..whole.start()].to_string()));
            }
            out.push(Segment::Var(self.rename(&m[1])));
            last = whole.end();
        }
        if last < text.len() {
            out.push(Segment::Text(text[last..].to_string()));
        }
        out
    }

    /// Converts source text to a Sankh word: `{{x}}` becomes `${X}` and a
    /// literal `$` becomes `\$`.
    pub fn word(&mut self, text: &str) -> String {
        self.segments(text)
            .into_iter()
            .map(|s| match s {
                Segment::Text(t) => t.replace('$', "\\$"),
                Segment::Var(v) => format!("${{{v}}}"),
            })
            .collect()
    }

    /// Converts source text to a dotenv value that reads back to the same
    /// text, with `{{x}}` substituted as `${X}`.
    pub fn env_value(&mut self, text: &str) -> String {
        let segs = self.segments(text);
        let plain = match segs.as_slice() {
            [] => Some(""),
            [Segment::Text(t)] => Some(t.as_str()),
            _ => None,
        };
        if let Some(t) = plain.filter(|t| {
            t.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_./:@%+,=-".contains(c))
        }) {
            return t.to_string();
        }
        let mut out = String::from("\"");
        for s in &segs {
            match s {
                Segment::Text(t) => {
                    for c in t.chars() {
                        match c {
                            '\\' | '"' | '$' => {
                                out.push('\\');
                                out.push(c);
                            }
                            '\n' => out.push_str("\\n"),
                            '\r' => {}
                            c => out.push(c),
                        }
                    }
                }
                Segment::Var(v) => out.push_str(&format!("${{{v}}}")),
            }
        }
        out.push('"');
        out
    }
}

/// `baseUrl` -> `BASE_URL`, `api-key` -> `API_KEY`, `HTTPServer` -> `HTTP_SERVER`.
pub fn upper_snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && i > 0 {
                let prev = chars[i - 1];
                let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
                if prev.is_ascii_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_ascii_uppercase() && next_lower)
                {
                    out.push('_');
                }
            }
            out.push(c.to_ascii_uppercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "VAR".into()
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{out}")
    } else {
        out
    }
}

/// File-system friendly name: `Create Pet (v2)` -> `create-pet-v2`.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut out: String = out.trim_matches('-').chars().take(60).collect();
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "request".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_snake_names() {
        for (from, to) in [
            ("baseUrl", "BASE_URL"),
            ("base_url", "BASE_URL"),
            ("api-key", "API_KEY"),
            ("HTTPServer", "HTTP_SERVER"),
            ("user.id", "USER_ID"),
            ("token", "TOKEN"),
            ("v2Path", "V2_PATH"),
            ("1st", "_1ST"),
            ("--", "VAR"),
        ] {
            assert_eq!(upper_snake(from), to, "{from}");
        }
    }

    #[test]
    fn var_map_is_consistent_and_avoids_collisions() {
        let mut v = VarMap::default();
        assert_eq!(v.rename("baseUrl"), "BASE_URL");
        assert_eq!(v.rename("base_url"), "BASE_URL_2");
        assert_eq!(v.rename("baseUrl"), "BASE_URL");
        assert_eq!(v.rename("$guid"), "GUID");
        assert!(v.dynamic.contains("GUID"));
        assert_eq!(v.fresh("BASE_URL"), "BASE_URL_3");
    }

    #[test]
    fn converts_words() {
        let mut v = VarMap::default();
        assert_eq!(
            v.word("{{baseUrl}}/x?p=$5&id={{ userId }}"),
            "${BASE_URL}/x?p=\\$5&id=${USER_ID}"
        );
    }

    #[test]
    fn env_values_read_back() {
        let mut v = VarMap::default();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.env");
        let cases = [
            "plain",
            "",
            "a b",
            "it's \"q\" $HOME \\ #x",
            "line1\nline2",
            "ünï",
        ];
        let text: String = cases
            .iter()
            .enumerate()
            .map(|(i, c)| format!("K{i}={}\n", v.env_value(c)))
            .collect();
        std::fs::write(&path, text).unwrap();
        let back = crate::env::read_env_file(&path).unwrap();
        for (i, c) in cases.iter().enumerate() {
            assert_eq!(back[&format!("K{i}")], *c);
        }
        assert_eq!(v.env_value("{{host}}/api"), "\"${HOST}/api\"");
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Create Pet (v2)"), "create-pet-v2");
        assert_eq!(slug("  ¿Qué?  "), "qué");
        assert_eq!(slug("!!!"), "request");
    }
}
