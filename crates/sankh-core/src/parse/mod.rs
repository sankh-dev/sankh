//! Parses request files: the `#` header block with `@` annotations, and the
//! single `curl` command that follows it.
//!
//! The pipeline is pure up to building the request: extract the command,
//! tokenize it, parse flags, then (at run time) expand variables and build a
//! [`spec::RequestSpec`]. Nothing in a request file is ever executed.

#![allow(clippy::result_large_err)]

pub mod expand;
pub mod extract;
pub mod flags;
pub mod header;
pub mod spec;
pub mod tokenize;

pub use header::{
    default_name, format_duration, parse_capture, parse_duration, parse_expect, strip_order_prefix,
};

use crate::diag::{Diag, Span};
use crate::request::*;
use flags::{Kind, Parsed, Strictness};

/// Parses a request file. Never fails: problems become diagnostics. A file
/// whose command cannot be read at all is in raw mode (edited as text).
pub fn parse(content: &str, file_name: &str) -> Request {
    let mut req = Request {
        name: String::new(),
        description: None,
        tags: Vec::new(),
        timeout_ms: None,
        expects: Vec::new(),
        captures: Vec::new(),
        mode: Mode::Form,
        raw_reason: None,
        curl: None,
        extra_header_lines: Vec::new(),
        shebang: None,
        body: String::new(),
        diagnostics: Vec::new(),
        command: None,
    };

    let mut offset = 0;
    let mut line_no = 0;
    let mut lines = content.split_inclusive('\n').peekable();
    if let Some(first) = lines.peek().filter(|l| l.starts_with("#!")) {
        req.shebang = Some(first.trim_end_matches(['\n', '\r']).to_string());
        offset += first.len();
        line_no += 1;
        lines.next();
    }
    while let Some(line) = lines.peek().filter(|l| l.trim_start().starts_with('#')) {
        line_no += 1;
        header::parse_header_line(&mut req, line.trim_end_matches(['\n', '\r']), line_no);
        offset += line.len();
        lines.next();
    }
    let body_start = offset;
    req.body = content[body_start..].to_string();
    for (i, line) in content[body_start..].lines().enumerate() {
        if header::annotation_of(line).is_some() {
            req.diagnostics.push(Diagnostic::line(
                line_no + i + 1,
                Severity::Warning,
                "annotation after the request body is ignored",
            ));
        }
    }

    if req.name.is_empty() {
        req.name = default_name(file_name);
    }

    let cmd = match extract::extract(content, body_start) {
        Ok(cmd) => cmd,
        Err(d) => {
            req.mode = Mode::Raw;
            req.raw_reason = Some(match &d.label {
                Some(l) => format!("{}: {l}", d.message),
                None => d.message.clone(),
            });
            req.diagnostics.push(d.locate(content));
            return req;
        }
    };
    let out = flags::parse_args(&cmd.words, cmd.span, Strictness::Strict);
    req.curl = Some(to_curl_command(&out.parsed));
    if out.errors.is_empty() {
        let ctx = spec::BuildCtx {
            vars: &expand::Unresolved,
            root: None,
        };
        match spec::build(&out.parsed, &ctx) {
            Ok(built) => push_all(&mut req, &built.warnings, content),
            Err(errors) => push_all(&mut req, &errors, content),
        }
    } else {
        push_all(&mut req, &out.errors, content);
    }
    req.command = Some(out.parsed);
    req
}

fn push_all(req: &mut Request, diags: &[Diag], content: &str) {
    req.diagnostics
        .extend(diags.iter().map(|d| d.locate(content)));
}

fn is_body(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Data | Kind::DataRaw | Kind::DataBinary | Kind::Json
    )
}

/// The form view's model of a parsed command. Values keep variables as
/// `$VAR` and literal dollars as `\$`.
pub fn to_curl_command(p: &Parsed) -> CurlCommand {
    let mut c = CurlCommand::default();
    let single_body = p.args.iter().filter(|a| is_body(a.flag.kind)).count() == 1;
    let mut method = None;
    let mut pushed: Vec<usize> = Vec::new();
    let mut verbatim = |c: &mut CurlCommand, a: &flags::Arg| {
        if !pushed.contains(&a.word) {
            pushed.push(a.word);
            c.flags.push(p.words[a.word].template());
        }
        if let Some(v) = a.value_word {
            c.flags.push(p.words[v].template());
        }
    };
    for (_, w) in &p.positionals {
        if c.url.is_empty() {
            c.url = w.template();
        }
    }
    for a in &p.args {
        let value = a.value.as_ref().map(|w| w.template());
        match (a.flag.kind, value) {
            (Kind::Request, Some(v)) => method = Some(v),
            (Kind::Url, Some(v)) if c.url.is_empty() => c.url = v,
            (Kind::Header, Some(v)) => match v.split_once(':') {
                Some((name, value)) if !value.trim().is_empty() && !name.contains(';') => {
                    c.headers.push(Header {
                        name: name.trim().to_string(),
                        value: value.trim_start().to_string(),
                    })
                }
                _ => verbatim(&mut c, a),
            },
            (kind, Some(v)) if single_body && is_body(kind) => {
                c.body = Some(v);
                c.body_flag = (a.flag.long != "data").then(|| format!("--{}", a.flag.long));
            }
            _ => verbatim(&mut c, a),
        }
    }
    c.method = match method {
        Some(m) => m.to_ascii_uppercase(),
        None => c.implied_method().to_string(),
    };
    c
}

/// Parses a pasted curl command for import. Unsupported and unknown flags
/// are dropped, as is shell syntax after the command (`| jq .`); the
/// returned notes say what was left out.
pub fn parse_curl(cmd: &str) -> Result<(CurlCommand, Vec<String>), String> {
    let text = cmd.trim();
    let mut notes = Vec::new();
    let mut start = 0;
    if text.starts_with("$ ") {
        start = 2;
    }
    let tokens = match tokenize::tokenize(text, start) {
        Ok(t) => t,
        Err(d) if d.message == "shell syntax is not supported" => {
            let at = d.span.map_or(text.len(), |s| s.start);
            let rest = text[at..].lines().next().unwrap_or("").trim();
            notes.push(format!("{rest} ({})", d.label.unwrap_or_default()));
            tokenize::tokenize(&text[..at], start).map_err(|d| d.message)?
        }
        Err(d) => return Err(d.message),
    };
    if notes.is_empty() && !text[tokens.end.min(text.len())..].trim().is_empty() {
        notes.push("everything after the first command".into());
    }
    let mut words = tokens.words;
    if words.first().and_then(|w| w.literal()).as_deref() != Some("curl") {
        return Err("the text is not a curl command".into());
    }
    let curl = words.remove(0).span;
    let out = flags::parse_args(
        &words,
        Span::new(curl.start, tokens.end),
        Strictness::Lenient,
    );
    for flag in &out.dropped {
        notes.push(format!("{flag} (not supported)"));
    }
    let c = to_curl_command(&out.parsed);
    if c.url.is_empty() {
        return Err("curl command has no URL".into());
    }
    Ok((c, notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"#!/usr/bin/env bash
# @name Create user
# @tags smoke users
# @capture USER_ID=.data.id
# @expect status 201
# @expect json .data.name == "test"
curl -sS -X POST "$BASE_URL/users" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"test"}'
"#;

    #[test]
    fn parses_sample() {
        let r = parse(SAMPLE, "02-create.sh");
        insta::assert_yaml_snapshot!(r);
    }

    #[test]
    fn shell_syntax_is_an_error() {
        let r = parse("# @name x\ncurl -sS \"$U\" | jq .\n", "x.sh");
        assert_eq!(r.mode, Mode::Raw);
        assert!(r.raw_reason.as_deref().unwrap().contains("pipe"));
        let d = &r.diagnostics[0];
        assert_eq!((d.line, d.column), (2, Some(15)));
        assert!(r.has_errors());
    }

    #[test]
    fn unsupported_flags_are_errors_with_positions() {
        let r = parse("curl -sS \\\n  --http3 \\\n  \"$U\"\n", "x.sh");
        assert_eq!(r.mode, Mode::Form);
        let d = &r.diagnostics[0];
        assert_eq!(d.message, "flag --http3 is not supported yet");
        assert_eq!((d.line, d.column, d.width), (2, Some(3), Some(7)));
    }

    #[test]
    fn unknown_annotations_warn_and_are_preserved() {
        let r = parse(
            "# @retry 3 every 1s\n# @frobnicate\n# plain comment\ncurl x\n",
            "x.sh",
        );
        assert_eq!(r.extra_header_lines.len(), 3);
        assert!(
            r.diagnostics
                .iter()
                .all(|d| d.severity == Severity::Warning)
        );
        assert_eq!(r.diagnostics.len(), 2);
    }

    #[test]
    fn joins_description_lines() {
        let r = parse(
            "# @description Creates a user.\n# @description\n# @description Needs $TOKEN.\ncurl x\n",
            "x.sh",
        );
        assert_eq!(
            r.description.as_deref(),
            Some("Creates a user.\n\nNeeds $TOKEN.")
        );
    }

    #[test]
    fn timeouts_and_annotation_errors() {
        let r = parse("# @timeout 3s\ncurl x\n", "x.sh");
        assert_eq!(r.timeout_ms, Some(3000));
        assert!(r.diagnostics.is_empty());
        let r = parse("# @timeout forever\ncurl x\n", "x.sh");
        assert!(r.has_errors());
        let r = parse(
            "# @capture token=.a\n# @expect status 999\ncurl x\n",
            "x.sh",
        );
        assert_eq!(r.diagnostics.len(), 2);
        assert!(r.has_errors());
        assert_eq!(r.diagnostics[1].line, 2);
    }

    #[test]
    fn several_data_flags_stay_flags() {
        let r = parse("curl x -d a -d b\n", "x.sh");
        let c = r.curl.unwrap();
        assert_eq!(c.body, None);
        assert_eq!(c.flags, vec!["-d", "a", "-d", "b"]);
        assert_eq!(c.method, "POST");
    }

    #[test]
    fn imports_browser_curl() {
        let (c, notes) = parse_curl(
            "curl 'https://api.example.com/x?a=1' \\\n  -H 'accept: application/json' \\\n  --data-raw $'{\"a\":\"it\\'s\"}' --compressed --http3 | jq .",
        )
        .unwrap();
        assert_eq!(c.method, "POST");
        assert_eq!(c.url, "https://api.example.com/x?a=1");
        assert_eq!(c.headers.len(), 1);
        assert_eq!(c.body.as_deref(), Some("{\"a\":\"it's\"}"));
        assert_eq!(c.body_flag.as_deref(), Some("--data-raw"));
        assert_eq!(c.flags, vec!["--compressed"]);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!(notes[0].starts_with("| jq . ("));
        assert!(notes[1].contains("--http3"));
    }
}
