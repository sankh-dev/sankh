//! Executes request files.
//!
//! The file is never modified. It is sourced by a small wrapper script that
//! defines a `curl` shell function, so the file's own `curl` call is routed
//! through it and gains flags that record the body, headers and timing:
//!
//! - `-o` goes *before* the user's arguments (curl applies the first `-o` to the URL);
//! - `-D`, `--no-include` and `-w '%{json}'` go *after* (the last one wins).
//!
//! If a raw-mode file calls curl several times, the last response is used.

use crate::assert::{self, AssertionResult};
use crate::capture;
use crate::collection::Collection;
use crate::env::Env;
use crate::parser;
use crate::redact::{self, Redactor};
use crate::request::{Expect, Request, Severity};
use crate::trust::Trusted;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const WRAPPER: &str = r#"curl() {
  command curl -o "$SANKH_OUT_BODY" --max-time "$SANKH_TIMEOUT" "$@" -D "$SANKH_OUT_HEADERS" --no-include -w '%{json}' > "$SANKH_OUT_META"
}
. "$1"
"#;
const DEFAULT_TIMEOUT_SECS: u64 = 30;
/// Bodies larger than this are truncated in results (assertions see the full body).
const MAX_BODY_DISPLAY: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub time_ms: f64,
    pub url: String,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Passed,
    Failed,
    Error,
    /// Stopped by the user before it finished.
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseView {
    pub status: u16,
    pub time_ms: f64,
    pub size: usize,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub body_truncated: bool,
    pub body_binary: bool,
    /// The raw body of an `image/*` response up to `MAX_BODY_DISPLAY`, for previews.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_base64: Option<String>,
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
    pub response: Option<ResponseView>,
    pub assertions: Vec<AssertionResult>,
    pub captures: Vec<CapturedValue>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub stderr: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub timeout: Duration,
    /// Set from another thread to stop the request in flight.
    pub cancel: Arc<AtomicBool>,
}

impl RunOptions {
    pub fn for_collection(c: &Collection) -> RunOptions {
        RunOptions {
            timeout: Duration::from_secs(c.config.timeout.unwrap_or(DEFAULT_TIMEOUT_SECS)),
            cancel: Arc::default(),
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// State carried across the requests of one run: environment (with
/// captures) and the redactor (which learns captured secrets).
pub struct RunContext {
    pub env: Env,
    pub redactor: Redactor,
    pub options: RunOptions,
}

impl RunContext {
    pub fn new(env: Env, options: RunOptions) -> RunContext {
        let redactor = Redactor::from_vars(&env.resolved());
        RunContext {
            env,
            redactor,
            options,
        }
    }
}

/// Picks the interpreter from the shebang: bash/zsh if named, else `sh`.
fn interpreter(req: &Request) -> &'static str {
    match req.shebang.as_deref() {
        Some(s) if s.contains("bash") => "bash",
        Some(s) if s.contains("zsh") => "zsh",
        _ => "sh",
    }
}

pub fn run_request(
    _trusted: &Trusted,
    collection: &Collection,
    file: &Path,
    ctx: &mut RunContext,
) -> RequestResult {
    let started = Instant::now();
    let rel = collection.rel(file);
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            return error_result(rel.clone(), rel, format!("cannot read file: {e}"), started);
        }
    };
    let req = collection.parse_file(file, &content);
    let mut result = execute(collection, file, &req, ctx);
    result.path = rel;
    result.duration_ms = started.elapsed().as_secs_f64() * 1000.0;
    result
}

fn error_result(path: String, name: String, error: String, started: Instant) -> RequestResult {
    RequestResult {
        path,
        name,
        outcome: Outcome::Error,
        response: None,
        assertions: Vec::new(),
        captures: Vec::new(),
        error: Some(error),
        warnings: Vec::new(),
        stderr: String::new(),
        duration_ms: started.elapsed().as_secs_f64() * 1000.0,
    }
}

fn execute(
    collection: &Collection,
    file: &Path,
    req: &Request,
    ctx: &mut RunContext,
) -> RequestResult {
    let started = Instant::now();
    let mut result = error_result(String::new(), req.name.clone(), String::new(), started);
    result.error = None;
    result.outcome = Outcome::Passed;

    for d in &req.diagnostics {
        let msg = format!("line {}: {}", d.line, d.message);
        if d.severity == Severity::Error {
            result.outcome = Outcome::Error;
            result.error = Some(match result.error.take() {
                Some(prev) => format!("{prev}\n{msg}"),
                None => msg,
            });
        } else {
            result.warnings.push(msg);
        }
    }
    if result.outcome == Outcome::Error {
        return result;
    }
    for var in ctx.env.missing(&req.body) {
        result.warnings.push(format!(
            "${var} is not set (define it in an env file, .env.local or the process environment)"
        ));
    }

    let response = match spawn(collection, file, req, ctx) {
        Ok((resp, stderr)) => {
            result.stderr = ctx.redactor.redact(&stderr);
            resp
        }
        Err((msg, stderr)) => {
            result.stderr = ctx.redactor.redact(&stderr);
            result.error = Some(ctx.redactor.redact(&msg));
            result.outcome = if ctx.options.cancelled() {
                Outcome::Cancelled
            } else {
                Outcome::Error
            };
            return result;
        }
    };

    let env = &ctx.env;
    let lookup = |name: &str| env.get(name).cloned();
    for patterns in req.status_expectations() {
        result
            .assertions
            .push(assert::check_status(&patterns, response.status));
    }
    for e in &req.expects {
        if let Expect::Json { expr, op, value } = e {
            result.assertions.push(assert::check_json(
                expr,
                *op,
                value.as_deref(),
                &response.body,
                &lookup,
            ));
        }
    }

    let all_passed = result.assertions.iter().all(|a| a.passed);
    if all_passed {
        for c in &req.captures {
            match capture::extract(&c.source, &response) {
                Ok(value) => {
                    if redact::is_secret_name(&c.var) {
                        ctx.redactor.add(&value);
                    }
                    ctx.env.captures.insert(c.var.clone(), value.clone());
                    result.captures.push(CapturedValue {
                        name: c.var.clone(),
                        value: value.clone(),
                    });
                }
                Err(msg) => {
                    result.error = Some(format!("capture {} failed: {msg}", c.var));
                }
            }
        }
    }

    let r = &ctx.redactor;
    for a in &mut result.assertions {
        a.message = a.message.as_deref().map(|m| r.redact(m));
    }
    for c in &mut result.captures {
        c.value = r.display_var(&c.name, &c.value);
    }
    result.response = Some(view(&response, r));
    result.outcome = if all_passed && result.error.is_none() {
        Outcome::Passed
    } else {
        Outcome::Failed
    };
    result
}

fn view(resp: &Response, r: &Redactor) -> ResponseView {
    let (body, binary) = match std::str::from_utf8(&resp.body) {
        Ok(s) => (s.to_string(), false),
        Err(_) => (format!("<{} bytes of binary data>", resp.body.len()), true),
    };
    let truncated = body.len() > MAX_BODY_DISPLAY;
    let body = if truncated {
        let mut end = MAX_BODY_DISPLAY;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        body[..end].to_string()
    } else {
        body
    };
    ResponseView {
        status: resp.status,
        time_ms: resp.time_ms,
        size: resp.body.len(),
        url: r.redact(&resp.url),
        headers: resp
            .headers
            .iter()
            .map(|(k, v)| (k.clone(), r.redact(v)))
            .collect(),
        body: r.redact(&body),
        body_truncated: truncated,
        body_binary: binary,
        body_base64: image_base64(resp),
    }
}

fn image_base64(resp: &Response) -> Option<String> {
    use base64::Engine;
    let ct = resp.header("content-type")?.trim_start();
    let is_image = ct.len() >= 6 && ct[..6].eq_ignore_ascii_case("image/");
    (is_image && resp.body.len() <= MAX_BODY_DISPLAY)
        .then(|| base64::engine::general_purpose::STANDARD.encode(&resp.body))
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| {
        // System32's bash.exe is the WSL launcher, which cannot see Windows paths.
        if cfg!(windows)
            && dir
                .to_string_lossy()
                .to_ascii_lowercase()
                .contains("system32")
        {
            return None;
        }
        let candidate = dir.join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
        let exe = dir.join(format!("{program}.exe"));
        exe.is_file().then_some(exe)
    })
}

type SpawnError = (String, String);

fn spawn(
    collection: &Collection,
    file: &Path,
    req: &Request,
    ctx: &RunContext,
) -> Result<(Response, String), SpawnError> {
    let shell_name = interpreter(req);
    let shell = find_in_path(shell_name)
        .or_else(|| (shell_name != "sh").then(|| find_in_path("sh")).flatten())
        .ok_or_else(|| {
            (
                format!(
                    "`{shell_name}` was not found. Sankh runs request files with a POSIX shell; on Windows use Git Bash or WSL"
                ),
                String::new(),
            )
        })?;
    if find_in_path("curl").is_none() {
        return Err(("`curl` was not found on PATH".into(), String::new()));
    }

    let tmp =
        tempfile::tempdir().map_err(|e| (format!("cannot create temp dir: {e}"), String::new()))?;
    let body_path = tmp.path().join("body");
    let headers_path = tmp.path().join("headers");
    let meta_path = tmp.path().join("meta.json");

    // Windows paths (`\\?\D:\...`) are mangled by a POSIX shell, so pass a
    // `./`-relative, forward-slash path; the shell runs from the root.
    let script = match file.strip_prefix(&collection.root) {
        Ok(rel) => {
            let parts: Vec<_> = rel.iter().map(|p| p.to_string_lossy()).collect();
            format!("./{}", parts.join("/"))
        }
        Err(_) => file.to_string_lossy().into_owned(),
    };

    let timeout = req.timeout().unwrap_or(ctx.options.timeout);
    let mut cmd = Command::new(&shell);
    cmd.arg("-c")
        .arg(WRAPPER)
        .arg("sankh")
        .arg(script)
        .current_dir(&collection.root)
        .env_clear()
        .envs(ctx.env.resolved())
        .env("SANKH", "1")
        .env("SANKH_OUT_BODY", &body_path)
        .env("SANKH_OUT_HEADERS", &headers_path)
        .env("SANKH_OUT_META", &meta_path)
        .env("SANKH_TIMEOUT", timeout.as_secs_f64().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);

    let child = cmd.spawn().map_err(|e| {
        (
            format!("cannot start {}: {e}", shell.display()),
            String::new(),
        )
    })?;
    let output = wait_with_timeout(child, timeout + Duration::from_secs(5), &ctx.options.cancel)
        .map_err(|e| (e, String::new()))?;
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let combined = if stdout.trim().is_empty() {
        stderr
    } else {
        format!("{stdout}{stderr}")
    };

    let meta_text = std::fs::read_to_string(&meta_path).unwrap_or_default();
    if meta_text.trim().is_empty() {
        return Err((
            format!(
                "no curl request was recorded (exit code {}). Is curl called through an absolute path or `command curl`?",
                output.status.code().map_or("?".into(), |c| c.to_string())
            ),
            combined,
        ));
    }
    let meta: serde_json::Value = serde_json::from_str(meta_text.trim())
        .map_err(|e| (format!("cannot read curl output: {e}"), combined.clone()))?;
    let status = meta["http_code"].as_u64().unwrap_or(0) as u16;
    let exit = meta["exitcode"].as_i64().unwrap_or(0);
    if status == 0 {
        let msg = meta["errormsg"].as_str().unwrap_or("request failed");
        let msg = if exit == 28 {
            format!(
                "timed out after {}: {msg}",
                parser::format_duration(timeout.as_millis() as u64)
            )
        } else {
            format!("curl error {exit}: {msg}")
        };
        return Err((msg, combined));
    }
    let headers_text = std::fs::read_to_string(&headers_path).unwrap_or_default();
    Ok((
        Response {
            status,
            headers: parse_header_dump(&headers_text),
            body: std::fs::read(&body_path).unwrap_or_default(),
            time_ms: meta["time_total"].as_f64().unwrap_or(0.0) * 1000.0,
            url: meta["url_effective"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        },
        combined,
    ))
}

/// Kills the shell and, on Unix, everything it started: a surviving `curl`
/// would hold the output pipes open and block the reader threads.
fn kill_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<std::process::Output, String> {
    use std::io::Read;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let out_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = stdout.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let err_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if cancel.load(Ordering::Relaxed) => {
                kill_tree(&mut child);
                return Err("cancelled".into());
            }
            Ok(None) if Instant::now() >= deadline => {
                kill_tree(&mut child);
                return Err(format!(
                    "request file did not finish within {}s",
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(e.to_string()),
        }
    };
    Ok(std::process::Output {
        status,
        stdout: out_thread.join().unwrap_or_default(),
        stderr: err_thread.join().unwrap_or_default(),
    })
}

/// Parses a `-D` header dump, keeping only the final response's headers
/// (redirects and `100 Continue` produce several blocks).
fn parse_header_dump(text: &str) -> Vec<(String, String)> {
    let normalized = text.replace("\r\n", "\n");
    let block = normalized
        .split("\n\n")
        .filter(|b| b.trim_start().starts_with("HTTP/"))
        .last()
        .unwrap_or("");
    block
        .lines()
        .skip(1)
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_last_header_block() {
        let dump = "HTTP/1.1 301 Moved\r\nLocation: /b\r\n\r\nHTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-A: 1\r\n\r\n";
        assert_eq!(
            parse_header_dump(dump),
            vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("X-A".to_string(), "1".to_string())
            ]
        );
    }

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
    }

    #[test]
    fn text_body_has_no_base64() {
        let v = view(
            &response("application/json", b"{}".to_vec()),
            &Redactor::default(),
        );
        assert_eq!(v.body_base64, None);
        assert!(!serde_json::to_string(&v).unwrap().contains("body_base64"));
    }

    #[test]
    fn large_image_has_no_base64() {
        let big = vec![0u8; MAX_BODY_DISPLAY + 1];
        let v = view(&response("IMAGE/JPEG", big), &Redactor::default());
        assert_eq!(v.body_base64, None);
    }
}
