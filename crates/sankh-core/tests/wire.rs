//! What goes over the wire for each supported flag, checked byte for byte
//! against a recording server. Expected values follow curl 8.

mod common;

use base64::Engine as _;
use common::{Recorder, build, respond, run, send, vars};
use sankh_core::http::{Engine, HostPolicy, SendError};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn header_names(r: &common::Recorded) -> Vec<&str> {
    r.headers.iter().map(|(k, _)| k.as_str()).collect()
}

#[test]
fn plain_get_sends_curls_default_headers() {
    let srv = Recorder::ok();
    let resp = run("curl -sS \"$BASE/a?x=1#frag\"", &srv.base);
    assert_eq!(resp.status, 200);
    assert_eq!(resp.version, "HTTP/1.1");
    let r = srv.last();
    assert_eq!((r.method.as_str(), r.target.as_str()), ("GET", "/a?x=1"));
    // curl sends Host first; hyper sends it last. Servers do not care.
    let mut names = header_names(&r);
    names.sort_unstable();
    assert_eq!(names, ["Accept", "Host", "User-Agent"]);
    assert_eq!(r.header("host").unwrap(), format!("127.0.0.1:{}", srv.port));
    assert_eq!(
        r.header("user-agent").unwrap(),
        format!("sankh/{}", sankh_core::version())
    );
    assert_eq!(r.header("accept"), Some("*/*"));
    assert!(r.body.is_empty());
}

#[test]
fn data_flags_join_like_curl() {
    let srv = Recorder::ok();
    run("curl -sS \"$BASE/f\" -d a -d 'b c'", &srv.base);
    let r = srv.last();
    assert_eq!(r.method, "POST");
    assert_eq!(r.body_text(), "a&b c");
    assert_eq!(
        r.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(r.header("content-length"), Some("5"));

    run(
        "curl -sS \"$BASE/f\" --data-urlencode 'q=a b&c/é~' --data-urlencode '=x y' -d raw",
        &srv.base,
    );
    assert_eq!(srv.last().body_text(), "q=a+b%26c%2F%C3%A9~&x+y&raw");

    run("curl -sS \"$BASE/f\" -d x --json y", &srv.base);
    assert_eq!(srv.last().body_text(), "xy");
    run("curl -sS \"$BASE/f\" --json y -d x", &srv.base);
    let r = srv.last();
    assert_eq!(r.body_text(), "y&x");
    assert_eq!(r.header("content-type"), Some("application/json"));
    assert_eq!(r.header("accept"), Some("application/json"));
}

#[test]
fn get_moves_data_into_the_query() {
    let srv = Recorder::ok();
    run(
        "curl -sS -G \"$BASE/s?a=1#top\" -d 'b=2' --data-urlencode 'c=d e'",
        &srv.base,
    );
    let r = srv.last();
    assert_eq!(
        (r.method.as_str(), r.target.as_str()),
        ("GET", "/s?a=1&b=2&c=d+e")
    );
    assert!(r.body.is_empty());
    assert_eq!(r.header("content-type"), None);

    run("curl -sS -G \"$BASE/s\" -d 'b=2'", &srv.base);
    assert_eq!(srv.last().target, "/s?b=2");
}

#[test]
fn files_are_read_from_the_collection() {
    let srv = Recorder::ok();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("body.txt"), "line1\r\nline2\n").unwrap();
    let v = vars(&[("BASE", &srv.base)]);
    let mut engine = Engine::default();

    send(
        &mut engine,
        &build("curl -sS \"$BASE/u\" -d @body.txt", &v, root.path()),
    )
    .unwrap();
    assert_eq!(srv.last().body_text(), "line1line2");
    send(
        &mut engine,
        &build(
            "curl -sS \"$BASE/u\" --data-binary @body.txt",
            &v,
            root.path(),
        ),
    )
    .unwrap();
    assert_eq!(srv.last().body_text(), "line1\r\nline2\n");
    send(
        &mut engine,
        &build("curl -sS \"$BASE/u\" --data-raw @body.txt", &v, root.path()),
    )
    .unwrap();
    assert_eq!(srv.last().body_text(), "@body.txt");
}

#[test]
fn multipart_forms_match_curl() {
    let srv = Recorder::ok();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("up.txt"), "hello").unwrap();
    std::fs::write(root.path().join("blob.bin"), [0u8, 1, 2]).unwrap();
    std::fs::write(root.path().join("note.txt"), "from file").unwrap();
    let v = vars(&[("BASE", &srv.base)]);
    let spec = build(
        "curl -sS \"$BASE/up\" -F name=val -F file=@up.txt -F raw=@blob.bin -F 'note=<note.txt' --form-string 'lit=@x;y'",
        &v,
        root.path(),
    );
    send(&mut Engine::default(), &spec).unwrap();
    let r = srv.last();
    assert_eq!(r.method, "POST");
    let ct = r.header("content-type").unwrap();
    let boundary = ct.strip_prefix("multipart/form-data; boundary=").expect(ct);
    assert_eq!(boundary.len(), 46);
    assert!(boundary.starts_with(&"-".repeat(24)));
    assert!(boundary[24..].chars().all(|c| c.is_ascii_alphanumeric()));
    let want = [
        ("name=\"name\"", "", "val"),
        (
            "name=\"file\"; filename=\"up.txt\"",
            "Content-Type: text/plain\r\n",
            "hello",
        ),
        (
            "name=\"raw\"; filename=\"blob.bin\"",
            "Content-Type: application/octet-stream\r\n",
            "\u{0}\u{1}\u{2}",
        ),
        ("name=\"note\"", "", "from file"),
        ("name=\"lit\"", "", "@x;y"),
    ];
    let mut expected = String::new();
    for (disp, ct, content) in want {
        expected.push_str(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; {disp}\r\n{ct}\r\n{content}\r\n"
        ));
    }
    expected.push_str(&format!("--{boundary}--\r\n"));
    assert_eq!(r.body_text(), expected);
}

#[test]
fn auth_headers_cookies_and_agent() {
    let srv = Recorder::ok();
    run(
        "curl -sS \"$BASE/h\" -u 'user:pa:ss' -A 'my-agent/1' -e http://from.test/ -b 'a=1; b=2' -H 'X-Empty;' -H 'Accept: application/xml' -H 'X-A: 1' -H 'X-A: 2'",
        &srv.base,
    );
    let r = srv.last();
    let basic = base64::engine::general_purpose::STANDARD.encode("user:pa:ss");
    assert_eq!(r.header("authorization").unwrap(), format!("Basic {basic}"));
    assert_eq!(r.header("user-agent"), Some("my-agent/1"));
    assert_eq!(r.header("referer"), Some("http://from.test/"));
    assert_eq!(r.header("cookie"), Some("a=1; b=2"));
    assert_eq!(r.header("x-empty"), Some(""));
    assert_eq!(r.header("accept"), Some("application/xml"));
    let accepts = r
        .headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case("accept"))
        .count();
    assert_eq!(accepts, 1);
    let xa: Vec<&str> = r
        .headers
        .iter()
        .filter(|(k, _)| k == "X-A")
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(xa, ["1", "2"]);

    run("curl -sS \"$BASE/h\" -u user -A ''", &srv.base);
    let r = srv.last();
    let basic = base64::engine::general_purpose::STANDARD.encode("user:");
    assert_eq!(r.header("authorization").unwrap(), format!("Basic {basic}"));
    assert_eq!(r.header("user-agent"), None);

    run("curl -sS \"$BASE/h\" -H 'Content-Type:' -d x", &srv.base);
    assert_eq!(srv.last().header("content-type"), None);
}

#[test]
fn methods() {
    let srv = Recorder::ok();
    run("curl -sS -I \"$BASE/h\"", &srv.base);
    assert_eq!(srv.last().method, "HEAD");
    run("curl -sS -X PATCH \"$BASE/h\" -d x", &srv.base);
    let r = srv.last();
    assert_eq!((r.method.as_str(), r.body_text().as_str()), ("PATCH", "x"));
    run("curl -sS -XDELETE \"$BASE/h\"", &srv.base);
    assert_eq!(srv.last().method, "DELETE");
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// `data` gzip-encoded with a single stored (uncompressed) block.
fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff, 1];
    let len = data.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(data).to_le_bytes());
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out
}

#[test]
fn compressed_decodes_only_when_asked() {
    let srv = Recorder::start(|_, _| {
        respond(
            200,
            &[("Content-Encoding", "gzip")],
            &gzip(b"{\"ok\":true}"),
        )
    });
    let resp = run("curl -sS --compressed \"$BASE/z\"", &srv.base);
    assert_eq!(resp.body, b"{\"ok\":true}");
    let ae = srv.last().header("accept-encoding").unwrap().to_string();
    assert!(ae.contains("gzip") && ae.contains("br"), "{ae}");

    let resp = run("curl -sS \"$BASE/z\"", &srv.base);
    assert_eq!(resp.body, gzip(b"{\"ok\":true}"));
    assert_eq!(srv.last().header("accept-encoding"), None);
}

fn redirecting() -> Recorder {
    Recorder::start(|r, _| {
        let path = r.target.split('?').next().unwrap_or_default();
        match path.strip_prefix("/r/") {
            Some(code) => {
                let code: u16 = code.parse().unwrap();
                respond(code, &[("Location", "/done")], b"")
            }
            None if path == "/loop" => respond(302, &[("Location", "/loop")], b""),
            None => respond(200, &[], r.method.as_bytes()),
        }
    })
}

#[test]
fn redirect_method_matrix() {
    let srv = redirecting();
    for (code, method, body) in [
        (301, "GET", ""),
        (302, "GET", ""),
        (303, "GET", ""),
        (307, "POST", "x=1"),
        (308, "POST", "x=1"),
    ] {
        let resp = run(
            &format!("curl -sS -L \"$BASE/r/{code}\" -d x=1 -H 'Authorization: Bearer t'"),
            &srv.base,
        );
        assert_eq!(resp.status, 200, "{code}");
        assert_eq!(resp.body, method.as_bytes(), "{code}");
        assert_eq!(resp.redirects.len(), 1);
        assert_eq!(resp.redirects[0].status, code);
        assert!(resp.url.ends_with("/done"));
        let r = srv.last();
        assert_eq!(r.body_text(), body, "{code}");
        assert_eq!(
            r.header("content-type").is_some(),
            !body.is_empty(),
            "{code}"
        );
        assert_eq!(
            r.header("authorization"),
            Some("Bearer t"),
            "same host keeps credentials"
        );
    }
}

#[test]
fn redirects_need_l_and_stop_at_the_limit() {
    let srv = redirecting();
    let resp = run("curl -sS \"$BASE/r/302\"", &srv.base);
    assert_eq!(resp.status, 302);
    assert_eq!(srv.requests().len(), 1);

    let root = tempfile::tempdir().unwrap();
    let spec = build(
        "curl -sS -L --max-redirs 2 \"$BASE/loop\"",
        &vars(&[("BASE", &srv.base)]),
        root.path(),
    );
    match send(&mut Engine::default(), &spec) {
        Err(SendError::Transport(m)) => assert!(m.contains("maximum (2) redirects"), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn credentials_do_not_follow_to_another_host() {
    let target = Recorder::ok();
    let other = format!("http://localhost:{}/landing", target.port);
    let hop = Recorder::start(move |_, _| respond(302, &[("Location", &other)], b""));
    let cmd = "curl -sS -L \"$BASE/start\" -u u:p -H 'Cookie: s=1' -H 'X-Keep: 1'";
    run(cmd, &hop.base);
    let r = target.last();
    assert_eq!(r.header("authorization"), None);
    assert_eq!(r.header("cookie"), None);
    assert_eq!(r.header("x-keep"), Some("1"));

    run(&cmd.replace("-L", "--location-trusted"), &hop.base);
    let r = target.last();
    assert!(r.header("authorization").is_some());
    assert_eq!(r.header("cookie"), Some("s=1"));
}

#[test]
fn retries_transient_statuses() {
    let srv = Recorder::start(|_, i| {
        if i == 0 {
            respond(503, &[], b"busy")
        } else {
            respond(200, &[], b"ok")
        }
    });
    let resp = run("curl -sS --retry 2 --retry-delay 0 \"$BASE/x\"", &srv.base);
    assert_eq!(resp.status, 200);
    assert_eq!(srv.requests().len(), 2);
    assert_eq!(resp.notes, ["retried 1 time(s)"]);

    let srv = Recorder::start(|_, _| respond(503, &[], b"busy"));
    let resp = run("curl -sS \"$BASE/x\"", &srv.base);
    assert_eq!(resp.status, 503);
    assert_eq!(srv.requests().len(), 1);
}

#[test]
fn body_is_capped() {
    let srv = Recorder::start(|_, _| respond(200, &[], &[b'x'; 100]));
    let root = tempfile::tempdir().unwrap();
    let spec = build(
        "curl -sS \"$BASE/big\"",
        &vars(&[("BASE", &srv.base)]),
        root.path(),
    );
    let mut engine = Engine::default();
    engine.max_body = 10;
    let resp = send(&mut engine, &spec).unwrap();
    assert_eq!(resp.body.len(), 10);
    assert!(resp.body_truncated);
}

fn slow() -> Recorder {
    Recorder::start(|_, _| {
        std::thread::sleep(Duration::from_secs(3));
        respond(200, &[], b"late")
    })
}

#[test]
fn timeouts_and_cancellation() {
    let srv = slow();
    let root = tempfile::tempdir().unwrap();
    let spec = build(
        "curl -sS -m 0.3 \"$BASE/slow\"",
        &vars(&[("BASE", &srv.base)]),
        root.path(),
    );
    let t = Instant::now();
    let timeout = spec.options.timeout.unwrap();
    let err = Engine::default()
        .send(&spec, timeout, &AtomicBool::new(false))
        .unwrap_err();
    assert_eq!(err, SendError::Timeout(Duration::from_millis(300)));
    assert!(t.elapsed() < Duration::from_secs(2));

    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        flag.store(true, Ordering::Relaxed);
    });
    let t = Instant::now();
    let err = Engine::default()
        .send(&spec, Duration::from_secs(10), &cancel)
        .unwrap_err();
    assert_eq!(err, SendError::Cancelled);
    assert!(t.elapsed() < Duration::from_secs(2));
}

#[test]
fn transport_errors_are_specific() {
    let root = tempfile::tempdir().unwrap();
    let v = vars(&[]);
    let spec = build("curl -sS http://127.0.0.1:9/", &v, root.path());
    match send(&mut Engine::default(), &spec) {
        Err(SendError::Transport(m)) => {
            assert!(m.starts_with("could not connect to 127.0.0.1:9"), "{m}")
        }
        other => panic!("{other:?}"),
    }
    let spec = build("curl -sS http://does-not-exist.invalid/", &v, root.path());
    match send(&mut Engine::default(), &spec) {
        Err(SendError::Transport(m)) => {
            assert_eq!(m, "could not resolve host `does-not-exist.invalid`")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn host_policy_blocks_before_connecting() {
    let srv = Recorder::ok();
    let root = tempfile::tempdir().unwrap();
    let spec = build(
        "curl -sS \"$BASE/x\"",
        &vars(&[("BASE", &srv.base)]),
        root.path(),
    );
    let mut engine = Engine::default();
    engine.policy = Some(Arc::new(HostPolicy {
        allow: vec![],
        deny: vec!["127.0.0.0/8".into()],
    }));
    match send(&mut engine, &spec) {
        Err(SendError::Config(m)) => assert!(m.contains("blocked"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(srv.requests().is_empty());
}

#[test]
fn cookie_jar_carries_cookies_through_a_run() {
    let srv = Recorder::start(|r, _| {
        if r.target == "/login" {
            respond(200, &[("Set-Cookie", "sid=abc; Path=/; HttpOnly")], b"")
        } else {
            respond(200, &[], b"")
        }
    });
    let root = tempfile::tempdir().unwrap();
    let v = vars(&[("BASE", &srv.base)]);
    let mut engine = Engine::default();
    send(
        &mut engine,
        &build("curl -sS -c jar.txt \"$BASE/login\"", &v, root.path()),
    )
    .unwrap();
    send(
        &mut engine,
        &build("curl -sS -c jar.txt \"$BASE/me\"", &v, root.path()),
    )
    .unwrap();
    assert_eq!(srv.last().header("cookie"), Some("sid=abc"));
    let jar = std::fs::read_to_string(root.path().join("jar.txt")).unwrap();
    assert!(
        jar.contains("#HttpOnly_127.0.0.1\tFALSE\t/\tFALSE\t0\tsid\tabc"),
        "{jar}"
    );

    send(
        &mut engine,
        &build("curl -sS \"$BASE/plain\"", &v, root.path()),
    )
    .unwrap();
    assert_eq!(srv.last().header("cookie"), None, "no jar without -b or -c");

    let mut fresh = Engine::default();
    send(
        &mut fresh,
        &build("curl -sS -b jar.txt \"$BASE/again\"", &v, root.path()),
    )
    .unwrap();
    assert_eq!(srv.last().header("cookie"), Some("sid=abc"));
}

#[test]
fn proxy_and_resolve() {
    let srv = Recorder::ok();
    let root = tempfile::tempdir().unwrap();
    let v = vars(&[("PROXY", &srv.base)]);
    send(
        &mut Engine::default(),
        &build(
            "curl -sS -x \"$PROXY\" http://api.example.test/p?q=1",
            &v,
            root.path(),
        ),
    )
    .unwrap();
    let r = srv.last();
    assert_eq!(r.target, "http://api.example.test/p?q=1");
    assert_eq!(r.header("host"), Some("api.example.test"));

    let port = srv.port;
    send(
        &mut Engine::default(),
        &build(
            &format!("curl -sS --resolve api.example.test:{port}:127.0.0.1 http://api.example.test:{port}/r"),
            &v,
            root.path(),
        ),
    )
    .unwrap();
    let r = srv.last();
    assert_eq!(r.target, "/r");
    assert_eq!(
        r.header("host").unwrap(),
        format!("api.example.test:{port}")
    );
}

#[test]
fn variables_are_sent_verbatim() {
    let srv = Recorder::ok();
    let root = tempfile::tempdir().unwrap();
    let hostile = "a\"b 'c' $HOME `id` $(id) \\ ; | & ünï";
    let v = vars(&[("BASE", &srv.base), ("V", hostile)]);
    send(
        &mut Engine::default(),
        &build(
            "curl -sS \"$BASE/e\" -H \"X-V: $V\" --data-raw \"$V\"",
            &v,
            root.path(),
        ),
    )
    .unwrap();
    let r = srv.last();
    assert_eq!(r.body_text(), hostile);
    assert_eq!(
        String::from_utf8_lossy(r.header("x-v").unwrap().as_bytes()),
        hostile
    );
}
