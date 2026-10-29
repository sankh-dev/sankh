//! A raw HTTP/1.1 server that records each request byte for byte, and helpers
//! to build and send requests the way the runner does.

#![allow(dead_code)]

use sankh_core::http::{Engine, Response, SendError};
use sankh_core::parse::expand::{Lookup, Vars};
use sankh_core::parse::spec::{self, BuildCtx, RequestSpec};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct Recorded {
    pub method: String,
    pub target: String,
    /// Header names as sent (case preserved), in order.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

type Handler = dyn Fn(&Recorded, usize) -> Vec<u8> + Send + Sync;

pub struct Recorder {
    pub base: String,
    pub port: u16,
    pub seen: Arc<Mutex<Vec<Recorded>>>,
}

impl Recorder {
    /// Answers every request with `handler(request, index)`, closing the
    /// connection after each response.
    pub fn start(
        handler: impl Fn(&Recorded, usize) -> Vec<u8> + Send + Sync + 'static,
    ) -> Recorder {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen: Arc<Mutex<Vec<Recorded>>> = Arc::default();
        let handler: Arc<Handler> = Arc::new(handler);
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = log.clone();
                let handler = handler.clone();
                std::thread::spawn(move || serve(stream, &log, &*handler));
            }
        });
        Recorder {
            base: format!("http://127.0.0.1:{port}"),
            port,
            seen,
        }
    }

    /// Answers `200 OK` with an empty JSON object.
    pub fn ok() -> Recorder {
        Recorder::start(|_, _| respond(200, &[("Content-Type", "application/json")], b"{}"))
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.seen.lock().unwrap().clone()
    }

    pub fn last(&self) -> Recorded {
        self.requests().pop().expect("no request was recorded")
    }
}

fn serve(stream: TcpStream, log: &Mutex<Vec<Recorded>>, handler: &Handler) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.trim_end().splitn(3, ' ');
    let mut rec = Recorded {
        method: parts.next().unwrap_or_default().to_string(),
        target: parts.next().unwrap_or_default().to_string(),
        ..Recorded::default()
    };
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 {
            return;
        }
        let h = h.trim_end_matches(['\r', '\n']);
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            rec.headers
                .push((k.to_string(), v.trim_start().to_string()));
        }
    }
    if let Some(len) = rec
        .header("content-length")
        .and_then(|v| v.parse::<usize>().ok())
    {
        let mut body = vec![0; len];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        rec.body = body;
    } else if rec
        .header("transfer-encoding")
        .is_some_and(|v| v.eq_ignore_ascii_case("chunked"))
    {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size).unwrap();
            let n = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
            let mut chunk = vec![0; n + 2];
            reader.read_exact(&mut chunk).unwrap();
            if n == 0 {
                break;
            }
            rec.body.extend_from_slice(&chunk[..n]);
        }
    }
    let index = {
        let mut log = log.lock().unwrap();
        log.push(rec.clone());
        log.len() - 1
    };
    let reply = handler(&rec, index);
    let mut stream = stream;
    let _ = stream.write_all(&reply);
    let _ = stream.flush();
}

/// A complete response with `Content-Length` and `Connection: close`.
pub fn respond(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status} X\r\n");
    for (k, v) in headers {
        out.push_str(&format!("{k}: {v}\r\n"));
    }
    out.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

pub struct Map(pub BTreeMap<String, String>);

impl Vars for Map {
    fn lookup(&self, name: &str) -> Lookup {
        match self.0.get(name) {
            Some(v) => Lookup::Value(v.clone()),
            None => Lookup::Missing,
        }
    }
}

pub fn vars(pairs: &[(&str, &str)]) -> Map {
    Map(pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect())
}

/// Builds the request for a one-line request file `cmd`, with files read
/// from `root`.
pub fn build(cmd: &str, vars: &Map, root: &Path) -> RequestSpec {
    let content = format!("{cmd}\n");
    let req = sankh_core::parser::parse(&content, "t.sh");
    assert!(!req.has_errors(), "{cmd}: {:?}", req.diagnostics);
    let parsed = req.command.expect("parsed command");
    let ctx = BuildCtx {
        vars,
        root: Some(root),
    };
    match spec::build(&parsed, &ctx) {
        Ok(b) => b.spec,
        Err(e) => panic!("{cmd}: {e:?}"),
    }
}

pub fn send(engine: &mut Engine, spec: &RequestSpec) -> Result<Response, SendError> {
    engine.send(spec, Duration::from_secs(10), &AtomicBool::new(false))
}

/// Builds and sends `cmd` with `$BASE` set to `base`; returns the response.
pub fn run(cmd: &str, base: &str) -> Response {
    let root = tempfile::tempdir().unwrap();
    let spec = build(cmd, &vars(&[("BASE", base)]), root.path());
    send(&mut Engine::default(), &spec).unwrap_or_else(|e| panic!("{cmd}: {e:?}"))
}
