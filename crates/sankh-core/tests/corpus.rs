//! Real-world curl commands: what Sankh would send for each, or the
//! diagnostics it reports instead. Every example request must parse cleanly.

use sankh_core::diag;
use sankh_core::parse::expand::{Lookup, Vars};
use sankh_core::parse::spec::{self, BuildCtx};
use std::fmt::Write as _;
use std::path::Path;

/// Every variable is defined as `<NAME>`.
struct Placeholders;

impl Vars for Placeholders {
    fn lookup(&self, name: &str) -> Lookup {
        Lookup::Value(match name {
            "BASE" => "http://localhost:3000".into(),
            _ => format!("<{name}>"),
        })
    }
}

fn add(diags: &mut Vec<sankh_core::request::Diagnostic>, new: &[diag::Diag], content: &str) {
    for d in new {
        let d = d.locate(content);
        if !diags.contains(&d) {
            diags.push(d);
        }
    }
}

fn describe(content: &str) -> String {
    let mut out = String::new();
    let req = sankh_core::parser::parse(content, "t.sh");
    let mut diags = req.diagnostics.clone();
    if let Some(cmd) = req.command.as_ref().filter(|_| !req.has_errors()) {
        let ctx = BuildCtx {
            vars: &Placeholders,
            root: None,
        };
        match spec::build(cmd, &ctx) {
            Ok(built) => {
                let s = built.spec;
                writeln!(out, "{} {}", s.method, s.url).unwrap();
                for (k, v) in &s.headers {
                    writeln!(out, "{k}: {v}").unwrap();
                }
                if let Some(body) = &s.body {
                    writeln!(out, "\n{}", String::from_utf8_lossy(body)).unwrap();
                }
                add(&mut diags, &built.warnings, content);
            }
            Err(errors) => add(&mut diags, &errors, content),
        }
    }
    for d in &diags {
        out.push('\n');
        out.push_str(&diag::render(d, "t.sh", content, false));
    }
    out
}

#[test]
fn corpus() {
    let text = include_str!("corpus/commands.txt");
    let mut out = String::new();
    for block in text.split("\n\n") {
        let cmd: String = block
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| format!("{l}\n"))
            .collect();
        if cmd.trim().is_empty() {
            continue;
        }
        writeln!(
            out,
            "$ {}\n{}\n{}",
            cmd.trim_end(),
            describe(&cmd).trim_end(),
            "-".repeat(60)
        )
        .unwrap();
    }
    let boundary = regex::Regex::new("-{24}[A-Za-z0-9]{22}").unwrap();
    insta::assert_snapshot!(boundary.replace_all(&out, "<boundary>"));
}

fn request_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            request_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "sh") {
            out.push(path);
        }
    }
}

#[test]
fn examples_are_clean() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut files = Vec::new();
    request_files(&root, &mut files);
    assert!(!files.is_empty());
    for path in files {
        let content = std::fs::read_to_string(&path).unwrap();
        let req = sankh_core::parser::parse(&content, "t.sh");
        assert!(
            req.diagnostics.is_empty(),
            "{}: {:?}",
            path.display(),
            req.diagnostics
        );
    }
}
