//! Runs request files with the built-in HTTP engine.
//!
//! A file is parsed, its variables are expanded from the environment, and
//! the resulting request is sent by [`crate::http::Engine`]. Nothing in a
//! request file is executed.

use crate::assert::{self, AssertionResult};
use crate::capture;
use crate::collection::Collection;
use crate::diag::{Diag, ErrorKind};
use crate::env::Env;
use crate::http::{self, Engine, HostPolicy, RedirectHop, SendError};
use crate::parse::expand::{Lookup, Vars};
use crate::parse::spec::{self, BuildCtx, RequestSpec};
use crate::parser;
use crate::redact::{self, Redactor};
use crate::request::{Diagnostic, Expect, Request, Severity};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub use crate::http::Response;

const DEFAULT_TIMEOUT_SECS: u64 = 30;
/// Bodies larger than this are truncated in results (assertions see the whole body).
const MAX_BODY_DISPLAY: usize = 2 * 1024 * 1024;
const INSECURE_WARNING: &str =
    "TLS certificate verification is disabled (-k); responses could come from anyone";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Passed,
    Failed,
    Error,
    /// Stopped by the user before it finished.
    Cancelled,
}

/// What was sent, for display.
#[derive(Debug, Clone, Serialize)]
pub struct RequestView {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub body_size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseView {
    pub status: u16,
    /// `HTTP/1.1`, `HTTP/2.0`.
    pub version: String,
    pub time_ms: f64,
    /// Until the response headers arrived.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttfb_ms: Option<f64>,
    pub size: usize,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// Only part of the body is shown: it was cut for display, or it was
    /// longer than `max_body` (see `body_capped`).
    pub body_truncated: bool,
    /// The body was longer than `max_body` and was not read in full.
    pub body_capped: bool,
    pub body_binary: bool,
    /// SHA-256 of a binary body, hex.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// The raw body of an `image/*` response up to `MAX_BODY_DISPLAY`, for previews.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_base64: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub redirects: Vec<RedirectHop>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CapturedValue {
    pub name: String,
    /// Redacted for display.
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RequestResult {
    pub path: String,
    pub name: String,
    pub outcome: Outcome,
    /// Why an `error` outcome happened.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<ErrorKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<RequestView>,
    pub response: Option<ResponseView>,
    pub assertions: Vec<AssertionResult>,
    pub captures: Vec<CapturedValue>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    /// Positioned errors and warnings in the file.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    /// The request skipped TLS certificate checks (`-k`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub insecure: bool,
    pub duration_ms: f64,
}

impl RequestResult {
    pub fn new(path: String, name: String) -> RequestResult {
        RequestResult {
            path,
            name,
            outcome: Outcome::Passed,
            error_kind: None,
            request: None,
            response: None,
            assertions: Vec::new(),
            captures: Vec::new(),
            error: None,
            warnings: Vec::new(),
            diagnostics: Vec::new(),
            insecure: false,
            duration_ms: 0.0,
        }
    }

    fn fail(&mut self, kind: ErrorKind, message: String) {
        self.outcome = Outcome::Error;
        self.error_kind = Some(kind);
        self.error = Some(message);
    }

    /// The exit-code class of this result, if it did not pass.
    pub fn failure_kind(&self) -> Option<ErrorKind> {
        match self.outcome {
            Outcome::Passed => None,
            Outcome::Failed => Some(ErrorKind::Assertion),
            Outcome::Error => Some(self.error_kind.unwrap_or(ErrorKind::Transport)),
            Outcome::Cancelled => Some(ErrorKind::Transport),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub timeout: Duration,
    /// Set from another thread to stop the request in flight.
    pub cancel: Arc<AtomicBool>,
    /// Leaves secrets unmasked in the result, for the local web UI only;
    /// CLI and MCP output must stay masked.
    pub reveal_secrets: bool,
    pub max_body: usize,
    pub env_proxy: bool,
    /// Hosts requests may reach (`sankh serve --allow-target/--deny-target`).
    pub policy: Option<Arc<HostPolicy>>,
}

impl RunOptions {
    pub fn for_collection(c: &Collection) -> RunOptions {
        RunOptions {
            timeout: Duration::from_secs(c.config.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS)),
            cancel: Arc::default(),
            reveal_secrets: false,
            max_body: c
                .config
                .max_body
                .as_ref()
                .and_then(|s| s.bytes().ok())
                .map_or(http::DEFAULT_MAX_BODY, |b| b as usize),
            env_proxy: c.config.env_proxy.unwrap_or(true),
            policy: None,
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// State carried across the requests of one run: environment (with
/// captures), the redactor (which learns captured secrets) and the HTTP
/// engine (connections and cookies).
pub struct RunContext {
    pub env: Env,
    pub redactor: Redactor,
    pub options: RunOptions,
    engine: Engine,
    warned_insecure: bool,
}

impl RunContext {
    pub fn new(env: Env, options: RunOptions) -> RunContext {
        let redactor = Redactor::from_vars(&env.resolved());
        let mut engine = Engine::default();
        engine.env_proxy = options.env_proxy;
        engine.max_body = options.max_body;
        engine.policy = options.policy.clone();
        RunContext {
            env,
            redactor,
            options,
            engine,
            warned_insecure: false,
        }
    }
}

/// Variables from the environment layers. Names in `pending` are captured by
/// an earlier request and only known once the run gets there.
pub struct EnvVars<'a> {
    pub env: &'a Env,
    pub pending: &'a BTreeSet<String>,
}

impl Vars for EnvVars<'_> {
    fn lookup(&self, name: &str) -> Lookup {
        match self.env.get(name) {
            Some(v) => Lookup::Value(v.clone()),
            None if self.pending.contains(name) => Lookup::Deferred,
            None => Lookup::Missing,
        }
    }
}

/// Problems found in one file before anything is sent.
#[derive(Debug, Clone, Serialize)]
pub struct FileCheck {
    /// Relative to the collection root.
    pub path: String,
    #[serde(skip)]
    pub content: String,
    pub diagnostics: Vec<Diagnostic>,
}

impl FileCheck {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

/// Checks every selected file without sending anything: syntax, flags,
/// variables (those captured by an earlier file in `files` are accepted),
/// and files referenced with `@`.
pub fn check(collection: &Collection, files: &[PathBuf], env: &Env) -> Vec<FileCheck> {
    let mut captured = BTreeSet::new();
    let mut out = Vec::new();
    for file in files {
        let path = collection.rel(file);
        let content = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                out.push(FileCheck {
                    path,
                    content: String::new(),
                    diagnostics: vec![Diagnostic {
                        kind: Some(ErrorKind::Config),
                        ..Diagnostic::line(0, Severity::Error, format!("cannot read file: {e}"))
                    }],
                });
                continue;
            }
        };
        let req = collection.parse_file(file, &content);
        let mut diagnostics = req.diagnostics.clone();
        if !req.has_errors() {
            if let Some(parsed) = &req.command {
                let vars = EnvVars {
                    env,
                    pending: &captured,
                };
                let ctx = BuildCtx {
                    vars: &vars,
                    root: Some(&collection.root),
                };
                if let Err(errors) = spec::build(parsed, &ctx) {
                    add_diags(&mut diagnostics, &errors, &content);
                }
            }
        }
        captured.extend(req.captures.iter().map(|c| c.var.clone()));
        out.push(FileCheck {
            path,
            content,
            diagnostics,
        });
    }
    out
}

/// Adds diagnostics from building with real values, skipping those the
/// static parse already reported.
fn add_diags(diags: &mut Vec<Diagnostic>, new: &[Diag], content: &str) {
    for d in new {
        let d = d.locate(content);
        if !diags.contains(&d) {
            diags.push(d);
        }
    }
}

/// Results for the files [`check`] rejected, or `None` when every file can
/// run. A run with any rejected file sends nothing.
pub fn preflight(
    collection: &Collection,
    files: &[PathBuf],
    env: &Env,
) -> Option<Vec<RequestResult>> {
    let checks = check(collection, files, env);
    if !checks.iter().any(FileCheck::has_errors) {
        return None;
    }
    let redactor = Redactor::from_vars(&env.resolved());
    Some(
        checks
            .into_iter()
            .filter(FileCheck::has_errors)
            .map(|c| {
                let file_name = c.path.rsplit('/').next().unwrap_or_default();
                let name = parser::parse(&c.content, file_name).name;
                let mut r = RequestResult::new(c.path, name);
                apply_diagnostics(&mut r, c.diagnostics, &redactor);
                r
            })
            .collect(),
    )
}

/// Puts diagnostics on a result; errors make it an error result.
fn apply_diagnostics(result: &mut RequestResult, diags: Vec<Diagnostic>, r: &Redactor) {
    let mut errors = Vec::new();
    let mut kind = None;
    for d in &diags {
        let msg = r.redact(&d.summary());
        if d.severity == Severity::Error {
            kind = kind.max(d.kind);
            errors.push(msg);
        } else {
            result.warnings.push(msg);
        }
    }
    if !errors.is_empty() {
        result.fail(kind.unwrap_or(ErrorKind::Parse), errors.join("\n"));
    }
    result.diagnostics.extend(diags);
}

pub fn run_request(collection: &Collection, file: &Path, ctx: &mut RunContext) -> RequestResult {
    let started = Instant::now();
    let rel = collection.rel(file);
    let mut result = match std::fs::read_to_string(file) {
        Ok(content) => {
            let req = collection.parse_file(file, &content);
            execute(collection, &req, &content, ctx)
        }
        Err(e) => {
            let mut r = RequestResult::new(rel.clone(), rel.clone());
            r.fail(ErrorKind::Config, format!("cannot read file: {e}"));
            r
        }
    };
    result.path = rel;
    result.duration_ms = started.elapsed().as_secs_f64() * 1000.0;
    result
}

fn execute(
    collection: &Collection,
    req: &Request,
    content: &str,
    ctx: &mut RunContext,
) -> RequestResult {
    let mut result = RequestResult::new(String::new(), req.name.clone());
    let unmasked = Redactor::default();
    let shown = |ctx: &RunContext| -> Redactor {
        if ctx.options.reveal_secrets {
            Redactor::default()
        } else {
            ctx.redactor.clone()
        }
    };

    let mut diags = req.diagnostics.clone();
    let spec = match (&req.command, req.has_errors()) {
        (Some(parsed), false) => {
            let empty = BTreeSet::new();
            let vars = EnvVars {
                env: &ctx.env,
                pending: &empty,
            };
            let build_ctx = BuildCtx {
                vars: &vars,
                root: Some(&collection.root),
            };
            match spec::build(parsed, &build_ctx) {
                Ok(built) => {
                    add_diags(&mut diags, &built.warnings, content);
                    Some(built.spec)
                }
                Err(errors) => {
                    add_diags(&mut diags, &errors, content);
                    None
                }
            }
        }
        _ => None,
    };
    apply_diagnostics(&mut result, diags, &shown(ctx));
    let Some(spec) = spec else {
        if result.error.is_none() {
            result.fail(ErrorKind::Parse, "the request could not be read".into());
        }
        return result;
    };

    if spec.options.insecure {
        result.insecure = true;
        if !ctx.warned_insecure {
            ctx.warned_insecure = true;
            result.warnings.push(INSECURE_WARNING.into());
        }
    }
    let r = shown(ctx);
    result.request = Some(request_view(&spec, &r, ctx.options.reveal_secrets));

    let timeout = spec
        .options
        .timeout
        .or(req.timeout())
        .unwrap_or(ctx.options.timeout);
    let cancel = ctx.options.cancel.clone();
    let response = match ctx.engine.send(&spec, timeout, &cancel) {
        Ok(resp) => resp,
        Err(e) => {
            let (kind, msg) = match e {
                SendError::Transport(m) => (ErrorKind::Transport, m),
                SendError::Timeout(t) => (
                    ErrorKind::Transport,
                    format!(
                        "timed out after {}",
                        parser::format_duration(t.as_millis() as u64)
                    ),
                ),
                SendError::Config(m) => (ErrorKind::Config, m),
                SendError::Cancelled => {
                    result.outcome = Outcome::Cancelled;
                    result.error = Some("cancelled".into());
                    return result;
                }
            };
            result.fail(kind, r.redact(&msg));
            return result;
        }
    };
    result
        .warnings
        .extend(response.notes.iter().map(|n| r.redact(n)));

    let env = &ctx.env;
    let lookup = |name: &str| env.get(name).cloned();
    for patterns in req.status_expectations() {
        result
            .assertions
            .push(assert::check_status(&patterns, response.status));
    }
    for e in &req.expects {
        if let Expect::Json { expr, op, value } = e {
            let mut a = assert::check_json(expr, *op, value.as_deref(), &response.body, &lookup);
            if response.body_truncated {
                a.passed = false;
                a.message = Some(capped_message(ctx.options.max_body));
            }
            result.assertions.push(a);
        }
    }

    let all_passed = result.assertions.iter().all(|a| a.passed);
    if all_passed {
        for c in &req.captures {
            if response.body_truncated
                && matches!(c.source, crate::request::CaptureSource::Jq { .. })
            {
                result.error = Some(format!(
                    "capture {} failed: {}",
                    c.var,
                    capped_message(ctx.options.max_body)
                ));
                continue;
            }
            match capture::extract(&c.source, &response) {
                Ok(value) => {
                    if redact::is_secret_name(&c.var) {
                        ctx.redactor.add(&value);
                    }
                    ctx.env.captures.insert(c.var.clone(), value.clone());
                    result.captures.push(CapturedValue {
                        name: c.var.clone(),
                        value,
                    });
                }
                Err(msg) => {
                    result.error = Some(format!("capture {} failed: {msg}", c.var));
                }
            }
        }
    }

    let r = if ctx.options.reveal_secrets {
        &unmasked
    } else {
        &ctx.redactor
    };
    for a in &mut result.assertions {
        a.message = a.message.as_deref().map(|m| r.redact(m));
    }
    if !ctx.options.reveal_secrets {
        for c in &mut result.captures {
            c.value = r.display_var(&c.name, &c.value);
        }
    }
    result.response = Some(view(&response, r));
    if all_passed && result.error.is_none() {
        result.outcome = Outcome::Passed;
    } else {
        result.outcome = Outcome::Failed;
        result.error_kind = Some(ErrorKind::Assertion);
    }
    result
}

fn capped_message(max_body: usize) -> String {
    format!(
        "the response body is larger than max_body ({}); raise max_body in sankh.toml",
        human_size(max_body)
    )
}

fn human_size(bytes: usize) -> String {
    const MB: usize = 1024 * 1024;
    if bytes >= MB && bytes % MB == 0 {
        format!("{} MB", bytes / MB)
    } else if bytes >= 1024 && bytes % 1024 == 0 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} bytes")
    }
}

/// Header values that carry credentials whatever the variable names are.
fn is_credential_header(name: &str) -> bool {
    ["authorization", "proxy-authorization", "cookie"]
        .iter()
        .any(|h| name.eq_ignore_ascii_case(h))
}

fn request_view(spec: &RequestSpec, r: &Redactor, reveal: bool) -> RequestView {
    let headers = spec
        .headers
        .iter()
        .map(|(k, v)| {
            let value = if !reveal && is_credential_header(k) {
                match v.split_once(' ') {
                    Some((scheme, _)) if !k.eq_ignore_ascii_case("cookie") => {
                        format!("{scheme} {}", redact::MASK)
                    }
                    _ => redact::MASK.to_string(),
                }
            } else {
                r.redact(v)
            };
            (k.clone(), value)
        })
        .collect();
    let body = spec.body.as_ref().map(|b| match std::str::from_utf8(b) {
        Ok(s) => r.redact(&cut(s, MAX_BODY_DISPLAY)),
        Err(_) => format!("<{} bytes of binary data>", b.len()),
    });
    RequestView {
        method: spec.method.clone(),
        url: r.redact(&spec.url),
        headers,
        body_size: spec.body.as_ref().map_or(0, Vec::len),
        body,
    }
}

fn cut(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn view(resp: &Response, r: &Redactor) -> ResponseView {
    let (body, binary, sha256) = match std::str::from_utf8(&resp.body) {
        Ok(s) => (s.to_string(), false, None),
        Err(_) => {
            use sha2::Digest;
            let digest = sha2::Sha256::digest(&resp.body);
            let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
            (
                format!("<{} bytes of binary data>", resp.body.len()),
                true,
                Some(hex),
            )
        }
    };
    let cut_for_display = body.len() > MAX_BODY_DISPLAY;
    ResponseView {
        status: resp.status,
        version: resp.version.clone(),
        time_ms: resp.time_ms,
        ttfb_ms: resp.ttfb_ms,
        size: resp.body.len(),
        url: r.redact(&resp.url),
        headers: resp
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), r.redact(v)))
            .collect(),
        body: r.redact(&cut(&body, MAX_BODY_DISPLAY)),
        body_truncated: cut_for_display || resp.body_truncated,
        body_capped: resp.body_truncated,
        body_binary: binary,
        sha256,
        body_base64: image_base64(resp),
        redirects: resp
            .redirects
            .iter()
            .map(|h| RedirectHop {
                status: h.status,
                url: r.redact(&h.url),
                location: r.redact(&h.location),
            })
            .collect(),
    }
}

fn image_base64(resp: &Response) -> Option<String> {
    use base64::Engine;
    let ct = resp.header("content-type")?.trim_start();
    let is_image = ct.len() >= 6 && ct[..6].eq_ignore_ascii_case("image/");
    (is_image && !resp.body_truncated && resp.body.len() <= MAX_BODY_DISPLAY)
        .then(|| base64::engine::general_purpose::STANDARD.encode(&resp.body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(content_type: &str, body: Vec<u8>) -> Response {
        Response {
            status: 200,
            headers: vec![("Content-Type".to_string(), content_type.to_string())],
            body,
            ..Response::default()
        }
    }

    #[test]
    fn image_body_is_base64() {
        let png = vec![0x89, b'P', b'N', b'G', 0xff];
        let v = view(&response("image/png", png), &Redactor::default());
        assert_eq!(v.body_base64.as_deref(), Some("iVBOR/8="));
        assert!(v.body_binary);
        assert_eq!(v.sha256.as_deref().map(str::len), Some(64));
    }

    #[test]
    fn text_body_has_no_base64() {
        let v = view(
            &response("application/json", b"{}".to_vec()),
            &Redactor::default(),
        );
        assert_eq!(v.body_base64, None);
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("body_base64") && !json.contains("sha256"));
    }

    #[test]
    fn large_image_has_no_base64() {
        let big = vec![0u8; MAX_BODY_DISPLAY + 1];
        let v = view(&response("IMAGE/JPEG", big), &Redactor::default());
        assert_eq!(v.body_base64, None);
    }

    #[test]
    fn echo_masks_credentials() {
        let spec = RequestSpec {
            method: "POST".into(),
            url: "http://x/".into(),
            headers: vec![
                ("Authorization".into(), "Bearer abc.def".into()),
                ("Cookie".into(), "sid=1".into()),
                ("Accept".into(), "*/*".into()),
            ],
            body: Some(b"{\"a\":1}".to_vec()),
            options: Default::default(),
            cookies: Default::default(),
        };
        let v = request_view(&spec, &Redactor::default(), false);
        assert_eq!(v.headers[0].1, "Bearer ***");
        assert_eq!(v.headers[1].1, "***");
        assert_eq!(v.headers[2].1, "*/*");
        assert_eq!(v.body.as_deref(), Some("{\"a\":1}"));
        let v = request_view(&spec, &Redactor::default(), true);
        assert_eq!(v.headers[0].1, "Bearer abc.def");
    }

    #[test]
    fn sizes_read_well() {
        assert_eq!(human_size(10 * 1024 * 1024), "10 MB");
        assert_eq!(human_size(512 * 1024), "512 KB");
        assert_eq!(human_size(1000), "1000 bytes");
    }
}
