//! The curl flags Sankh understands, as data: one entry per flag drives the
//! parser, its errors and the generated compatibility page.

use super::tokenize::Word;
use crate::diag::{Diag, Span};

/// What a flag does in the request model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Url,
    Request,
    Header,
    /// `-d`, `--data`, `--data-ascii`: `@file` strips newlines.
    Data,
    DataRaw,
    DataBinary,
    DataUrlencode,
    Json,
    Get,
    Head,
    User,
    UserAgent,
    Referer,
    Location,
    LocationTrusted,
    MaxRedirs,
    MaxTime,
    ConnectTimeout,
    Compressed,
    Insecure,
    Form,
    FormString,
    Cookie,
    CookieJar,
    Proxy,
    NoProxy,
    CaCert,
    Cert,
    Key,
    Resolve,
    Retry,
    RetryDelay,
    Http11,
    Http2,
    /// Accepted with no effect.
    Ignored,
    /// Conflicts with how Sankh handles responses.
    Rejected,
    /// Known curl flag that Sankh does not implement.
    Unsupported,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Flag {
    /// Long name without the leading `--`.
    pub long: &'static str,
    pub short: Option<char>,
    /// Placeholder for the value in docs; `None` for switches.
    pub value: Option<&'static str>,
    pub kind: Kind,
    /// Release tier that added support (1-3); 0 when not supported.
    pub tier: u8,
    /// Behavior, or for unsupported flags what to use instead.
    pub doc: &'static str,
}

impl Flag {
    pub fn takes_value(&self) -> bool {
        self.value.is_some()
    }

    pub fn supported(&self) -> bool {
        !matches!(self.kind, Kind::Rejected | Kind::Unsupported)
    }

    /// `-H, --header` style label.
    pub fn names(&self) -> String {
        match self.short {
            Some(s) => format!("-{s}, --{}", self.long),
            None => format!("--{}", self.long),
        }
    }
}

const fn f(
    long: &'static str,
    short: Option<char>,
    value: Option<&'static str>,
    kind: Kind,
    tier: u8,
    doc: &'static str,
) -> Flag {
    Flag {
        long,
        short,
        value,
        kind,
        tier,
        doc,
    }
}

const OUTPUT: &str =
    "Sankh records the status, headers, body and timing itself; use `# @capture` and `# @expect`.";
const NO_EFFECT: &str = "Accepted; no effect.";

use Kind::*;

pub static FLAGS: &[Flag] = &[
    // Tier 1
    f(
        "url",
        None,
        Some("<url>"),
        Url,
        1,
        "The URL, as an alternative to a positional argument. Exactly one URL per file; `http` or `https`. Without a scheme, `http://` is assumed.",
    ),
    f(
        "request",
        Some('X'),
        Some("<method>"),
        Request,
        1,
        "Request method.",
    ),
    f(
        "header",
        Some('H'),
        Some("<header>"),
        Header,
        1,
        "Adds a header; duplicates are sent in order. A header of the same name replaces the one Sankh would add. `Name:` removes it; `Name;` sends it with an empty value.",
    ),
    f(
        "data",
        Some('d'),
        Some("<data>"),
        Data,
        1,
        "Request body; makes the request a `POST`. Several data flags are joined with `&`. `@file` reads a file and strips CR and LF. Without a `Content-Type` header, `application/x-www-form-urlencoded` is sent, as curl does.",
    ),
    f("data-ascii", None, Some("<data>"), Data, 1, "Same as `-d`."),
    f(
        "data-raw",
        None,
        Some("<data>"),
        DataRaw,
        1,
        "Like `-d`, but `@` has no special meaning.",
    ),
    f(
        "user",
        Some('u'),
        Some("<user:password>"),
        User,
        1,
        "Basic authentication. Without `:password` the password is empty (no prompt).",
    ),
    f(
        "get",
        Some('G'),
        None,
        Get,
        1,
        "Sends the data flags as the query string of a `GET`.",
    ),
    f("silent", Some('s'), None, Ignored, 1, NO_EFFECT),
    f("show-error", Some('S'), None, Ignored, 1, NO_EFFECT),
    f(
        "include",
        Some('i'),
        None,
        Ignored,
        1,
        "Accepted; no effect. Sankh always records response headers.",
    ),
    f(
        "verbose",
        Some('v'),
        None,
        Ignored,
        1,
        "Accepted; no effect. Sankh always records the request it sent.",
    ),
    f("output", Some('o'), Some("<file>"), Rejected, 1, OUTPUT),
    f("remote-name", Some('O'), None, Rejected, 1, OUTPUT),
    f(
        "dump-header",
        Some('D'),
        Some("<file>"),
        Rejected,
        1,
        OUTPUT,
    ),
    f(
        "write-out",
        Some('w'),
        Some("<format>"),
        Rejected,
        1,
        OUTPUT,
    ),
    f("output-dir", None, Some("<dir>"), Rejected, 1, OUTPUT),
    f("remote-header-name", Some('J'), None, Rejected, 1, OUTPUT),
    // Tier 2
    f(
        "location",
        Some('L'),
        None,
        Location,
        2,
        "Follows redirects. A `POST` becomes a `GET` without a body on 301, 302 and 303; 307 and 308 keep the method and body. `Authorization` and `Cookie` headers are not sent to another host.",
    ),
    f(
        "location-trusted",
        None,
        None,
        LocationTrusted,
        2,
        "Like `-L`, and keeps sending credentials to other hosts.",
    ),
    f(
        "max-redirs",
        None,
        Some("<num>"),
        MaxRedirs,
        2,
        "Most redirects to follow with `-L` (default 50; -1 for no limit).",
    ),
    f(
        "max-time",
        Some('m'),
        Some("<seconds>"),
        MaxTime,
        2,
        "Limit for each attempt, redirects included; decimals allowed. Overrides `# @timeout` and `timeout` in `sankh.toml`; 0 keeps them.",
    ),
    f(
        "connect-timeout",
        None,
        Some("<seconds>"),
        ConnectTimeout,
        2,
        "Limit for connecting, decimals allowed; 0 for none.",
    ),
    f(
        "compressed",
        None,
        None,
        Compressed,
        2,
        "Sends `Accept-Encoding` and decodes gzip, deflate and brotli bodies. Without it the body is returned as received.",
    ),
    f(
        "insecure",
        Some('k'),
        None,
        Insecure,
        2,
        "Skips certificate and hostname verification. Sankh prints a warning once per run.",
    ),
    f(
        "data-urlencode",
        None,
        Some("<data>"),
        DataUrlencode,
        2,
        "URL-encodes data: `name=value` encodes the value, `=value` the whole value, `name@file` and `@file` a file's contents.",
    ),
    f(
        "data-binary",
        None,
        Some("<data>"),
        DataBinary,
        2,
        "Like `-d`, but `@file` keeps the file's bytes as they are.",
    ),
    f(
        "user-agent",
        Some('A'),
        Some("<name>"),
        UserAgent,
        2,
        "Sets `User-Agent` (default `sankh/<version>`). An empty value removes it.",
    ),
    f(
        "referer",
        Some('e'),
        Some("<url>"),
        Referer,
        2,
        "Sets `Referer`.",
    ),
    f("head", Some('I'), None, Head, 2, "Sends a `HEAD` request."),
    f(
        "json",
        None,
        Some("<data>"),
        Json,
        2,
        "JSON body: like `--data-binary`, and sets `Content-Type` and `Accept` to `application/json`. Several are concatenated.",
    ),
    // Tier 3
    f(
        "form",
        Some('F'),
        Some("<name=content>"),
        Form,
        3,
        "Multipart form field: `name=value`, `name=@file` (upload), `name=<file` (text from a file), with `;type=` and `;filename=`.",
    ),
    f(
        "form-string",
        None,
        Some("<name=string>"),
        FormString,
        3,
        "Multipart field whose value is used as is (`@`, `<` and `;` are literal).",
    ),
    f(
        "cookie",
        Some('b'),
        Some("<data|file>"),
        Cookie,
        3,
        "`name=value` pairs to send, or a cookie file to load (Netscape format). Enables the run's cookie jar.",
    ),
    f(
        "cookie-jar",
        Some('c'),
        Some("<file>"),
        CookieJar,
        3,
        "Writes the run's cookies to a file in the collection, and keeps cookies between requests of the run.",
    ),
    f(
        "proxy",
        Some('x'),
        Some("<url>"),
        Proxy,
        3,
        "HTTP or HTTPS proxy. Without it, `HTTP_PROXY`, `HTTPS_PROXY` and `NO_PROXY` apply (set `env_proxy = false` in `sankh.toml` to ignore them).",
    ),
    f(
        "noproxy",
        None,
        Some("<hosts>"),
        NoProxy,
        3,
        "Comma-separated hosts that bypass the proxy; `*` for all.",
    ),
    f(
        "cacert",
        None,
        Some("<file>"),
        CaCert,
        3,
        "PEM file of certificates to trust instead of the system store.",
    ),
    f(
        "cert",
        Some('E'),
        Some("<file>"),
        Cert,
        3,
        "Client certificate (PEM), optionally with its private key.",
    ),
    f(
        "key",
        None,
        Some("<file>"),
        Key,
        3,
        "Private key (PEM) for `--cert`.",
    ),
    f(
        "resolve",
        None,
        Some("<host:port:addr>"),
        Resolve,
        3,
        "Connects to `addr` for `host:port` instead of resolving the name.",
    ),
    f(
        "retry",
        None,
        Some("<num>"),
        Retry,
        3,
        "Retries transient failures (timeouts and 408, 429, 500, 502, 503, 504), waiting 1 s and doubling, or as `Retry-After` says.",
    ),
    f(
        "retry-delay",
        None,
        Some("<seconds>"),
        RetryDelay,
        3,
        "Fixed wait between `--retry` attempts; 0 keeps the doubling wait.",
    ),
    f("http1.1", None, None, Http11, 3, "Uses HTTP/1.1 only."),
    f(
        "http2",
        None,
        None,
        Http2,
        3,
        "Uses HTTP/2 when the server offers it over TLS (the default).",
    ),
    // Accepted, no effect
    f(
        "globoff",
        Some('g'),
        None,
        Ignored,
        1,
        "Accepted; no effect. Sankh never expands `[]` and `{}` in URLs.",
    ),
    f(
        "fail",
        Some('f'),
        None,
        Ignored,
        1,
        "Accepted; no effect. Use `# @expect status` to check the status.",
    ),
    f(
        "fail-with-body",
        None,
        None,
        Ignored,
        1,
        "Accepted; no effect. Use `# @expect status` to check the status.",
    ),
    f("no-buffer", Some('N'), None, Ignored, 1, NO_EFFECT),
    f("no-progress-meter", None, None, Ignored, 1, NO_EFFECT),
    f("progress-bar", Some('#'), None, Ignored, 1, NO_EFFECT),
    f(
        "basic",
        None,
        None,
        Ignored,
        1,
        "Accepted; no effect. Basic is the only authentication scheme.",
    ),
    // Not supported
    f(
        "http3",
        None,
        None,
        Unsupported,
        0,
        "HTTP/3 is not supported.",
    ),
    f(
        "http3-only",
        None,
        None,
        Unsupported,
        0,
        "HTTP/3 is not supported.",
    ),
    f(
        "http1.0",
        Some('0'),
        None,
        Unsupported,
        0,
        "HTTP/1.0 is not supported.",
    ),
    f(
        "http2-prior-knowledge",
        None,
        None,
        Unsupported,
        0,
        "Use `--http2` over HTTPS.",
    ),
    f(
        "upload-file",
        Some('T'),
        Some("<file>"),
        Unsupported,
        0,
        "Use `-X PUT --data-binary @file`.",
    ),
    f(
        "config",
        Some('K'),
        Some("<file>"),
        Unsupported,
        0,
        "Write the options in the request file.",
    ),
    f(
        "next",
        Some(':'),
        None,
        Unsupported,
        0,
        "Put each request in its own file.",
    ),
    f(
        "oauth2-bearer",
        None,
        Some("<token>"),
        Unsupported,
        0,
        "Use `-H \"Authorization: Bearer $TOKEN\"`.",
    ),
    f(
        "aws-sigv4",
        None,
        Some("<provider>"),
        Unsupported,
        0,
        "Only Basic authentication (`-u`) is supported.",
    ),
    f(
        "digest",
        None,
        None,
        Unsupported,
        0,
        "Only Basic authentication (`-u`) is supported.",
    ),
    f(
        "ntlm",
        None,
        None,
        Unsupported,
        0,
        "Only Basic authentication (`-u`) is supported.",
    ),
    f(
        "negotiate",
        None,
        None,
        Unsupported,
        0,
        "Only Basic authentication (`-u`) is supported.",
    ),
    f(
        "anyauth",
        None,
        None,
        Unsupported,
        0,
        "Only Basic authentication (`-u`) is supported.",
    ),
    f(
        "range",
        Some('r'),
        Some("<range>"),
        Unsupported,
        0,
        "Use `-H \"Range: bytes=0-99\"`.",
    ),
    f(
        "path-as-is",
        None,
        None,
        Unsupported,
        0,
        "Dot segments in URLs are always normalized.",
    ),
    f("request-target", None, Some("<path>"), Unsupported, 0, ""),
    f("unix-socket", None, Some("<path>"), Unsupported, 0, ""),
    f(
        "abstract-unix-socket",
        None,
        Some("<path>"),
        Unsupported,
        0,
        "",
    ),
    f("interface", None, Some("<name>"), Unsupported, 0, ""),
    f("local-port", None, Some("<range>"), Unsupported, 0, ""),
    f("ipv4", Some('4'), None, Unsupported, 0, ""),
    f("ipv6", Some('6'), None, Unsupported, 0, ""),
    f(
        "cert-type",
        None,
        Some("<type>"),
        Unsupported,
        0,
        "Only PEM certificates are supported.",
    ),
    f(
        "key-type",
        None,
        Some("<type>"),
        Unsupported,
        0,
        "Only PEM keys are supported.",
    ),
    f(
        "pass",
        None,
        Some("<phrase>"),
        Unsupported,
        0,
        "Encrypted private keys are not supported.",
    ),
    f(
        "capath",
        None,
        Some("<dir>"),
        Unsupported,
        0,
        "Use `--cacert` with a PEM bundle.",
    ),
    f("crlfile", None, Some("<file>"), Unsupported, 0, ""),
    f("pinnedpubkey", None, Some("<hashes>"), Unsupported, 0, ""),
    f("ciphers", None, Some("<list>"), Unsupported, 0, ""),
    f("tlsv1", Some('1'), None, Unsupported, 0, ""),
    f("tlsv1.0", None, None, Unsupported, 0, ""),
    f("tlsv1.1", None, None, Unsupported, 0, ""),
    f("tlsv1.2", None, None, Unsupported, 0, ""),
    f("tlsv1.3", None, None, Unsupported, 0, ""),
    f("tls-max", None, Some("<version>"), Unsupported, 0, ""),
    f(
        "proto",
        None,
        Some("<protocols>"),
        Unsupported,
        0,
        "Only http and https are supported.",
    ),
    f(
        "proto-redir",
        None,
        Some("<protocols>"),
        Unsupported,
        0,
        "Only http and https are supported.",
    ),
    f("limit-rate", None, Some("<speed>"), Unsupported, 0, ""),
    f(
        "max-filesize",
        None,
        Some("<bytes>"),
        Unsupported,
        0,
        "Set `max_body` in `sankh.toml`.",
    ),
    f(
        "retry-max-time",
        None,
        Some("<seconds>"),
        Unsupported,
        0,
        "",
    ),
    f("retry-connrefused", None, None, Unsupported, 0, ""),
    f("retry-all-errors", None, None, Unsupported, 0, ""),
    f("trace", None, Some("<file>"), Unsupported, 0, ""),
    f("trace-ascii", None, Some("<file>"), Unsupported, 0, ""),
    f("trace-time", None, None, Unsupported, 0, ""),
    f("stderr", None, Some("<file>"), Unsupported, 0, ""),
    f("libcurl", None, Some("<file>"), Unsupported, 0, ""),
    f("alt-svc", None, Some("<file>"), Unsupported, 0, ""),
    f("hsts", None, Some("<file>"), Unsupported, 0, ""),
    f("doh-url", None, Some("<url>"), Unsupported, 0, ""),
    f("dns-servers", None, Some("<addresses>"), Unsupported, 0, ""),
    f(
        "socks5",
        None,
        Some("<host:port>"),
        Unsupported,
        0,
        "Only HTTP and HTTPS proxies are supported.",
    ),
    f(
        "socks5-hostname",
        None,
        Some("<host:port>"),
        Unsupported,
        0,
        "Only HTTP and HTTPS proxies are supported.",
    ),
    f(
        "proxy-user",
        Some('U'),
        Some("<user:password>"),
        Unsupported,
        0,
        "Put the credentials in the proxy URL.",
    ),
    f("proxy-insecure", None, None, Unsupported, 0, ""),
    f("proxytunnel", Some('p'), None, Unsupported, 0, ""),
    f("junk-session-cookies", Some('j'), None, Unsupported, 0, ""),
    f("ignore-content-length", None, None, Unsupported, 0, ""),
    f("raw", None, None, Unsupported, 0, ""),
    f("tr-encoding", None, None, Unsupported, 0, ""),
    f("post301", None, None, Unsupported, 0, ""),
    f("post302", None, None, Unsupported, 0, ""),
    f("post303", None, None, Unsupported, 0, ""),
    f("append", Some('a'), None, Unsupported, 0, ""),
    f("list-only", Some('l'), None, Unsupported, 0, ""),
    f("quote", Some('Q'), Some("<command>"), Unsupported, 0, ""),
    f("ssl", None, None, Unsupported, 0, ""),
    f("ssl-reqd", None, None, Unsupported, 0, ""),
    f(
        "continue-at",
        Some('C'),
        Some("<offset>"),
        Unsupported,
        0,
        "",
    ),
    f(
        "time-cond",
        Some('z'),
        Some("<time>"),
        Unsupported,
        0,
        "Use `-H \"If-Modified-Since: ...\"`.",
    ),
    f(
        "parallel",
        Some('Z'),
        None,
        Unsupported,
        0,
        "Put each request in its own file.",
    ),
    f(
        "netrc",
        Some('n'),
        None,
        Unsupported,
        0,
        "Use `-u \"$USER:$PASSWORD\"` with variables.",
    ),
    f(
        "netrc-optional",
        None,
        None,
        Unsupported,
        0,
        "Use `-u \"$USER:$PASSWORD\"` with variables.",
    ),
    f(
        "netrc-file",
        None,
        Some("<file>"),
        Unsupported,
        0,
        "Use `-u \"$USER:$PASSWORD\"` with variables.",
    ),
    f(
        "variable",
        None,
        Some("<name=value>"),
        Unsupported,
        0,
        "Define variables in an environment file.",
    ),
    f(
        "expand-url",
        None,
        Some("<url>"),
        Unsupported,
        0,
        "Write `$NAME` in the URL.",
    ),
];

pub fn by_long(name: &str) -> Option<&'static Flag> {
    FLAGS.iter().find(|f| f.long == name)
}

pub fn by_short(c: char) -> Option<&'static Flag> {
    FLAGS.iter().find(|f| f.short == Some(c))
}

/// A flag as written in the command, with its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arg {
    pub flag: &'static Flag,
    /// `-H` or `--header`, as written.
    pub name: String,
    /// Where the flag's name is written.
    pub span: Span,
    /// Index in the command's words of the flag.
    pub word: usize,
    pub value: Option<Word>,
    /// Index of the value's word when it is separate from the flag.
    pub value_word: Option<usize>,
}

/// The command's flags in order, plus positional URLs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    pub words: Vec<Word>,
    pub args: Vec<Arg>,
    /// Positional arguments (URLs) with their word index.
    pub positionals: Vec<(usize, Word)>,
    /// Span of the whole command, for errors that have no better place.
    pub span: Span,
}

impl Parsed {
    pub fn has(&self, kind: Kind) -> bool {
        self.args.iter().any(|a| a.flag.kind == kind)
    }
}

/// How unknown and unsupported flags are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    /// Every unsupported or unknown flag is an error (request files).
    Strict,
    /// Unsupported and unknown flags are dropped and listed (curl import).
    Lenient,
}

/// Result of [`parse_args`]: errors are empty in lenient mode, which lists
/// what it dropped instead.
pub struct ArgsOutcome {
    pub parsed: Parsed,
    pub errors: Vec<Diag>,
    pub dropped: Vec<String>,
}

/// Splits the words after `curl` into flags and positionals.
pub fn parse_args(words: &[Word], span: Span, mode: Strictness) -> ArgsOutcome {
    let mut out = ArgsOutcome {
        parsed: Parsed {
            words: words.to_vec(),
            span,
            ..Parsed::default()
        },
        errors: Vec::new(),
        dropped: Vec::new(),
    };
    let mut i = 0;
    while i < words.len() {
        let w = &words[i];
        let prefix = w.literal_prefix();
        let is_flag = prefix.starts_with('-') && (prefix.len() > 1 || w.segs.len() > 1);
        if !is_flag {
            out.parsed.positionals.push((i, w.clone()));
            i += 1;
            continue;
        }
        if let Some(long) = prefix.strip_prefix("--") {
            i = long_flag(&mut out, words, i, long, mode);
        } else {
            i = short_flags(&mut out, words, i, mode);
        }
    }
    out
}

fn reject(out: &mut ArgsOutcome, flag: &'static Flag, written: &str, span: Span, mode: Strictness) {
    if mode == Strictness::Lenient {
        out.dropped.push(written.to_string());
        return;
    }
    let d = match flag.kind {
        Kind::Rejected => Diag::error(format!("flag {written} is not allowed"))
            .at(span)
            .label("Sankh handles the response itself")
            .help(flag.doc),
        _ => {
            let d = Diag::error(format!("flag {written} is not supported yet"))
                .at(span)
                .help("supported flags: https://sankh.dev/docs/curl-flags")
                .note("open an issue to request it");
            if flag.doc.is_empty() {
                d
            } else {
                d.label(flag.doc)
            }
        }
    };
    out.errors.push(d);
}

fn unknown(out: &mut ArgsOutcome, written: &str, span: Span, mode: Strictness) {
    if mode == Strictness::Lenient {
        out.dropped.push(written.to_string());
        return;
    }
    let mut d = Diag::error(format!("unknown flag {written}"))
        .at(span)
        .label("curl has no such option");
    d = match suggest(written) {
        Some(s) => d.help(format!("did you mean `{s}`?")),
        None => d.help("supported flags: https://sankh.dev/docs/curl-flags"),
    };
    out.errors.push(d);
}

/// The closest flag name within two edits.
fn suggest(written: &str) -> Option<String> {
    let name = written.trim_start_matches('-');
    FLAGS
        .iter()
        .filter(|f| f.supported())
        .map(|f| (edit_distance(name, f.long), f))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map(|(_, f)| format!("--{}", f.long))
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Handles `--name` at `words[i]`; returns the next index.
fn long_flag(
    out: &mut ArgsOutcome,
    words: &[Word],
    i: usize,
    long: &str,
    mode: Strictness,
) -> usize {
    let w = &words[i];
    let span = w.span;
    if w.has_vars() {
        out.errors.push(
            Diag::error("flag names cannot contain variables")
                .at(span)
                .help("write the flag literally and put the variable in its value"),
        );
        return i + 1;
    }
    let written = format!("--{long}");
    let Some(flag) = by_long(long) else {
        if let Some((name, _)) = long.split_once('=') {
            if by_long(name).is_some() {
                if mode == Strictness::Strict {
                    out.errors.push(
                        Diag::error(format!("`--{name}=...` is not curl syntax"))
                            .at(span)
                            .help(format!("write `--{name} <value>` with a space")),
                    );
                } else {
                    out.dropped.push(written);
                }
                return i + 1;
            }
        }
        unknown(out, &written, span, mode);
        return i + 1;
    };
    let mut next = i + 1;
    let (value, value_word) = if flag.takes_value() {
        match words.get(i + 1) {
            Some(v) => {
                next = i + 2;
                (Some(v.clone()), Some(i + 1))
            }
            None => {
                out.errors.push(
                    Diag::error(format!("{written} needs a value"))
                        .at(span)
                        .label(format!("expected {}", flag.value.unwrap_or("a value"))),
                );
                return next;
            }
        }
    } else {
        (None, None)
    };
    if !flag.supported() {
        reject(out, flag, &written, span, mode);
        return next;
    }
    out.parsed.args.push(Arg {
        flag,
        name: written,
        span,
        word: i,
        value,
        value_word,
    });
    next
}

/// Handles `-abc` at `words[i]`: switches may be combined, and a flag that
/// takes a value uses the rest of the word or the next word.
fn short_flags(out: &mut ArgsOutcome, words: &[Word], i: usize, mode: Strictness) -> usize {
    let w = &words[i];
    let prefix = w.literal_prefix();
    let mut offset = 1;
    for c in prefix[1..].chars() {
        let char_span = Span::new(
            (w.span.start + offset).min(w.span.end),
            (w.span.start + offset + c.len_utf8()).min(w.span.end),
        );
        let written = format!("-{c}");
        let flag_span = if offset == 1 {
            Span::new(w.span.start, char_span.end)
        } else {
            char_span
        };
        offset += c.len_utf8();
        let Some(flag) = by_short(c) else {
            unknown(out, &written, flag_span, mode);
            return i + 1;
        };
        if flag.takes_value() {
            let rest = w.tail(offset);
            let attached = !rest.segs.is_empty() || offset < prefix.len();
            let (value, value_word, next) = if attached {
                (rest, None, i + 1)
            } else if let Some(v) = words.get(i + 1) {
                (v.clone(), Some(i + 1), i + 2)
            } else {
                out.errors.push(
                    Diag::error(format!("{written} needs a value"))
                        .at(flag_span)
                        .label(format!("expected {}", flag.value.unwrap_or("a value"))),
                );
                return i + 1;
            };
            if !flag.supported() {
                reject(out, flag, &written, flag_span, mode);
                return next;
            }
            out.parsed.args.push(Arg {
                flag,
                name: written,
                span: flag_span,
                word: i,
                value: Some(value),
                value_word,
            });
            return next;
        }
        if !flag.supported() {
            reject(out, flag, &written, flag_span, mode);
            continue;
        }
        out.parsed.args.push(Arg {
            flag,
            name: written,
            span: flag_span,
            word: i,
            value: None,
            value_word: None,
        });
    }
    if w.segs.len() > 1 {
        out.errors.push(
            Diag::error("flag names cannot contain variables")
                .at(w.span)
                .help("write the flag literally and put the variable in its value"),
        );
    }
    i + 1
}

/// The flag compatibility page, generated from [`FLAGS`].
pub fn markdown() -> String {
    let mut out = String::from(
        "<!-- Generated from crates/sankh-core/src/parse/flags.rs; run `SANKH_UPDATE_DOCS=1 cargo test -p sankh-core --test flags_doc` to refresh. -->\n\n# curl flags\n\nSankh reads request files as curl commands and sends them with its own HTTP client. It never runs `curl` or a shell. A flag that is not listed as supported is an error, never silently ignored, so a request cannot behave differently from what the file says.\n\nShort switches can be combined (`-sSL`), and a value can follow a short flag directly (`-XPOST`). `--flag=value` is not curl syntax; write `--flag value`.\n",
    );
    let row = |f: &Flag| {
        let mut names = match f.short {
            Some(s) => format!("`-{s}`, `--{}`", f.long),
            None => format!("`--{}`", f.long),
        };
        if let Some(v) = f.value {
            names.push_str(&format!(" `{v}`"));
        }
        let doc = if f.doc.is_empty() { "" } else { f.doc };
        format!(
            "| {} | {} |\n",
            names.replace('|', "\\|"),
            doc.replace('|', "\\|")
        )
    };
    type Keep = fn(&Flag) -> bool;
    let sections: [(&str, Keep); 4] = [
        ("Supported", |f| f.supported() && f.kind != Kind::Ignored),
        ("Accepted with no effect", |f| f.kind == Kind::Ignored),
        ("Not allowed", |f| f.kind == Kind::Rejected),
        ("Not supported yet", |f| f.kind == Kind::Unsupported),
    ];
    for (title, keep) in sections {
        out.push_str(&format!(
            "\n## {title}\n\n| Flag | Behavior |\n| --- | --- |\n"
        ));
        for flag in FLAGS.iter().filter(|f| keep(f)) {
            out.push_str(&row(flag));
        }
    }
    out.push_str("\nAny other flag is reported as unknown. To ask for one, open an issue at <https://github.com/sankh-dev/sankh/issues>.\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::tokenize::tokenize;

    fn parse(src: &str, mode: Strictness) -> ArgsOutcome {
        let t = tokenize(src, 0).unwrap();
        parse_args(&t.words, Span::new(0, src.len()), mode)
    }

    fn names(o: &ArgsOutcome) -> Vec<String> {
        o.parsed
            .args
            .iter()
            .map(|a| match &a.value {
                Some(v) => format!("{}={}", a.flag.long, v.template()),
                None => a.flag.long.to_string(),
            })
            .collect()
    }

    #[test]
    fn table_has_no_duplicates() {
        for (i, a) in FLAGS.iter().enumerate() {
            for b in &FLAGS[i + 1..] {
                assert_ne!(a.long, b.long);
                if a.short.is_some() {
                    assert_ne!(a.short, b.short, "{} and {}", a.long, b.long);
                }
            }
        }
    }

    #[test]
    fn combined_and_attached() {
        let o = parse(
            "-sSL -XPOST -H 'A: b' -d@x.json --json $J u",
            Strictness::Strict,
        );
        assert!(o.errors.is_empty(), "{:?}", o.errors);
        assert_eq!(
            names(&o),
            vec![
                "silent",
                "show-error",
                "location",
                "request=POST",
                "header=A: b",
                "data=@x.json",
                "json=$J"
            ]
        );
        assert_eq!(o.parsed.positionals.len(), 1);
    }

    #[test]
    fn variables_are_never_flags() {
        let o = parse("$OPTS \"$X\"", Strictness::Strict);
        assert!(o.parsed.args.is_empty());
        assert_eq!(o.parsed.positionals.len(), 2);
    }

    #[test]
    fn errors_for_unsupported_unknown_and_rejected() {
        let o = parse(
            "--http3 --heder x -o out --url=x -T f -H",
            Strictness::Strict,
        );
        let msgs: Vec<&str> = o.errors.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            msgs,
            vec![
                "flag --http3 is not supported yet",
                "unknown flag --heder",
                "flag -o is not allowed",
                "`--url=...` is not curl syntax",
                "flag -T is not supported yet",
                "-H needs a value"
            ]
        );
        assert_eq!(
            o.errors[1].help.as_deref(),
            Some("did you mean `--header`?")
        );
    }

    #[test]
    fn lenient_drops() {
        let o = parse("-sS --http3 -o out --frob x -H 'A: b'", Strictness::Lenient);
        assert!(o.errors.is_empty());
        assert_eq!(o.dropped, vec!["--http3", "-o", "--frob"]);
        assert_eq!(names(&o), vec!["silent", "show-error", "header=A: b"]);
    }
}
