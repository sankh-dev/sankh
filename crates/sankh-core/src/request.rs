//! Parsed request model.

pub use crate::diag::ErrorKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// 1-based line number in the file; 0 when it applies to the whole file.
    pub line: usize,
    pub severity: Severity,
    pub message: String,
    /// Error class (errors only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ErrorKind>,
    /// 1-based character column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// Characters to underline from `column`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<usize>,
    /// Text shown next to the underline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Diagnostic {
    /// A whole-line diagnostic (annotation problems).
    pub fn line(line: usize, severity: Severity, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            line,
            severity,
            message: message.into(),
            kind: (severity == Severity::Error).then_some(ErrorKind::Parse),
            column: None,
            width: None,
            label: None,
            help: None,
            note: None,
        }
    }

    /// One-line summary: `line 5:3: message (label)`.
    pub fn summary(&self) -> String {
        let mut out = match (self.line, self.column) {
            (0, _) => String::new(),
            (l, Some(c)) => format!("line {l}:{c}: "),
            (l, None) => format!("line {l}: "),
        };
        out.push_str(&self.message);
        if let Some(label) = &self.label {
            out.push_str(&format!(": {label}"));
        }
        if let Some(help) = &self.help {
            out.push_str(&format!(" (help: {})", help.replace('\n', " ")));
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum StatusPattern {
    Exact { code: u16 },
    Class { class: u8 },
}

impl StatusPattern {
    pub fn matches(&self, status: u16) -> bool {
        match self {
            StatusPattern::Exact { code } => *code == status,
            StatusPattern::Class { class } => status / 100 == u16::from(*class),
        }
    }
}

impl std::fmt::Display for StatusPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusPattern::Exact { code } => write!(f, "{code}"),
            StatusPattern::Class { class } => write!(f, "{class}xx"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Exists,
    Matches,
    Contains,
}

impl JsonOp {
    pub fn symbol(&self) -> &'static str {
        match self {
            JsonOp::Eq => "==",
            JsonOp::Ne => "!=",
            JsonOp::Gt => ">",
            JsonOp::Ge => ">=",
            JsonOp::Lt => "<",
            JsonOp::Le => "<=",
            JsonOp::Exists => "exists",
            JsonOp::Matches => "matches",
            JsonOp::Contains => "contains",
        }
    }

    pub fn from_symbol(s: &str) -> Option<JsonOp> {
        Some(match s {
            "==" => JsonOp::Eq,
            "!=" => JsonOp::Ne,
            ">" => JsonOp::Gt,
            ">=" => JsonOp::Ge,
            "<" => JsonOp::Lt,
            "<=" => JsonOp::Le,
            "exists" => JsonOp::Exists,
            "matches" => JsonOp::Matches,
            "contains" => JsonOp::Contains,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Expect {
    Status {
        patterns: Vec<StatusPattern>,
    },
    Json {
        expr: String,
        op: JsonOp,
        /// JSON literal source text; may embed `$VAR` inside strings.
        value: Option<String>,
    },
}

impl std::fmt::Display for Expect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expect::Status { patterns } => {
                let p: Vec<String> = patterns.iter().map(|p| p.to_string()).collect();
                write!(f, "status {}", p.join("|"))
            }
            Expect::Json { expr, op, value } => match value {
                Some(v) => write!(f, "json {expr} {} {v}", op.symbol()),
                None => write!(f, "json {expr} {}", op.symbol()),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CaptureSource {
    Jq { expr: String },
    Header { name: String },
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capture {
    pub var: String,
    pub source: CaptureSource,
}

impl std::fmt::Display for Capture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.source {
            CaptureSource::Jq { expr } => write!(f, "{}={expr}", self.var),
            CaptureSource::Header { name } => write!(f, "{}=header {name}", self.var),
            CaptureSource::Status => write!(f, "{}=status", self.var),
        }
    }
}

/// A curl invocation broken into the parts the form view edits.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurlCommand {
    pub method: String,
    pub url: String,
    pub headers: Vec<Header>,
    pub body: Option<String>,
    /// Body flag when it is not plain `-d` (e.g. `--data-raw`, `--json`).
    #[serde(default)]
    pub body_flag: Option<String>,
    /// Remaining flags, kept verbatim (e.g. `-sS`, `-L`, `-u user:pass`).
    pub flags: Vec<String>,
}

impl CurlCommand {
    /// The method curl uses when there is no `-X`.
    pub fn implied_method(&self) -> &'static str {
        let has = |names: &[&str]| self.flags.iter().any(|f| names.contains(&f.as_str()));
        if has(&["-I", "--head"]) {
            "HEAD"
        } else if has(&["-G", "--get"]) {
            "GET"
        } else if self.body.is_some()
            || has(&[
                "-d",
                "--data",
                "--data-ascii",
                "--data-raw",
                "--data-binary",
                "--data-urlencode",
                "--json",
                "-F",
                "--form",
                "--form-string",
            ])
        {
            "POST"
        } else {
            "GET"
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Header annotations plus one curl command the form view can edit.
    Form,
    /// The command could not be read (shell syntax, quoting); edited as plain
    /// text and not runnable until fixed.
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub name: String,
    /// Joined with `\n` when there are several `@description` lines.
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// Per-request limit from `@timeout`, overriding the collection default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    pub expects: Vec<Expect>,
    pub captures: Vec<Capture>,
    pub mode: Mode,
    /// Why the file is in raw mode, if it is.
    pub raw_reason: Option<String>,
    pub curl: Option<CurlCommand>,
    /// Header lines that are not recognised annotations, preserved verbatim.
    pub extra_header_lines: Vec<String>,
    pub shebang: Option<String>,
    /// Everything after the header block.
    pub body: String,
    pub diagnostics: Vec<Diagnostic>,
    /// The parsed command, for building the request at run time.
    #[serde(skip)]
    pub command: Option<crate::parse::flags::Parsed>,
}

impl Request {
    pub fn timeout(&self) -> Option<std::time::Duration> {
        self.timeout_ms.map(std::time::Duration::from_millis)
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Effective status expectations: explicit ones, or `2xx` by default.
    pub fn status_expectations(&self) -> Vec<Vec<StatusPattern>> {
        let explicit: Vec<Vec<StatusPattern>> = self
            .expects
            .iter()
            .filter_map(|e| match e {
                Expect::Status { patterns } => Some(patterns.clone()),
                _ => None,
            })
            .collect();
        if explicit.is_empty() {
            vec![vec![StatusPattern::Class { class: 2 }]]
        } else {
            explicit
        }
    }
}
