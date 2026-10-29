//! Turns parsed flags into a typed request, applying curl's rules for
//! methods, bodies and default headers.

use super::expand::{self, Value, Vars};
use super::flags::{Arg, Kind, Parsed};
use crate::diag::{Diag, Span};
use crate::http::body;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_MAX_REDIRECTS: u32 = 50;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequestSpec {
    pub method: String,
    pub url: String,
    /// Everything Sankh sends, in order, except what the HTTP client adds
    /// itself (`Host`, `Content-Length`, `Accept-Encoding`).
    pub headers: Vec<(String, String)>,
    #[serde(skip)]
    pub body: Option<Vec<u8>>,
    pub options: Options,
    pub cookies: CookieOptions,
}

impl RequestSpec {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum HttpVersion {
    /// HTTP/2 when offered over TLS, else HTTP/1.1.
    #[default]
    Auto,
    Http1,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub struct Resolve {
    pub host: String,
    pub port: u16,
    pub addrs: Vec<std::net::IpAddr>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Options {
    pub follow_redirects: bool,
    /// Keep credentials when a redirect changes host (`--location-trusted`).
    pub location_trusted: bool,
    pub max_redirects: u32,
    /// `--max-time`; `None` uses `@timeout` or the collection default.
    pub timeout: Option<Duration>,
    pub connect_timeout: Option<Duration>,
    pub insecure: bool,
    pub compressed: bool,
    pub proxy: Option<String>,
    pub noproxy: Option<String>,
    pub ca_cert: Option<PathBuf>,
    pub client_cert: Option<PathBuf>,
    pub client_key: Option<PathBuf>,
    pub resolve: Vec<Resolve>,
    pub http: HttpVersion,
    pub retry: u32,
    pub retry_delay: Option<Duration>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            follow_redirects: false,
            location_trusted: false,
            max_redirects: DEFAULT_MAX_REDIRECTS,
            timeout: None,
            connect_timeout: None,
            insecure: false,
            compressed: false,
            proxy: None,
            noproxy: None,
            ca_cert: None,
            client_cert: None,
            client_key: None,
            resolve: Vec::new(),
            http: HttpVersion::Auto,
            retry: 0,
            retry_delay: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct CookieOptions {
    /// Send and store cookies with the run's jar (`-b file`, `-c`).
    pub engine: bool,
    /// Cookie files to load into the jar.
    pub load: Vec<PathBuf>,
    /// File to write the jar to after the request.
    pub save: Option<PathBuf>,
}

pub struct BuildCtx<'a> {
    pub vars: &'a dyn Vars,
    /// Collection root for file paths; `None` leaves files unread (static check).
    pub root: Option<&'a Path>,
}

pub struct Built {
    pub spec: RequestSpec,
    pub warnings: Vec<Diag>,
}

/// Builds the request; all problems found are returned together.
pub fn build(p: &Parsed, ctx: &BuildCtx) -> Result<Built, Vec<Diag>> {
    let mut b = Builder {
        ctx,
        errors: Vec::new(),
        warnings: Vec::new(),
    };
    let spec = b.run(p);
    if b.errors.is_empty() {
        Ok(Built {
            spec,
            warnings: b.warnings,
        })
    } else {
        Err(b.errors)
    }
}

struct Builder<'a> {
    ctx: &'a BuildCtx<'a>,
    errors: Vec<Diag>,
    warnings: Vec<Diag>,
}

/// A header the user wrote with `-H`.
enum Custom {
    Set(String, String),
    Remove(String),
}

struct FormPart {
    name: String,
    content: Vec<u8>,
    filename: Option<String>,
    content_type: Option<String>,
}

impl Builder<'_> {
    fn value(&mut self, a: &Arg) -> Option<(Value, Span)> {
        let w = a.value.as_ref()?;
        match expand::expand(w, self.ctx.vars) {
            Ok(v) => Some((v, w.span)),
            Err(d) => {
                self.errors.push(d);
                None
            }
        }
    }

    fn err(&mut self, d: Diag) {
        self.errors.push(d);
    }

    /// Resolves a file path inside the collection; `None` when it cannot or
    /// need not be checked.
    fn path(&mut self, text: &str, span: Span, deferred: bool) -> Option<PathBuf> {
        let root = self.ctx.root?;
        if deferred {
            return None;
        }
        let rel = Path::new(text);
        if text.is_empty() {
            self.err(Diag::error("empty file name").config().at(span));
            return None;
        }
        if rel.is_absolute() || text.starts_with('~') {
            self.err(
                Diag::error(format!("`{text}` is outside the collection"))
                    .config()
                    .at(span)
                    .label("absolute paths are not allowed")
                    .help("use a path relative to the collection root"),
            );
            return None;
        }
        let joined = root.join(rel);
        let canon = match joined.canonicalize() {
            Ok(c) => c,
            Err(_) => {
                self.err(
                    Diag::error(format!("file `{text}` not found"))
                        .config()
                        .at(span)
                        .help(format!(
                            "paths are relative to the collection root ({})",
                            root.display()
                        )),
                );
                return None;
            }
        };
        if !canon.starts_with(root) {
            self.err(
                Diag::error(format!("`{text}` is outside the collection"))
                    .config()
                    .at(span)
                    .help("files must be inside the collection folder"),
            );
            return None;
        }
        Some(canon)
    }

    /// A path that may not exist yet (cookie jar output): its folder must.
    fn output_path(&mut self, text: &str, span: Span, deferred: bool) -> Option<PathBuf> {
        let root = self.ctx.root?;
        if deferred {
            return None;
        }
        let rel = Path::new(text);
        let name = rel.file_name().map(|n| n.to_os_string());
        let (Some(name), false) = (name, rel.is_absolute() || text.starts_with('~')) else {
            self.err(
                Diag::error(format!("`{text}` is outside the collection"))
                    .config()
                    .at(span)
                    .help("use a path relative to the collection root"),
            );
            return None;
        };
        let parent = rel.parent().filter(|p| !p.as_os_str().is_empty());
        let dir = match parent {
            Some(p) => self.path(&p.to_string_lossy(), span, false)?,
            None => root.to_path_buf(),
        };
        Some(dir.join(name))
    }

    fn read(&mut self, text: &str, span: Span, deferred: bool) -> Vec<u8> {
        if text == "-" {
            self.err(
                Diag::error("reading from stdin is not supported")
                    .at(span)
                    .help("put the data in a file inside the collection"),
            );
            return Vec::new();
        }
        let Some(path) = self.path(text, span, deferred) else {
            return Vec::new();
        };
        match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.err(
                    Diag::error(format!("cannot read `{text}`: {e}"))
                        .config()
                        .at(span),
                );
                Vec::new()
            }
        }
    }

    fn seconds(&mut self, a: &Arg) -> Option<Duration> {
        let (v, span) = self.value(a)?;
        if v.deferred {
            return None;
        }
        match v.text.trim().parse::<f64>() {
            Ok(0.0) => None,
            Ok(s) if s.is_finite() && s > 0.0 && s < 1e9 => Some(Duration::from_secs_f64(s)),
            _ => {
                self.err(
                    Diag::error(format!("{} needs a number of seconds", a.name))
                        .at(span)
                        .label(format!("`{}` is not a number of seconds", v.text)),
                );
                None
            }
        }
    }

    fn integer(&mut self, a: &Arg, min: i64) -> Option<i64> {
        let (v, span) = self.value(a)?;
        if v.deferred {
            return None;
        }
        match v.text.trim().parse::<i64>() {
            Ok(n) if n >= min => Some(n),
            _ => {
                self.err(
                    Diag::error(format!("{} needs a whole number", a.name))
                        .at(span)
                        .label(format!("`{}` is not valid here", v.text)),
                );
                None
            }
        }
    }

    fn run(&mut self, p: &Parsed) -> RequestSpec {
        let mut opts = Options::default();
        let mut cookies = CookieOptions::default();
        let mut method: Option<(String, Span)> = None;
        let mut urls: Vec<(Value, Span)> = Vec::new();
        let mut customs: Vec<Custom> = Vec::new();
        let mut data: Option<Vec<u8>> = None;
        let mut data_span: Option<Span> = None;
        let mut json = false;
        let mut form: Vec<FormPart> = Vec::new();
        let mut form_span: Option<Span> = None;
        let mut get = false;
        let mut head: Option<Span> = None;
        let mut auth: Option<String> = None;
        let mut user_agent: Option<String> = None;
        let mut referer: Option<String> = None;
        let mut cookie_strings: Vec<String> = Vec::new();

        for (_, w) in &p.positionals {
            match expand::expand(w, self.ctx.vars) {
                Ok(v) => urls.push((v, w.span)),
                Err(d) => self.err(d),
            }
        }

        for a in &p.args {
            match a.flag.kind {
                Kind::Url => {
                    if let Some(v) = self.value(a) {
                        urls.push(v);
                    }
                }
                Kind::Request => {
                    if let Some((v, span)) = self.value(a) {
                        if !v.deferred && !is_token(&v.text) {
                            self.err(
                                Diag::error(format!("invalid method `{}`", v.text))
                                    .at(span)
                                    .label("a method is a single word such as POST"),
                            );
                        }
                        method = Some((v.text, span));
                    }
                }
                Kind::Header => {
                    if let Some((v, span)) = self.value(a) {
                        if let Some(c) = self.header(&v, span) {
                            customs.push(c);
                        }
                    }
                }
                Kind::Data
                | Kind::DataRaw
                | Kind::DataBinary
                | Kind::DataUrlencode
                | Kind::Json => {
                    let Some((v, span)) = self.value(a) else {
                        continue;
                    };
                    data_span.get_or_insert(span);
                    let piece = match a.flag.kind {
                        Kind::Data => match v.text.strip_prefix('@') {
                            Some(path) => {
                                let mut bytes = self.read(path, span, v.deferred);
                                bytes.retain(|b| *b != b'\r' && *b != b'\n');
                                bytes
                            }
                            None => v.text.into_bytes(),
                        },
                        Kind::DataBinary | Kind::Json => match v.text.strip_prefix('@') {
                            Some(path) => self.read(path, span, v.deferred),
                            None => v.text.into_bytes(),
                        },
                        Kind::DataUrlencode => self.urlencoded(&v, span),
                        _ => v.text.into_bytes(),
                    };
                    let buf = data.get_or_insert_with(Vec::new);
                    if a.flag.kind == Kind::Json {
                        json = true;
                    } else if !buf.is_empty() {
                        buf.push(b'&');
                    }
                    buf.extend(piece);
                }
                Kind::Form | Kind::FormString => {
                    if let Some((v, span)) = self.value(a) {
                        form_span.get_or_insert(span);
                        if let Some(part) =
                            self.form_part(&v, span, a.flag.kind == Kind::FormString)
                        {
                            form.push(part);
                        }
                    }
                }
                Kind::Get => get = true,
                Kind::Head => head = Some(a.span),
                Kind::User => {
                    if let Some((v, _)) = self.value(a) {
                        use base64::Engine;
                        let creds = if v.text.contains(':') {
                            v.text
                        } else {
                            format!("{}:", v.text)
                        };
                        auth = Some(format!(
                            "Basic {}",
                            base64::engine::general_purpose::STANDARD.encode(creds)
                        ));
                    }
                }
                Kind::UserAgent => {
                    if let Some((v, _)) = self.value(a) {
                        user_agent = Some(v.text);
                    }
                }
                Kind::Referer => {
                    if let Some((v, span)) = self.value(a) {
                        if v.text.contains(";auto") {
                            self.err(
                                Diag::error("`--referer ...;auto` is not supported")
                                    .at(span)
                                    .help("give the Referer value only"),
                            );
                        }
                        referer = Some(v.text);
                    }
                }
                Kind::Location => opts.follow_redirects = true,
                Kind::LocationTrusted => {
                    opts.follow_redirects = true;
                    opts.location_trusted = true;
                }
                Kind::MaxRedirs => {
                    if let Some(n) = self.integer(a, -1) {
                        opts.max_redirects = if n < 0 {
                            u32::MAX
                        } else {
                            n.min(u32::MAX as i64) as u32
                        };
                    }
                }
                Kind::MaxTime => opts.timeout = self.seconds(a).or(opts.timeout),
                Kind::ConnectTimeout => {
                    opts.connect_timeout = self.seconds(a).or(opts.connect_timeout)
                }
                Kind::Compressed => opts.compressed = true,
                Kind::Insecure => opts.insecure = true,
                Kind::Cookie => {
                    if let Some((v, span)) = self.value(a) {
                        if v.text.contains('=') {
                            cookie_strings.push(v.text);
                        } else {
                            cookies.engine = true;
                            if let Some(path) = self.path(&v.text, span, v.deferred) {
                                cookies.load.push(path);
                            }
                        }
                    }
                }
                Kind::CookieJar => {
                    if let Some((v, span)) = self.value(a) {
                        cookies.engine = true;
                        cookies.save = self.output_path(&v.text, span, v.deferred);
                    }
                }
                Kind::Proxy => {
                    if let Some((v, span)) = self.value(a) {
                        opts.proxy = self.proxy(&v, span);
                    }
                }
                Kind::NoProxy => {
                    if let Some((v, _)) = self.value(a) {
                        opts.noproxy = Some(v.text);
                    }
                }
                Kind::CaCert | Kind::Cert | Kind::Key => {
                    if let Some((v, span)) = self.value(a) {
                        let path = self.path(&v.text, span, v.deferred);
                        match a.flag.kind {
                            Kind::CaCert => opts.ca_cert = path,
                            Kind::Cert => opts.client_cert = path,
                            _ => opts.client_key = path,
                        }
                    }
                }
                Kind::Resolve => {
                    if let Some((v, span)) = self.value(a) {
                        if let Some(r) = self.resolve(&v, span) {
                            opts.resolve.push(r);
                        }
                    }
                }
                Kind::Retry => {
                    if let Some(n) = self.integer(a, 0) {
                        opts.retry = n.min(100) as u32;
                    }
                }
                Kind::RetryDelay => opts.retry_delay = self.seconds(a),
                Kind::Http11 => opts.http = HttpVersion::Http1,
                Kind::Http2 => opts.http = HttpVersion::Auto,
                Kind::Ignored | Kind::Rejected | Kind::Unsupported => {}
            }
        }

        if !form.is_empty() && data.is_some() {
            self.err(
                Diag::error("`-F` cannot be combined with data flags")
                    .at(form_span.unwrap_or(p.span))
                    .help("send either a multipart form (-F) or a body (-d, --json, ...)"),
            );
        }
        if !form.is_empty() && get {
            self.err(
                Diag::error("`-F` cannot be combined with `-G`").at(form_span.unwrap_or(p.span)),
            );
        }
        if let Some(span) = head {
            if (data.is_some() && !get) || !form.is_empty() {
                self.err(
                    Diag::error("`-I` sends a HEAD request, which cannot have a body")
                        .at(span)
                        .help("remove -I, or add -G to send the data as the query string"),
                );
            }
        }

        let has_body = !form.is_empty() || (data.is_some() && !get);
        let method = match method {
            Some((m, _)) => m,
            None if head.is_some() => "HEAD".into(),
            None if has_body => "POST".into(),
            None => "GET".into(),
        };

        let url = self.url(p, urls, if get { data.take() } else { None });

        let mut content_type = None;
        let body = if !form.is_empty() {
            let boundary = body::boundary();
            content_type = Some(format!("multipart/form-data; boundary={boundary}"));
            Some(body::multipart(
                &boundary,
                form.iter().map(|f| body::Part {
                    name: &f.name,
                    filename: f.filename.as_deref(),
                    content_type: f.content_type.as_deref(),
                    content: &f.content,
                }),
            ))
        } else if get {
            None
        } else {
            data
        };
        if body.is_some() && content_type.is_none() {
            content_type = Some(if json {
                "application/json".into()
            } else {
                "application/x-www-form-urlencoded".into()
            });
        }

        let mut generated: Vec<(String, String)> = Vec::new();
        if let Some(a) = auth {
            generated.push(("Authorization".into(), a));
        }
        match user_agent {
            Some(ua) if ua.is_empty() => {}
            Some(ua) => generated.push(("User-Agent".into(), ua)),
            None => generated.push(("User-Agent".into(), format!("sankh/{}", crate::version()))),
        }
        if !json {
            generated.push(("Accept".into(), "*/*".into()));
        }
        if let Some(r) = referer.filter(|r| !r.is_empty()) {
            generated.push(("Referer".into(), r));
        }
        if !cookie_strings.is_empty() {
            generated.push(("Cookie".into(), cookie_strings.join("; ")));
        }

        let custom_names: Vec<String> = customs
            .iter()
            .map(|c| match c {
                Custom::Set(n, _) | Custom::Remove(n) => n.clone(),
            })
            .collect();
        let overridden = |name: &str| custom_names.iter().any(|n| n.eq_ignore_ascii_case(name));
        let mut headers: Vec<(String, String)> = generated
            .into_iter()
            .filter(|(n, _)| !overridden(n))
            .collect();
        let user_content_type = customs
            .iter()
            .any(|c| matches!(c, Custom::Set(n, _) if n.eq_ignore_ascii_case("content-type")));
        for c in customs {
            if let Custom::Set(n, v) = c {
                headers.push((n, v));
            }
        }
        if let Some(ct) = content_type {
            if !overridden("content-type") {
                headers.push(("Content-Type".into(), ct));
            }
        }
        if json && !overridden("accept") {
            headers.push(("Accept".into(), "application/json".into()));
        }

        if let (Some(b), false, false, true) = (&body, json, user_content_type, form.is_empty()) {
            let start = b.iter().find(|c| !c.is_ascii_whitespace());
            if matches!(start, Some(b'{') | Some(b'[')) && !overridden("content-type") {
                let mut d = Diag::warning(
                    "the body looks like JSON but is sent as application/x-www-form-urlencoded",
                )
                .help("add -H 'Content-Type: application/json', or use --json");
                if let Some(span) = data_span {
                    d = d.at(span);
                }
                self.warnings.push(d);
            }
        }

        RequestSpec {
            method,
            url,
            headers,
            body,
            options: opts,
            cookies,
        }
    }

    fn header(&mut self, v: &Value, span: Span) -> Option<Custom> {
        let text = &v.text;
        if text.starts_with('@') {
            self.err(
                Diag::error("reading headers from a file is not supported")
                    .at(span)
                    .help("write each header with its own -H"),
            );
            return None;
        }
        let colon = text.find(':');
        let semi = text.find(';');
        let (name, value, empty) = match (colon, semi) {
            (Some(c), _) => (
                &text[..c],
                text[c + 1..].trim_start_matches([' ', '\t']),
                false,
            ),
            (None, Some(s)) if text[s + 1..].trim().is_empty() => (&text[..s], "", true),
            _ => {
                if !v.deferred {
                    self.err(
                        Diag::error(format!("header `{text}` has no `:`"))
                            .at(span)
                            .help("write `Name: value`"),
                    );
                }
                return None;
            }
        };
        let name = name.trim();
        if v.deferred && name.is_empty() {
            return None;
        }
        if !is_token(name) {
            self.err(
                Diag::error(format!("invalid header name `{name}`"))
                    .at(span)
                    .label("header names cannot contain spaces or separators"),
            );
            return None;
        }
        let value = value.trim_end_matches([' ', '\t']);
        if value.contains(['\r', '\n', '\0']) {
            self.err(
                Diag::error(format!(
                    "the value of header `{name}` contains a line break"
                ))
                .config()
                .at(span),
            );
            return None;
        }
        if empty {
            return Some(Custom::Set(name.to_string(), String::new()));
        }
        if value.is_empty() {
            let lower = name.to_ascii_lowercase();
            if [
                "accept",
                "host",
                "content-length",
                "transfer-encoding",
                "connection",
                "accept-encoding",
            ]
            .contains(&lower.as_str())
            {
                self.err(
                    Diag::error(format!("Sankh cannot remove the `{name}` header"))
                        .at(span)
                        .help(format!("set a value instead, e.g. -H '{name}: ...'")),
                );
                return None;
            }
            return Some(Custom::Remove(name.to_string()));
        }
        Some(Custom::Set(name.to_string(), value.to_string()))
    }

    /// `--data-urlencode` forms, following curl: `=content`, `name=content`,
    /// `@file`, `name@file` and plain `content`.
    fn urlencoded(&mut self, v: &Value, span: Span) -> Vec<u8> {
        let text = &v.text;
        let sep = text.find('=').or_else(|| text.find('@'));
        let (name, content) = match sep {
            Some(i) if text.as_bytes()[i] == b'@' => {
                let content = self.read(&text[i + 1..], span, v.deferred);
                (&text[..i], content)
            }
            Some(i) => (&text[..i], text.as_bytes()[i + 1..].to_vec()),
            None => ("", text.as_bytes().to_vec()),
        };
        let mut out = Vec::new();
        if !name.is_empty() {
            out.extend_from_slice(name.as_bytes());
            out.push(b'=');
        }
        out.extend(body::urlencode(&content).into_bytes());
        out
    }

    fn form_part(&mut self, v: &Value, span: Span, literal: bool) -> Option<FormPart> {
        let Some((name, rest)) = v.text.split_once('=') else {
            if !v.deferred {
                self.err(
                    Diag::error("form fields need `name=value`")
                        .at(span)
                        .label(format!("`{}` has no `=`", v.text)),
                );
            }
            return None;
        };
        let name = name.to_string();
        if literal {
            return Some(FormPart {
                name,
                content: rest.as_bytes().to_vec(),
                filename: None,
                content_type: None,
            });
        }
        let (content, params) = match split_form_value(rest) {
            Ok(x) => x,
            Err(msg) => {
                self.err(Diag::error(msg).at(span));
                return None;
            }
        };
        let mut filename = None;
        let mut content_type = None;
        for (key, value) in params {
            match key.to_ascii_lowercase().as_str() {
                "type" => content_type = Some(value),
                "filename" => filename = Some(value),
                other => {
                    self.err(
                        Diag::error(format!("`;{other}=` in form fields is not supported"))
                            .at(span)
                            .help("only `;type=` and `;filename=` are supported"),
                    );
                    return None;
                }
            }
        }
        if let Some(path) = content.strip_prefix('@') {
            let bytes = self.read(path, span, v.deferred);
            let base = Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let guessed = body::guess_type(path);
            Some(FormPart {
                name,
                content: bytes,
                filename: Some(filename.unwrap_or(base)),
                content_type: Some(content_type.unwrap_or(guessed.to_string())),
            })
        } else if let Some(path) = content.strip_prefix('<') {
            Some(FormPart {
                name,
                content: self.read(path, span, v.deferred),
                filename,
                content_type,
            })
        } else {
            Some(FormPart {
                name,
                content: content.into_bytes(),
                filename,
                content_type,
            })
        }
    }

    fn proxy(&mut self, v: &Value, span: Span) -> Option<String> {
        if v.deferred {
            return None;
        }
        let text = if v.text.contains("://") {
            v.text.clone()
        } else {
            format!("http://{}", v.text)
        };
        match url::Url::parse(&text) {
            Ok(u) if matches!(u.scheme(), "http" | "https") => Some(text),
            Ok(u) => {
                self.err(
                    Diag::error(format!("{} proxies are not supported", u.scheme()))
                        .at(span)
                        .help("use an http:// or https:// proxy"),
                );
                None
            }
            Err(e) => {
                self.err(
                    Diag::error(format!("invalid proxy `{}`: {e}", v.text))
                        .config()
                        .at(span),
                );
                None
            }
        }
    }

    fn resolve(&mut self, v: &Value, span: Span) -> Option<Resolve> {
        if v.deferred {
            return None;
        }
        let bad = |msg: &str| {
            Diag::error(format!("invalid --resolve `{}`", v.text))
                .at(span)
                .label(msg.to_string())
                .help("write `host:port:address`, e.g. `api.test:443:127.0.0.1`")
        };
        let mut parts = v.text.splitn(3, ':');
        let (Some(host), Some(port), Some(addrs)) = (parts.next(), parts.next(), parts.next())
        else {
            self.err(bad("expected three parts"));
            return None;
        };
        if host.starts_with(['+', '-']) || host == "*" {
            self.err(bad("wildcards and `+`/`-` prefixes are not supported"));
            return None;
        }
        let Ok(port) = port.parse::<u16>() else {
            self.err(bad("invalid port"));
            return None;
        };
        let mut ips = Vec::new();
        for a in addrs.split(',') {
            match a
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse()
            {
                Ok(ip) => ips.push(ip),
                Err(_) => {
                    self.err(bad(&format!("`{a}` is not an IP address")));
                    return None;
                }
            }
        }
        Some(Resolve {
            host: host.to_ascii_lowercase(),
            port,
            addrs: ips,
        })
    }

    fn url(&mut self, p: &Parsed, urls: Vec<(Value, Span)>, query: Option<Vec<u8>>) -> String {
        let mut urls = urls.into_iter();
        let Some((first, span)) = urls.next() else {
            self.err(
                Diag::error("the curl command has no URL")
                    .at(Span::new(p.span.start, p.span.start + 4))
                    .help("add the URL, e.g. curl -sS \"$BASE_URL/users\""),
            );
            return String::new();
        };
        for (_, extra) in urls {
            self.err(
                Diag::error("more than one URL")
                    .at(extra)
                    .label("second URL")
                    .help("a request file sends one request; if this is a flag's value, check the flag before it"),
            );
        }
        let mut text = first.text.trim().to_string();
        if let Some(q) = query.filter(|q| !q.is_empty()) {
            let fragment = text.find('#').map(|i| text.split_off(i));
            text.push(if text.contains('?') { '&' } else { '?' });
            text.push_str(&String::from_utf8_lossy(&q));
            if let Some(f) = fragment {
                text.push_str(&f);
            }
        }
        if first.deferred {
            return text;
        }
        if text.is_empty() {
            self.err(Diag::error("the URL is empty").config().at(span));
            return text;
        }
        if let Some(c) = text.chars().find(|c| c.is_whitespace() || c.is_control()) {
            self.err(
                Diag::error(format!("the URL contains {c:?}"))
                    .config()
                    .at(span)
                    .help("percent-encode it, e.g. a space as %20"),
            );
            return text;
        }
        let with_scheme = if text.contains("://") {
            text
        } else {
            format!("http://{text}")
        };
        match url::Url::parse(&with_scheme) {
            Ok(u) if matches!(u.scheme(), "http" | "https") => {
                if u.host_str().is_none_or(str::is_empty) {
                    self.err(
                        Diag::error(format!("the URL `{with_scheme}` has no host"))
                            .config()
                            .at(span),
                    );
                }
                with_scheme
            }
            Ok(u) => {
                self.err(
                    Diag::error(format!("{} URLs are not supported", u.scheme()))
                        .at(span)
                        .help("Sankh sends http and https requests only"),
                );
                with_scheme
            }
            Err(e) => {
                self.err(
                    Diag::error(format!("invalid URL `{with_scheme}`: {e}"))
                        .config()
                        .at(span),
                );
                with_scheme
            }
        }
    }
}

/// An HTTP token: method or header name.
fn is_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Splits a `-F` value into its content and `;key=value` parameters. Content
/// may be double-quoted to contain `;`.
fn split_form_value(text: &str) -> Result<(String, Vec<(String, String)>), String> {
    fn take(s: &str) -> Result<(String, &str), String> {
        if let Some(rest) = s.strip_prefix('"') {
            let mut out = String::new();
            let mut chars = rest.char_indices();
            while let Some((i, c)) = chars.next() {
                match c {
                    '\\' => {
                        if let Some((_, n)) = chars.next() {
                            out.push(n);
                        }
                    }
                    '"' => return Ok((out, &rest[i + 1..])),
                    c => out.push(c),
                }
            }
            Err("unterminated quote in form field".into())
        } else {
            let end = param_start(s).unwrap_or(s.len());
            Ok((s[..end].to_string(), &s[end..]))
        }
    }
    /// Index of the first `;` that starts a known parameter.
    fn param_start(s: &str) -> Option<usize> {
        s.match_indices(';').map(|(i, _)| i).find(|&i| {
            let rest = s[i + 1..].trim_start().to_ascii_lowercase();
            ["type=", "filename=", "headers=", "encoder="]
                .iter()
                .any(|p| rest.starts_with(p))
        })
    }
    let (content, mut rest) = match text.strip_prefix(['@', '<']) {
        Some(path) => {
            let (path, rest) = take(path)?;
            (format!("{}{path}", &text[..1]), rest)
        }
        None => take(text)?,
    };
    let mut params = Vec::new();
    while let Some(r) = rest.strip_prefix(';') {
        let r = r.trim_start();
        let Some((key, after)) = r.split_once('=') else {
            return Err(format!("invalid form parameter `;{r}`"));
        };
        let (value, next) = take(after)?;
        params.push((key.trim().to_string(), value));
        rest = next;
    }
    if !rest.is_empty() {
        return Err(format!("unexpected `{rest}` after a quoted form value"));
    }
    Ok((content, params))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::expand::{Lookup, Unresolved};
    use crate::parse::flags::{Strictness, parse_args};
    use crate::parse::tokenize::tokenize;

    struct Env;
    impl Vars for Env {
        fn lookup(&self, name: &str) -> Lookup {
            match name {
                "BASE" => Lookup::Value("http://api.test".into()),
                "TOKEN" => Lookup::Value("t0k".into()),
                _ => Lookup::Missing,
            }
        }
    }

    fn build_with(src: &str, vars: &dyn Vars, root: Option<&Path>) -> Result<Built, Vec<Diag>> {
        let t = tokenize(src, 0).unwrap();
        let o = parse_args(&t.words, Span::new(0, src.len()), Strictness::Strict);
        assert!(o.errors.is_empty(), "{:?}", o.errors);
        build(&o.parsed, &BuildCtx { vars, root })
    }

    fn spec(src: &str) -> RequestSpec {
        build_with(src, &Env, None)
            .unwrap_or_else(|e| panic!("{src}: {e:?}"))
            .spec
    }

    fn errors(src: &str) -> Vec<String> {
        match build_with(src, &Env, None) {
            Ok(_) => vec![],
            Err(e) => e.into_iter().map(|d| d.message).collect(),
        }
    }

    fn body(s: &RequestSpec) -> String {
        String::from_utf8(s.body.clone().unwrap_or_default()).unwrap()
    }

    #[test]
    fn methods() {
        assert_eq!(spec("$BASE").method, "GET");
        assert_eq!(spec("$BASE -d x").method, "POST");
        assert_eq!(spec("$BASE -d x -G").method, "GET");
        assert_eq!(spec("$BASE -I").method, "HEAD");
        assert_eq!(spec("$BASE -X patch -d x").method, "patch");
        assert_eq!(spec("$BASE -F a=1").method, "POST");
    }

    #[test]
    fn data_joining_like_curl() {
        let s = spec("$BASE -d a -d 'b c' --data-binary x --data-raw @z");
        assert_eq!(body(&s), "a&b c&x&@z");
        assert_eq!(
            s.header("content-type"),
            Some("application/x-www-form-urlencoded")
        );
        let s = spec("$BASE -d x --json y");
        assert_eq!(body(&s), "xy");
        assert_eq!(s.header("content-type"), Some("application/json"));
        assert_eq!(s.header("accept"), Some("application/json"));
        let s = spec("$BASE --json y -d x");
        assert_eq!(body(&s), "y&x");
    }

    #[test]
    fn get_moves_data_to_query() {
        let s =
            spec("'http://h/x?z=1#f' -G -d a=1 --data-urlencode 'q=a b&c' --data-urlencode '=x y'");
        assert_eq!(s.url, "http://h/x?z=1&a=1&q=a+b%26c&x+y#f");
        assert_eq!(s.body, None);
        assert_eq!(s.header("content-type"), None);
    }

    #[test]
    fn headers_override_and_remove_defaults() {
        let s = spec(
            "$BASE -u user -A agent -e ref -H 'X-A;' -H 'Content-Type:' -H 'user-agent: mine' -d x -b 'a=1'",
        );
        assert_eq!(
            s.headers,
            vec![
                ("Authorization".to_string(), "Basic dXNlcjo=".to_string()),
                ("Accept".to_string(), "*/*".to_string()),
                ("Referer".to_string(), "ref".to_string()),
                ("Cookie".to_string(), "a=1".to_string()),
                ("X-A".to_string(), String::new()),
                ("user-agent".to_string(), "mine".to_string()),
            ]
        );
        assert_eq!(spec("$BASE -A ''").header("user-agent"), None);
        assert!(
            spec("$BASE")
                .header("user-agent")
                .unwrap()
                .starts_with("sankh/")
        );
        assert_eq!(
            errors("$BASE -H 'Accept:'"),
            vec!["Sankh cannot remove the `Accept` header"]
        );
        assert_eq!(
            errors("$BASE -H 'nocolon'"),
            vec!["header `nocolon` has no `:`"]
        );
    }

    #[test]
    fn json_looking_body_warns() {
        let b = build_with("$BASE -d '{\"a\":1}'", &Env, None).unwrap();
        assert_eq!(b.warnings.len(), 1);
        let b = build_with(
            "$BASE -d '{\"a\":1}' -H 'Content-Type: application/json'",
            &Env,
            None,
        )
        .unwrap();
        assert!(b.warnings.is_empty());
    }

    #[test]
    fn options() {
        let s = spec(
            "$BASE -sSLk --compressed -m 1.5 --connect-timeout 2 --max-redirs 3 --retry 2 --http1.1 -x proxy:3128 --resolve api.test:443:127.0.0.1,[::1]",
        );
        let o = &s.options;
        assert!(o.follow_redirects && o.insecure && o.compressed);
        assert_eq!(o.timeout, Some(Duration::from_millis(1500)));
        assert_eq!(o.max_redirects, 3);
        assert_eq!(o.retry, 2);
        assert_eq!(o.http, HttpVersion::Http1);
        assert_eq!(o.proxy.as_deref(), Some("http://proxy:3128"));
        assert_eq!(o.resolve[0].addrs.len(), 2);
    }

    #[test]
    fn url_rules() {
        assert_eq!(spec("localhost:8080/x").url, "http://localhost:8080/x");
        assert_eq!(errors("ftp://x"), vec!["ftp URLs are not supported"]);
        assert_eq!(errors("$BASE $BASE"), vec!["more than one URL"]);
        assert_eq!(errors("-sS"), vec!["the curl command has no URL"]);
        assert_eq!(
            errors("$BASE -H \"A: $NOPE\""),
            vec!["variable NOPE is not defined"]
        );
        assert_eq!(
            errors("$BASE -I -d x"),
            vec!["`-I` sends a HEAD request, which cannot have a body"]
        );
        assert_eq!(
            errors("$BASE -F a=1 -d x"),
            vec!["`-F` cannot be combined with data flags"]
        );
        assert_eq!(
            errors("$BASE -m soon"),
            vec!["-m needs a number of seconds"]
        );
        assert_eq!(errors("$BASE -X 'GE T'"), vec!["invalid method `GE T`"]);
    }

    #[test]
    fn deferred_values_skip_validation() {
        let t = tokenize("\"$URL\" -X \"$M\" -H \"$H\" -m \"$T\"", 0).unwrap();
        let o = parse_args(&t.words, Span::new(0, 10), Strictness::Strict);
        let b = build(
            &o.parsed,
            &BuildCtx {
                vars: &Unresolved,
                root: None,
            },
        );
        assert!(b.is_ok(), "{:?}", b.err());
    }

    #[test]
    fn files_stay_inside_the_collection() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("body.json"), "{\"a\":\r\n1}\n").unwrap();
        std::fs::write(root.join("logo.png"), [0x89, b'P']).unwrap();
        let s = build_with("$BASE -d @body.json", &Env, Some(&root))
            .unwrap()
            .spec;
        assert_eq!(body(&s), "{\"a\":1}");
        let s = build_with("$BASE --data-binary @body.json", &Env, Some(&root))
            .unwrap()
            .spec;
        assert_eq!(body(&s), "{\"a\":\r\n1}\n");
        let errs = |src: &str| -> Vec<String> {
            build_with(src, &Env, Some(&root))
                .err()
                .unwrap_or_default()
                .into_iter()
                .map(|d| d.message)
                .collect()
        };
        assert_eq!(
            errs("$BASE -d @/etc/passwd"),
            vec!["`/etc/passwd` is outside the collection"]
        );
        assert_eq!(errs("$BASE -d @../x"), vec!["file `../x` not found"]);
        assert_eq!(
            errs("$BASE -d @missing.json"),
            vec!["file `missing.json` not found"]
        );
        assert_eq!(
            errs("$BASE -d @-"),
            vec!["reading from stdin is not supported"]
        );
        let s = build_with(
            "$BASE -F 'f=@logo.png' -F 'n=\"a;b\";type=text/plain' -F 't=<body.json'",
            &Env,
            Some(&root),
        )
        .unwrap()
        .spec;
        let ct = s.header("content-type").unwrap().to_string();
        let boundary = ct.strip_prefix("multipart/form-data; boundary=").unwrap();
        let text = String::from_utf8_lossy(s.body.as_ref().unwrap()).into_owned();
        assert!(text.starts_with(&format!("--{boundary}\r\nContent-Disposition: form-data; name=\"f\"; filename=\"logo.png\"\r\nContent-Type: image/png\r\n\r\n")), "{text}");
        assert!(
            text.contains("name=\"n\"\r\nContent-Type: text/plain\r\n\r\na;b\r\n"),
            "{text}"
        );
        assert!(text.ends_with(&format!("--{boundary}--\r\n")));
    }

    #[test]
    fn form_values() {
        assert_eq!(
            split_form_value("a;b").unwrap(),
            ("a;b".to_string(), vec![])
        );
        assert_eq!(
            split_form_value("@x.bin;type=a/b;filename=\"y z\"").unwrap(),
            (
                "@x.bin".to_string(),
                vec![
                    ("type".into(), "a/b".into()),
                    ("filename".into(), "y z".into())
                ]
            )
        );
        assert_eq!(
            split_form_value("@\"my file;1.png\";type=image/png").unwrap(),
            (
                "@my file;1.png".to_string(),
                vec![("type".into(), "image/png".into())]
            )
        );
        assert_eq!(
            split_form_value("\"Ada\"").unwrap(),
            ("Ada".to_string(), vec![])
        );
    }
}
