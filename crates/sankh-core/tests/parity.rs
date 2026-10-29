//! Sends the same commands with Sankh and with the real curl and compares
//! what the server received. Runs only with `SANKH_CURL_PARITY=1` (CI sets
//! it); needs `curl` on PATH.

mod common;

use common::{Recorded, Recorder, build, send, vars};
use sankh_core::http::Engine;
use sankh_core::parse::{expand, tokenize};
use std::process::Command;

const COMMANDS: &[&str] = &[
    "curl -sS \"$BASE/a?x=1\"",
    "curl -sS \"$BASE/f\" -d a -d 'b c'",
    "curl -sS \"$BASE/f\" --data-urlencode 'q=a b&c/é~' --data-urlencode '=x y' -d raw",
    "curl -sS \"$BASE/f\" --data-binary @body.txt",
    "curl -sS \"$BASE/f\" -d @body.txt",
    "curl -sS -G \"$BASE/s?a=1\" -d 'b=2' --data-urlencode 'c=d e'",
    "curl -sS \"$BASE/j\" --json '{\"a\":1}'",
    "curl -sS \"$BASE/j\" -d x --json y",
    "curl -sS -X PATCH \"$BASE/h\" -d x -H 'Content-Type: text/plain'",
    "curl -sS \"$BASE/h\" -u 'user:pa:ss' -e http://from.test/ -b 'a=1; b=2'",
    "curl -sS \"$BASE/h\" -H 'X-Empty;' -H 'Accept: application/xml' -H 'X-A: 1' -H 'X-A: 2'",
    "curl -sS \"$BASE/h\" -H 'Content-Type:' -d x",
    "curl -sS \"$BASE/h\" -u user -A ''",
    "curl -sS -I \"$BASE/h\"",
    "curl -sS \"$BASE/up\" -F name=val -F file=@body.txt -F 'note=<body.txt;type=text/plain' --form-string 'lit=@x;y'",
    "curl -sS \"$BASE/up\" -F 'f=@body.txt;filename=other.csv;type=text/csv'",
    "curl -sS --compressed \"$BASE/z\"",
];

/// Header lines that may differ: `User-Agent` names the client, the codings
/// curl offers depend on how it was built, and hyper writes `Host` and
/// `Content-Length` in other places (servers do not care about order).
fn comparable(r: &Recorded) -> (String, String, Vec<String>, String) {
    let boundary = r
        .header("content-type")
        .and_then(|ct| ct.split_once("boundary="))
        .map(|(_, b)| b.to_string());
    let norm = |s: &str| match &boundary {
        Some(b) => s.replace(b.as_str(), "<boundary>"),
        None => s.to_string(),
    };
    let moved =
        |k: &str| k.eq_ignore_ascii_case("host") || k.eq_ignore_ascii_case("content-length");
    let headers = r
        .headers
        .iter()
        .filter(|(k, _)| !moved(k) && !k.eq_ignore_ascii_case("user-agent"))
        .map(|(k, v)| {
            let v = if k.eq_ignore_ascii_case("accept-encoding") {
                "<codings>".to_string()
            } else {
                norm(v)
            };
            format!("{}: {v}", k.to_ascii_lowercase())
        })
        .chain(
            r.header("content-length")
                .map(|n| format!("content-length: {n}")),
        )
        .collect();
    (
        r.method.clone(),
        r.target.clone(),
        headers,
        norm(&r.body_text()),
    )
}

fn curl_argv(cmd: &str, base: &str) -> Vec<String> {
    let words = tokenize::tokenize(cmd, 0).unwrap().words;
    let v = vars(&[("BASE", base)]);
    words
        .iter()
        .skip(1)
        .map(|w| expand::expand(w, &v).unwrap().text)
        .collect()
}

#[test]
fn matches_curl() {
    if std::env::var_os("SANKH_CURL_PARITY").is_none() {
        eprintln!("skipped: set SANKH_CURL_PARITY=1 to compare with curl");
        return;
    }
    let srv = Recorder::start(|_, _| common::respond(200, &[], b"ok"));
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("body.txt"), "line1\r\nline2\n").unwrap();
    let mut failures = Vec::new();
    for cmd in COMMANDS {
        send(
            &mut Engine::default(),
            &build(cmd, &vars(&[("BASE", &srv.base)]), root.path()),
        )
        .unwrap();
        let ours = comparable(&srv.last());
        let status = Command::new("curl")
            .args(curl_argv(cmd, &srv.base))
            .current_dir(root.path())
            .stdout(std::process::Stdio::null())
            .status()
            .expect("run curl");
        assert!(status.success(), "curl failed: {cmd}");
        let theirs = comparable(&srv.last());
        if ours != theirs {
            failures.push(format!("{cmd}\n  sankh: {ours:?}\n  curl:  {theirs:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "differences from curl:\n{}",
        failures.join("\n")
    );
}
