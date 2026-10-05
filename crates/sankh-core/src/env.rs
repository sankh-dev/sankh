//! Environment layering.
//!
//! Lowest to highest priority: `environments/<name>.env`, `.env.local`,
//! the process environment, then values captured earlier in the run.

use crate::collection::{Collection, ENV_DIR};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub type Vars = BTreeMap<String, String>;

/// Gitignored local overrides at the collection root.
pub const ENV_LOCAL: &str = ".env.local";

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
        let local = collection.root.join(ENV_LOCAL);
        if local.is_file() {
            file_vars.extend(read_env_file(&local)?);
        }
        Ok(Env {
            file_vars,
            process: host_process_vars(),
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

/// The process environment, minus what an AppImage launcher injected.
///
/// The desktop AppImage points `LD_LIBRARY_PATH`, `XDG_DATA_DIRS`, GTK
/// variables and so on into its own mount. Request files run the system's
/// `curl`, which breaks when it loads the AppImage's older libraries.
pub fn host_process_vars() -> Vars {
    let vars: Vars = std::env::vars().collect();
    match (vars.get("APPIMAGE"), vars.get("APPDIR")) {
        (Some(_), Some(appdir)) if !appdir.is_empty() => {
            let appdir = appdir.clone();
            strip_appdir(vars, &appdir)
        }
        _ => vars,
    }
}

/// Drops `:`-separated entries under `appdir` from every variable, and
/// variables left with no entries.
fn strip_appdir(vars: Vars, appdir: &str) -> Vars {
    let root = appdir.trim_end_matches('/');
    let prefix = format!("{root}/");
    let inside = |entry: &str| entry == root || entry.starts_with(&prefix);
    vars.into_iter()
        .filter_map(|(key, value)| {
            if !value.split(':').any(inside) {
                return Some((key, value));
            }
            let kept: Vec<&str> = value
                .split(':')
                .filter(|e| !e.is_empty() && !inside(e))
                .collect();
            (!kept.is_empty()).then(|| (key, kept.join(":")))
        })
        .collect()
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

#[derive(Debug, thiserror::Error)]
pub enum EnvEditError {
    #[error(
        "invalid environment name `{0}` (use lowercase letters, digits, `.`, `_` and `-`, starting with a letter or digit)"
    )]
    BadEnvName(String),
    #[error("invalid variable name `{0}` (use letters, digits and `_`, not starting with a digit)")]
    BadVarName(String),
    #[error("variable `{0}` is listed twice")]
    Duplicate(String),
    #[error("environment `{0}` already exists")]
    Exists(String),
    #[error("environment `{0}` not found")]
    Missing(String),
    #[error("invalid sankh.toml: {0}")]
    Config(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// `[a-z0-9][a-z0-9._-]*`, the form `slug()` produces for imported environments.
pub fn valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
}

/// A shell variable name: `[A-Za-z_][A-Za-z0-9_]*`.
pub fn valid_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn env_path(collection: &Collection, name: &str) -> PathBuf {
    collection.root.join(ENV_DIR).join(format!("{name}.env"))
}

/// Path of an existing environment file; never outside `environments/`.
pub fn existing_env(collection: &Collection, name: &str) -> Result<PathBuf, EnvEditError> {
    let path = env_path(collection, name);
    if valid_env_name(name) && path.is_file() {
        Ok(path)
    } else {
        Err(EnvEditError::Missing(name.to_string()))
    }
}

fn check_env_name(name: &str) -> Result<(), EnvEditError> {
    if valid_env_name(name) {
        Ok(())
    } else {
        Err(EnvEditError::BadEnvName(name.to_string()))
    }
}

/// One line (or a multi-line quoted value) of a dotenv file.
enum EnvLine {
    Other(String),
    Var {
        key: String,
        value: String,
        text: String,
    },
}

/// Splits dotenv text into lines, keeping comments and each value as written:
/// `${VAR}` stays a reference and a literal dollar sign reads as `\$`.
fn parse_lines(text: &str) -> Vec<EnvLine> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let body = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        let Some((key, rest)) = body.split_once('=') else {
            out.push(EnvLine::Other(line.to_string()));
            continue;
        };
        let key = key.trim();
        if trimmed.starts_with('#') || !valid_var_name(key) {
            out.push(EnvLine::Other(line.to_string()));
            continue;
        }
        let rest = rest.trim_start();
        let mut text = line.to_string();
        let value = match rest.chars().next() {
            Some(q @ ('"' | '\'')) => {
                let mut raw = rest[1..].to_string();
                let mut value = quoted_value(&raw, q);
                while value.is_none() {
                    let Some(next) = lines.next() else { break };
                    text.push('\n');
                    text.push_str(next);
                    raw.push('\n');
                    raw.push_str(next);
                    value = quoted_value(&raw, q);
                }
                value.unwrap_or(raw)
            }
            _ => {
                let end = rest.find(" #").unwrap_or(rest.len());
                rest[..end].trim_end().to_string()
            }
        };
        out.push(EnvLine::Var {
            key: key.to_string(),
            value,
            text,
        });
    }
    out
}

/// The value inside quotes, or `None` while the closing quote is missing.
fn quoted_value(raw: &str, quote: char) -> Option<String> {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        match c {
            c if c == quote => return Some(out),
            '$' if quote == '\'' => out.push_str("\\$"),
            '\\' if quote == '"' => match chars.next() {
                Some('n') => out.push('\n'),
                Some('$') => out.push_str("\\$"),
                Some(other) => out.push(other),
                None => out.push('\\'),
            },
            c => out.push(c),
        }
    }
    None
}

/// Writes a value so it reads back as written: `${VAR}` expands, `\$` is literal.
pub fn quote_env_value(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "_./:@%+,=-".contains(c))
    {
        return value.to_string();
    }
    let mut out = String::from("\"");
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'$') => {
                chars.next();
                out.push_str("\\$");
            }
            '\\' | '"' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Variables of one dotenv file in file order, values as written (see
/// [`quote_env_value`]). A missing file has no variables.
pub fn read_env_raw(path: &Path) -> Result<Vec<(String, String)>, EnvEditError> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out: Vec<(String, String)> = Vec::new();
    for line in parse_lines(&text) {
        if let EnvLine::Var { key, value, .. } = line {
            match out.iter_mut().find(|(k, _)| *k == key) {
                Some(e) => e.1 = value,
                None => out.push((key, value)),
            }
        }
    }
    Ok(out)
}

/// Replaces the variables of a dotenv file, keeping comments, blank lines and
/// the position of unchanged keys. New keys are appended in the given order.
pub fn write_env_file(path: &Path, vars: &[(String, String)]) -> Result<(), EnvEditError> {
    for (i, (k, _)) in vars.iter().enumerate() {
        if !valid_var_name(k) {
            return Err(EnvEditError::BadVarName(k.clone()));
        }
        if vars[..i].iter().any(|(other, _)| other == k) {
            return Err(EnvEditError::Duplicate(k.clone()));
        }
    }
    let existing = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let mut out: Vec<String> = Vec::new();
    let mut written: Vec<&str> = Vec::new();
    for line in parse_lines(&existing) {
        match line {
            EnvLine::Other(text) => out.push(text),
            EnvLine::Var { key, value, text } => {
                if written.contains(&key.as_str()) {
                    continue;
                }
                let Some((k, new)) = vars.iter().find(|(k, _)| *k == key) else {
                    continue;
                };
                out.push(if *new == value {
                    text
                } else {
                    format!("{k}={}", quote_env_value(new))
                });
                written.push(k);
            }
        }
    }
    for (k, v) in vars {
        if !written.contains(&k.as_str()) {
            out.push(format!("{k}={}", quote_env_value(v)));
        }
    }
    let mut text = out.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)?;
    Ok(())
}

/// Creates `environments/<name>.env`, empty or as a copy of `copy_from`.
pub fn create_env(
    collection: &Collection,
    name: &str,
    copy_from: Option<&str>,
) -> Result<(), EnvEditError> {
    check_env_name(name)?;
    let path = env_path(collection, name);
    if path.exists() {
        return Err(EnvEditError::Exists(name.to_string()));
    }
    let text = match copy_from {
        Some(src) => std::fs::read_to_string(existing_env(collection, src)?)?,
        None => String::new(),
    };
    std::fs::create_dir_all(collection.root.join(ENV_DIR))?;
    std::fs::write(path, text)?;
    Ok(())
}

/// Renames an environment, following it in `default_env`.
pub fn rename_env(collection: &Collection, from: &str, to: &str) -> Result<(), EnvEditError> {
    check_env_name(to)?;
    let src = existing_env(collection, from)?;
    let dest = env_path(collection, to);
    if dest.exists() {
        return Err(EnvEditError::Exists(to.to_string()));
    }
    std::fs::rename(src, dest)?;
    if collection.config.default_env.as_deref() == Some(from) {
        set_default_env(collection, Some(to))?;
    }
    Ok(())
}

/// Deletes an environment, clearing `default_env` if it named it.
pub fn delete_env(collection: &Collection, name: &str) -> Result<(), EnvEditError> {
    std::fs::remove_file(existing_env(collection, name)?)?;
    if collection.config.default_env.as_deref() == Some(name) {
        set_default_env(collection, None)?;
    }
    Ok(())
}

/// Sets or removes the top-level `default_env` key of `sankh.toml`, leaving the
/// rest of the file as written.
pub fn set_default_env(collection: &Collection, name: Option<&str>) -> Result<(), EnvEditError> {
    if let Some(n) = name {
        existing_env(collection, n)?;
    }
    let path = collection.root.join(crate::collection::CONFIG_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    let new_line = name.map(|n| format!("default_env = {}", toml::Value::String(n.to_string())));
    let mut lines: Vec<String> = Vec::new();
    let mut in_table = false;
    let mut replaced = false;
    let mut insert_at = None;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with('[') && !in_table {
            in_table = true;
            insert_at = Some(lines.len());
        }
        let is_key = !in_table
            && t.strip_prefix("default_env")
                .is_some_and(|r| r.trim_start().starts_with('='));
        if is_key {
            if let (Some(l), false) = (&new_line, replaced) {
                lines.push(l.clone());
            }
            replaced = true;
            continue;
        }
        lines.push(line.to_string());
    }
    if let (Some(l), false) = (new_line, replaced) {
        let at = insert_at.unwrap_or(lines.len());
        let at = if at > 0 && lines[at - 1].trim().is_empty() {
            at - 1
        } else {
            at
        };
        lines.insert(at, l);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    toml::from_str::<crate::collection::Config>(&out)
        .map_err(|e| EnvEditError::Config(e.to_string()))?;
    std::fs::write(path, out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_keep_comments_and_references() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dev.env");
        std::fs::write(
            &path,
            "# header\nBASE_URL=http://x\nAPI=\"${BASE_URL}/v1\" # api\nLIT='a$b'\n\nOLD=1\n",
        )
        .unwrap();
        let vars = read_env_raw(&path).unwrap();
        assert_eq!(
            vars,
            vec![
                ("BASE_URL".to_string(), "http://x".to_string()),
                ("API".to_string(), "${BASE_URL}/v1".to_string()),
                ("LIT".to_string(), "a\\$b".to_string()),
                ("OLD".to_string(), "1".to_string()),
            ]
        );
        let new = vec![
            ("BASE_URL".to_string(), "http://y".to_string()),
            ("API".to_string(), "${BASE_URL}/v1".to_string()),
            ("LIT".to_string(), "a\\$b".to_string()),
            ("NEW".to_string(), "two words \"q\"".to_string()),
        ];
        write_env_file(&path, &new).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "# header\nBASE_URL=http://y\nAPI=\"${BASE_URL}/v1\" # api\nLIT='a$b'\n\nNEW=\"two words \\\"q\\\"\"\n"
        );
        assert_eq!(read_env_raw(&path).unwrap(), new);
        let loaded = read_env_file(&path).unwrap();
        assert_eq!(loaded["API"], "http://y/v1");
        assert_eq!(loaded["LIT"], "a$b");
        assert_eq!(loaded["NEW"], "two words \"q\"");
        assert!(matches!(
            write_env_file(&path, &[("1X".into(), String::new())]),
            Err(EnvEditError::BadVarName(_))
        ));
    }

    #[test]
    fn create_rename_delete_follow_default() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("sankh.toml"),
            "name = \"X\"\ndefault_env = \"dev\"\n\n[order]\n\"\" = [\"a\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("environments")).unwrap();
        std::fs::write(root.join("environments/dev.env"), "A=1\n").unwrap();
        let c = Collection::open(root).unwrap();
        create_env(&c, "staging", Some("dev")).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("environments/staging.env")).unwrap(),
            "A=1\n"
        );
        assert!(matches!(
            create_env(&c, "Bad Name", None),
            Err(EnvEditError::BadEnvName(_))
        ));
        rename_env(&c, "dev", "local").unwrap();
        let c = Collection::open(root).unwrap();
        assert_eq!(c.config.default_env.as_deref(), Some("local"));
        assert!(c.config.order.contains_key(""));
        delete_env(&c, "local").unwrap();
        let c = Collection::open(root).unwrap();
        assert_eq!(c.config.default_env, None);
        set_default_env(&c, Some("staging")).unwrap();
        let text = std::fs::read_to_string(root.join("sankh.toml")).unwrap();
        assert_eq!(
            text,
            "name = \"X\"\ndefault_env = \"staging\"\n\n[order]\n\"\" = [\"a\"]\n"
        );
    }

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

    #[test]
    fn strips_appimage_paths() {
        let appdir = "/tmp/.mount_SankhAb12";
        let vars = Vars::from([
            (
                "LD_LIBRARY_PATH".to_string(),
                format!("{appdir}/usr/lib/:{appdir}/lib64/:/opt/lib"),
            ),
            (
                "XDG_DATA_DIRS".to_string(),
                format!("{appdir}/usr/share:/usr/share:/usr/local/share"),
            ),
            (
                "GDK_PIXBUF_MODULE_FILE".to_string(),
                format!("{appdir}//usr/lib/gdk-pixbuf-2.0/loaders.cache"),
            ),
            ("APPDIR".to_string(), appdir.to_string()),
            (
                "APPIMAGE".to_string(),
                "/home/me/Sankh.AppImage".to_string(),
            ),
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            (
                "OTHER".to_string(),
                "/tmp/.mount_SankhAb12x/keep".to_string(),
            ),
        ]);
        let out = strip_appdir(vars, appdir);
        assert_eq!(out["LD_LIBRARY_PATH"], "/opt/lib");
        assert_eq!(out["XDG_DATA_DIRS"], "/usr/share:/usr/local/share");
        assert!(!out.contains_key("GDK_PIXBUF_MODULE_FILE"));
        assert!(!out.contains_key("APPDIR"));
        assert_eq!(out["APPIMAGE"], "/home/me/Sankh.AppImage");
        assert_eq!(out["PATH"], "/usr/bin:/bin");
        assert_eq!(out["OTHER"], "/tmp/.mount_SankhAb12x/keep");
    }
}
