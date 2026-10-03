//! Renders an [`ImportedCollection`] into collection files.

use super::*;
use crate::format::{self, RequestForm};
use crate::request::Mode;
use crate::{parser, shell};
use std::path::Path;

/// Files to write (relative `/`-separated path, content) plus the report.
#[derive(Debug, Clone, Default)]
pub struct ImportOutput {
    pub files: Vec<(String, String)>,
    pub report: ImportReport,
}

pub fn render(c: &ImportedCollection) -> ImportOutput {
    let mut out = ImportOutput::default();
    out.report.collection = c.name.clone();
    for w in &c.root.warnings {
        warn(&mut out.report, "", w);
    }
    folder(c, &c.root, "", &mut out);

    let mut env_names: Vec<String> = Vec::new();
    for env in &c.environments {
        let base = slug(&env.name);
        let mut name = base.clone();
        let mut n = 2;
        while env_names.contains(&name) {
            name = format!("{base}-{n}");
            n += 1;
        }
        let mut text = format!("# Imported from environment \"{}\"\n", one_line(&env.name));
        for (k, v) in &env.vars {
            text.push_str(&format!("{k}={v}\n"));
        }
        out.files
            .push((format!("{}/{name}.env", crate::collection::ENV_DIR), text));
        env_names.push(name);
    }

    let mut toml = format!(
        "name = {}\nformat = 1\n",
        toml::Value::String(c.name.clone())
    );
    if let Some(first) = env_names.first() {
        toml.push_str(&format!(
            "default_env = {}\n",
            toml::Value::String(first.clone())
        ));
    }
    out.files.push(("sankh.toml".into(), toml));
    out.files
        .push((".gitignore".into(), ".env.local\nsankh-report.*\n".into()));

    let mut placeholders = c.placeholders.clone();
    for r in c.vars.renames() {
        if c.vars.dynamic.contains(&r.to) && !placeholders.iter().any(|p| p.name == r.to) {
            placeholders.push(Placeholder {
                name: r.to.clone(),
                note: format!(
                    "replaces the dynamic variable {{{{{}}}}}; set a value before running",
                    r.from
                ),
            });
        }
    }
    let mut example = String::from(
        "# Copy to .env.local (gitignored) and fill in. Secrets never go in environments/*.env.\n",
    );
    for p in &placeholders {
        example.push_str(&format!("\n# {}\n{}=\n", p.note, p.name));
    }
    out.files.push((".env.example".into(), example));

    out.report.environments = env_names;
    out.report.renamed = c.vars.renames();
    out.report.placeholders = placeholders;
    out
}

/// Writes rendered files under `dir`, refusing a non-empty `dir` unless `force`.
pub fn write(output: &ImportOutput, dir: &Path, force: bool) -> Result<(), ImportError> {
    if !force && dir.is_dir() && std::fs::read_dir(dir)?.next().is_some() {
        return Err(ImportError::NotEmpty(dir.to_path_buf()));
    }
    std::fs::create_dir_all(dir)?;
    for (rel, content) in &output.files {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

fn folder(c: &ImportedCollection, f: &ImportedFolder, prefix: &str, out: &mut ImportOutput) {
    let width = f.items.len().to_string().len().max(2);
    for (i, item) in f.items.iter().enumerate() {
        let n = format!("{:0width$}", i + 1);
        match item {
            ImportedItem::Folder(sub) => {
                let path = format!("{prefix}{n}-{}", slug(&sub.name));
                for w in &sub.warnings {
                    warn(&mut out.report, &format!("{path}/"), w);
                }
                if count_requests(sub) == 0 {
                    warn(
                        &mut out.report,
                        &format!("{path}/"),
                        &format!("folder `{}` has no requests and was skipped", sub.name),
                    );
                    continue;
                }
                out.report.folders += 1;
                folder(c, sub, &format!("{path}/"), out);
            }
            ImportedItem::Request(req) => {
                let file_name = format!("{n}-{}.sh", slug(&req.name));
                let path = format!("{prefix}{file_name}");
                let text = request(c, req, &file_name, &path, &mut out.report);
                out.report.requests += 1;
                out.files.push((path, text));
            }
        }
    }
}

fn request(
    c: &ImportedCollection,
    req: &ImportedRequest,
    file_name: &str,
    path: &str,
    report: &mut ImportReport,
) -> String {
    for w in &req.warnings {
        warn(report, path, w);
    }
    let mut extra: Vec<String> = Vec::new();
    let dynamic: Vec<String> = shell::referenced_vars(&format::render_curl(&req.curl))
        .into_iter()
        .filter(|v| c.vars.dynamic.contains(v))
        .collect();
    if !dynamic.is_empty() {
        extra.push(format!("# @require {}", dynamic.join(" ")));
        warn(
            report,
            path,
            &format!(
                "uses dynamic variables with no Sankh equivalent ({}); set them before running",
                dynamic.join(", ")
            ),
        );
    }
    extra.extend(req.comments.iter().cloned());

    let mut curl = req.curl.clone();
    let (line, body) = format::split_flags(&curl.flags);
    curl.flags = [line, body].concat();
    let form = RequestForm {
        shebang: None,
        name: req.name.clone(),
        description: req.description.clone(),
        tags: Vec::new(),
        expects: req.expects.clone(),
        captures: req.captures.clone(),
        extra_header_lines: extra,
        curl,
    };
    let text = format::render(&form);

    let parsed = parser::parse(&text, file_name);
    if parsed.mode == Mode::Raw {
        warn(
            report,
            path,
            &format!(
                "opens in raw mode: {}",
                parsed.raw_reason.unwrap_or_default()
            ),
        );
    } else if parsed.has_errors() {
        for d in parsed
            .diagnostics
            .iter()
            .filter(|d| d.severity == crate::request::Severity::Error)
        {
            warn(report, path, &format!("line {}: {}", d.line, d.message));
        }
    } else if parsed.curl.as_ref() != Some(&form.curl) {
        warn(
            report,
            path,
            "the curl command does not read back identically; review the file",
        );
    }
    text
}

fn count_requests(f: &ImportedFolder) -> usize {
    f.items
        .iter()
        .map(|i| match i {
            ImportedItem::Folder(sub) => count_requests(sub),
            ImportedItem::Request(_) => 1,
        })
        .sum()
}

fn warn(report: &mut ImportReport, path: &str, message: &str) {
    report.warnings.push(Warning {
        path: path.to_string(),
        message: message.to_string(),
    });
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
