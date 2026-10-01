//! Parses request files: the `#` header block with `@` annotations, and the
//! single `curl` command that follows it.

use crate::request::*;
use crate::shell;
use regex::Regex;
use std::sync::LazyLock;

static VAR_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Z_][A-Z0-9_]*$").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9_-]+$").unwrap());

const LATER_ANNOTATIONS: &[&str] = &[
    "require", "secret", "timeout", "retry", "skip", "delay", "depends",
];

/// Display name derived from a filename: `02-create-order.sh` -> "create order".
pub fn default_name(file_name: &str) -> String {
    let stem = file_name.strip_suffix(".sh").unwrap_or(file_name);
    let stem = strip_order_prefix(stem);
    stem.replace(['-', '_'], " ").trim().to_string()
}

/// Strips a numeric ordering prefix such as `01-` or `3_`.
pub fn strip_order_prefix(name: &str) -> &str {
    let digits = name.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        let rest = &name[digits..];
        if let Some(r) = rest.strip_prefix(['-', '_', '.', ' ']) {
            return r;
        }
    }
    name
}

/// Parses a request file. Never fails: problems become diagnostics, and
/// anything outside the strict format becomes raw mode.
pub fn parse(content: &str, file_name: &str) -> Request {
    let lines: Vec<&str> = content.lines().collect();
    let mut req = Request {
        name: String::new(),
        description: None,
        tags: Vec::new(),
        expects: Vec::new(),
        captures: Vec::new(),
        mode: Mode::Form,
        raw_reason: None,
        curl: None,
        extra_header_lines: Vec::new(),
        shebang: None,
        body: String::new(),
        diagnostics: Vec::new(),
    };

    let mut i = 0;
    if lines.first().is_some_and(|l| l.starts_with("#!")) {
        req.shebang = Some(lines[0].to_string());
        i = 1;
    }
    while i < lines.len() && lines[i].trim_start().starts_with('#') {
        parse_header_line(&mut req, lines[i], i + 1);
        i += 1;
    }
    let body_start = i;
    req.body = lines[body_start..].join("\n");
    if content.ends_with('\n') && !req.body.is_empty() {
        req.body.push('\n');
    }

    for (offset, line) in lines[body_start..].iter().enumerate() {
        if annotation_of(line).is_some() {
            req.diagnostics.push(Diagnostic {
                line: body_start + offset + 1,
                severity: Severity::Warning,
                message: "annotation after the request body is ignored".into(),
            });
        }
    }

    if req.name.is_empty() {
        req.name = default_name(file_name);
    }

    match parse_body(&req.body) {
        Ok(curl) => req.curl = Some(curl),
        Err(reason) => {
            req.mode = Mode::Raw;
            req.raw_reason = Some(reason);
        }
    }
    req
}

fn annotation_of(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim_start().strip_prefix('#')?.trim_start();
    let rest = rest.strip_prefix('@')?;
    let (name, args) = match rest.find(char::is_whitespace) {
        Some(p) => (&rest[..p], rest[p..].trim()),
        None => (rest, ""),
    };
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some((name, args))
}

fn parse_header_line(req: &mut Request, line: &str, line_no: usize) {
    let Some((name, args)) = annotation_of(line) else {
        req.extra_header_lines.push(line.to_string());
        return;
    };
    let mut err = |msg: String| {
        req.diagnostics.push(Diagnostic {
            line: line_no,
            severity: Severity::Error,
            message: msg,
        })
    };
    match name {
        "name" => {
            if args.is_empty() {
                err("`@name` needs a value".into());
            } else {
                req.name = args.to_string();
            }
        }
        "description" => req.description = Some(args.to_string()),
        "tags" => {
            for tag in args.split_whitespace() {
                if !TAG.is_match(tag) {
                    err(format!(
                        "invalid tag `{tag}`: use lowercase letters, digits, `-` and `_`"
                    ));
                } else if !req.tags.iter().any(|t| t == tag) {
                    req.tags.push(tag.to_string());
                }
            }
        }
        "capture" => match parse_capture(args) {
            Ok(c) => req.captures.push(c),
            Err(m) => err(m),
        },
        "expect" => match parse_expect(args) {
            Ok(Some(e)) => req.expects.push(e),
            Ok(None) => {
                req.extra_header_lines.push(line.to_string());
                req.diagnostics.push(Diagnostic {
                    line: line_no,
                    severity: Severity::Warning,
                    message: format!("`@expect {args}` is not supported in this version; ignored"),
                });
            }
            Err(m) => err(m),
        },
        other => {
            req.extra_header_lines.push(line.to_string());
            let message = if LATER_ANNOTATIONS.contains(&other) {
                format!("`@{other}` is not supported in this version; ignored")
            } else {
                format!("unknown annotation `@{other}`; ignored")
            };
            req.diagnostics.push(Diagnostic {
                line: line_no,
                severity: Severity::Warning,
                message,
            });
        }
    }
}

pub fn parse_capture(args: &str) -> Result<Capture, String> {
    let Some((var, source)) = args.split_once('=') else {
        return Err(format!(
            "`@capture {args}`: expected `NAME=<source>`, e.g. `@capture TOKEN=.data.token`"
        ));
    };
    let var = var.trim();
    let source = source.trim();
    if !VAR_NAME.is_match(var) {
        return Err(format!(
            "invalid capture name `{var}`: use uppercase letters, digits and `_`"
        ));
    }
    let source = if source == "status" {
        CaptureSource::Status
    } else if let Some(h) = source.strip_prefix("header ") {
        let name = h.trim();
        if name.is_empty() {
            return Err("`header` capture needs a header name".into());
        }
        CaptureSource::Header {
            name: name.to_string(),
        }
    } else if source.starts_with('.') {
        crate::jq::validate(source).map_err(|e| e.to_string())?;
        CaptureSource::Jq {
            expr: source.to_string(),
        }
    } else {
        return Err(format!(
            "capture source `{source}` must be a jq expression starting with `.`, `header <Name>` or `status`"
        ));
    };
    Ok(Capture {
        var: var.to_string(),
        source,
    })
}

/// Parses the arguments of `@expect`. `Ok(None)` means a known-but-later kind.
pub fn parse_expect(args: &str) -> Result<Option<Expect>, String> {
    let (kind, rest) = match args.find(char::is_whitespace) {
        Some(p) => (&args[..p], args[p..].trim()),
        None => (args, ""),
    };
    match kind {
        "status" => parse_status(rest).map(Some),
        "json" => parse_json_expect(rest).map(Some),
        "header" | "time" | "body" => Ok(None),
        "" => Err("`@expect` needs `status` or `json`".into()),
        other => Err(format!(
            "unknown expectation `{other}`; expected `status` or `json`"
        )),
    }
}

fn parse_status(rest: &str) -> Result<Expect, String> {
    if rest.is_empty() {
        return Err("`@expect status` needs a code, e.g. `200`, `2xx` or `200|201`".into());
    }
    let mut patterns = Vec::new();
    for part in rest.split('|').map(str::trim) {
        let lower = part.to_ascii_lowercase();
        let pattern = if let Some(c) = lower.strip_suffix("xx") {
            match c.parse::<u8>() {
                Ok(class @ 1..=5) if c.len() == 1 => StatusPattern::Class { class },
                _ => return Err(format!("invalid status class `{part}`; use 1xx to 5xx")),
            }
        } else {
            match part.parse::<u16>() {
                Ok(code @ 100..=599) => StatusPattern::Exact { code },
                _ => return Err(format!("invalid status code `{part}`")),
            }
        };
        patterns.push(pattern);
    }
    Ok(Expect::Status { patterns })
}

fn parse_json_expect(rest: &str) -> Result<Expect, String> {
    const USAGE: &str =
        "expected `@expect json <jq-expr> <operator> <value>` or `@expect json <jq-expr> exists`";
    if rest.is_empty() {
        return Err(USAGE.into());
    }
    if let Some(expr) = rest.strip_suffix("exists") {
        if expr.ends_with(char::is_whitespace) && !expr.trim().is_empty() {
            let expr = expr.trim();
            crate::jq::validate(expr).map_err(|e| e.to_string())?;
            return Ok(Expect::Json {
                expr: expr.to_string(),
                op: JsonOp::Exists,
                value: None,
            });
        }
    }

    // Scan from the right for the shortest trailing JSON literal that is
    // preceded by an operator token; jq expressions may contain operators too.
    let boundaries: Vec<usize> = rest
        .char_indices()
        .filter(|(_, c)| c.is_whitespace())
        .map(|(i, _)| i)
        .rev()
        .collect();
    for idx in boundaries {
        let rhs = rest[idx..].trim();
        if rhs.is_empty() || serde_json::from_str::<serde_json::Value>(rhs).is_err() {
            continue;
        }
        let lhs = rest[..idx].trim_end();
        let (expr, op_tok) = match lhs.rfind(char::is_whitespace) {
            Some(p) => (lhs[..p].trim(), lhs[p..].trim_start()),
            None => continue,
        };
        if op_tok == "=" {
            return Err("unknown operator `=`, did you mean `==`?".into());
        }
        let Some(op) = JsonOp::from_symbol(op_tok) else {
            continue;
        };
        if op == JsonOp::Exists || expr.is_empty() {
            continue;
        }
        crate::jq::validate(expr).map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_str(rhs).unwrap();
        match op {
            JsonOp::Gt | JsonOp::Ge | JsonOp::Lt | JsonOp::Le if !value.is_number() => {
                return Err(format!(
                    "operator `{}` needs a number, got `{rhs}`",
                    op.symbol()
                ));
            }
            JsonOp::Matches => {
                let Some(pattern) = value.as_str() else {
                    return Err(format!("`matches` needs a quoted regex, got `{rhs}`"));
                };
                if !pattern.contains('$') || pattern.ends_with('$') {
                    Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
                }
            }
            _ => {}
        }
        return Ok(Expect::Json {
            expr: expr.to_string(),
            op,
            value: Some(rhs.to_string()),
        });
    }

    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.len() >= 3 {
        let op_tok = tokens[tokens.len() - 2];
        let last = tokens[tokens.len() - 1];
        if op_tok == "=" {
            return Err("unknown operator `=`, did you mean `==`?".into());
        }
        if JsonOp::from_symbol(op_tok).is_some() {
            return Err(format!(
                "value `{last}` must be a JSON literal (\"text\", 42, true, null); did you mean \"{last}\"?"
            ));
        }
    }
    Err(USAGE.into())
}

/// Checks the request body is exactly one plain `curl` command and parses it.
fn parse_body(body: &str) -> Result<CurlCommand, String> {
    let joined = shell::join_continuations(body);
    let cmd = joined.trim();
    if cmd.is_empty() {
        return Err("no curl command found".into());
    }
    if let Some(feature) = shell::find_shell_feature(cmd) {
        return Err(format!("the body uses {feature}"));
    }
    parse_curl(cmd)
}

/// Parses a single `curl ...` command line (e.g. pasted from a browser).
pub fn parse_curl(cmd: &str) -> Result<CurlCommand, String> {
    let joined = shell::join_continuations(cmd);
    let words = shell::split_words(joined.trim())?;
    let mut it = words.into_iter();
    match it.next().as_deref() {
        Some("curl") => {}
        _ => return Err("the body is not a curl command".into()),
    }
    let mut c = CurlCommand::default();
    let mut explicit_method = None;
    let next_val = |it: &mut std::vec::IntoIter<String>, flag: &str| {
        it.next().ok_or_else(|| format!("`{flag}` needs a value"))
    };
    while let Some(w) = it.next() {
        match w.as_str() {
            "-X" | "--request" => explicit_method = Some(next_val(&mut it, &w)?),
            "-H" | "--header" => {
                let h = next_val(&mut it, &w)?;
                let Some((name, value)) = h.split_once(':') else {
                    return Err(format!("header `{h}` has no `:`"));
                };
                c.headers.push(Header {
                    name: name.trim().to_string(),
                    value: value.trim_start().to_string(),
                });
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii" | "--json" => {
                if c.body.is_some() {
                    return Err("more than one request body".into());
                }
                let v = next_val(&mut it, &w)?;
                c.body = Some(v);
                if w != "-d" && w != "--data" {
                    c.body_flag = Some(w.clone());
                }
            }
            "--url" => {
                let v = next_val(&mut it, &w)?;
                set_url(&mut c, v)?;
            }
            s if s.starts_with("-X") && s.len() > 2 && !s.starts_with("--") => {
                explicit_method = Some(s[2..].to_string())
            }
            s if takes_value(s) => {
                let v = next_val(&mut it, s)?;
                c.flags.push(w.clone());
                c.flags.push(v);
            }
            s if s.starts_with('-') && s.len() > 1 => c.flags.push(w.clone()),
            _ => set_url(&mut c, w)?,
        }
    }
    if c.url.is_empty() {
        return Err("curl command has no URL".into());
    }
    c.method = explicit_method
        .map(|m| m.to_ascii_uppercase())
        .unwrap_or_else(|| if c.body.is_some() { "POST" } else { "GET" }.into());
    Ok(c)
}

fn set_url(c: &mut CurlCommand, url: String) -> Result<(), String> {
    if !c.url.is_empty() {
        return Err("curl command has more than one URL".into());
    }
    c.url = url;
    Ok(())
}

fn takes_value(flag: &str) -> bool {
    matches!(
        flag,
        "-u" | "--user"
            | "-A"
            | "--user-agent"
            | "-b"
            | "--cookie"
            | "-c"
            | "--cookie-jar"
            | "-e"
            | "--referer"
            | "-m"
            | "--max-time"
            | "--connect-timeout"
            | "-x"
            | "--proxy"
            | "--cacert"
            | "--cert"
            | "--key"
            | "-E"
            | "-F"
            | "--form"
            | "--form-string"
            | "--resolve"
            | "--retry"
            | "-w"
            | "--write-out"
            | "-o"
            | "--output"
            | "-D"
            | "--dump-header"
            | "-T"
            | "--upload-file"
            | "--data-urlencode"
            | "--oauth2-bearer"
            | "-r"
            | "--range"
    )
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
    fn default_names() {
        assert_eq!(default_name("02-create-order.sh"), "create order");
        assert_eq!(default_name("list.sh"), "list");
        assert_eq!(default_name("10_get_user.sh"), "get user");
    }

    #[test]
    fn json_expect_forms() {
        let ok = [
            (
                r#".data.name == "test""#,
                ".data.name",
                JsonOp::Eq,
                Some(r#""test""#),
            ),
            (".data.qty >= 1", ".data.qty", JsonOp::Ge, Some("1")),
            (
                ".items | length > 0",
                ".items | length",
                JsonOp::Gt,
                Some("0"),
            ),
            (".data.id exists", ".data.id", JsonOp::Exists, None),
            (
                r#".data.email matches "^.+@.+$""#,
                ".data.email",
                JsonOp::Matches,
                Some(r#""^.+@.+$""#),
            ),
            (
                r#".data.tags contains "new""#,
                ".data.tags",
                JsonOp::Contains,
                Some(r#""new""#),
            ),
            (
                ".data.deleted_at == null",
                ".data.deleted_at",
                JsonOp::Eq,
                Some("null"),
            ),
            (r#".a == "x == y""#, ".a", JsonOp::Eq, Some(r#""x == y""#)),
            (
                r#".a == (.b == 1) != false"#,
                ".a == (.b == 1)",
                JsonOp::Ne,
                Some("false"),
            ),
            (
                r#".owner == "$USER_ID""#,
                ".owner",
                JsonOp::Eq,
                Some(r#""$USER_ID""#),
            ),
        ];
        for (src, expr, op, value) in ok {
            let e = parse_json_expect(src).unwrap_or_else(|e| panic!("{src}: {e}"));
            assert_eq!(
                e,
                Expect::Json {
                    expr: expr.into(),
                    op,
                    value: value.map(String::from)
                },
                "{src}"
            );
        }
    }

    #[test]
    fn json_expect_errors() {
        let e = parse_json_expect(".a = 1").unwrap_err();
        assert!(e.contains("did you mean `==`"), "{e}");
        let e = parse_json_expect(".a == test").unwrap_err();
        assert!(e.contains("did you mean \"test\""), "{e}");
        let e = parse_json_expect(".a > \"x\"").unwrap_err();
        assert!(e.contains("needs a number"), "{e}");
    }

    #[test]
    fn status_forms() {
        assert!(parse_status("2xx").is_ok());
        assert!(parse_status("200|201|204").is_ok());
        assert!(parse_status("6xx").is_err());
        assert!(parse_status("abc").is_err());
    }

    #[test]
    fn raw_mode_for_pipes() {
        let r = parse("# @name x\ncurl -sS \"$U\" | jq .\n", "x.sh");
        assert_eq!(r.mode, Mode::Raw);
        assert!(r.raw_reason.unwrap().contains("pipe"));
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
    fn errors_for_malformed_known_annotations() {
        let r = parse(
            "# @capture token=.a\n# @expect status 999\ncurl x\n",
            "x.sh",
        );
        assert_eq!(r.diagnostics.len(), 2);
        assert!(r.has_errors());
        assert_eq!(r.diagnostics[1].line, 2);
    }

    #[test]
    fn parses_browser_curl() {
        let c = parse_curl(
            "curl 'https://api.example.com/x?a=1' \\\n  -H 'accept: application/json' \\\n  --data-raw '{\"a\":1}' --compressed",
        )
        .unwrap();
        assert_eq!(c.method, "POST");
        assert_eq!(c.url, "https://api.example.com/x?a=1");
        assert_eq!(c.headers.len(), 1);
        assert_eq!(c.body_flag.as_deref(), Some("--data-raw"));
        assert_eq!(c.flags, vec!["--compressed"]);
    }
}
