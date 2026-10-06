//! `sankh mcp` end to end: the real binary speaking JSON-RPC over stdio.

mod common;

use common::petstore;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/petstore")
}

fn sankh_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sankh"))
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Mcp {
    /// Starts `sankh mcp PATHS...` isolated from the developer's config, and
    /// completes the initialize handshake.
    fn start(config_dir: &Path, paths: &[&Path], envs: &[(&str, &str)]) -> Mcp {
        let mut cmd = Command::new(sankh_bin());
        cmd.arg("mcp")
            .args(paths)
            .env("SANKH_CONFIG_DIR", config_dir)
            .env("SANKH_DATA_DIR", config_dir.join("data"))
            .env("SANKH_TRUST", "1")
            .env_remove("TOKEN")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().expect("spawn sankh mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut mcp = Mcp {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        let init = mcp.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "sankh-test", "version": "0" }
            }),
        );
        assert_eq!(init["result"]["serverInfo"]["name"], "sankh", "{init}");
        mcp.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        mcp
    }

    fn send(&mut self, msg: Value) {
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).unwrap();
            assert!(n > 0, "server closed stdout");
            let msg: Value = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("non-JSON on stdout ({e}): {line}"));
            if msg["id"] == id {
                return msg;
            }
        }
    }

    /// Calls a tool; returns (is_error, text blocks joined).
    fn call(&mut self, name: &str, args: Value) -> (bool, String) {
        let res = self.request("tools/call", json!({ "name": name, "arguments": args }));
        let result = &res["result"];
        assert!(
            result.is_object(),
            "tool call failed at protocol level: {res}"
        );
        let text = result["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        (result["isError"] == true, text)
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn trust(config_dir: &Path, path: &Path) {
    let status = Command::new(sankh_bin())
        .arg("trust")
        .arg(path)
        .env("SANKH_CONFIG_DIR", config_dir)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
}

/// The JSON report is the last text block of a `run` result.
fn report(text: &str) -> Value {
    let start = text.find("\n{").expect("report JSON") + 1;
    serde_json::from_str(&text[start..]).unwrap()
}

#[test]
fn lists_tools_with_safety_annotations() {
    let cfg = tempfile::tempdir().unwrap();
    let mut mcp = Mcp::start(cfg.path(), &[&example()], &[]);
    let res = mcp.request("tools/list", json!({}));
    let tools = res["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for want in [
        "list_collections",
        "list_requests",
        "show_request",
        "list_environments",
        "run",
    ] {
        assert!(names.contains(&want), "missing {want} in {names:?}");
    }
    for t in tools {
        let read_only = t["annotations"]["readOnlyHint"] == true;
        assert_eq!(read_only, t["name"] != "run", "{t}");
    }
    let run = tools.iter().find(|t| t["name"] == "run").unwrap();
    assert_eq!(run["annotations"]["destructiveHint"], true);
}

#[test]
fn untrusted_collection_never_runs_even_with_sankh_trust_env() {
    let cfg = tempfile::tempdir().unwrap();
    let col = tempfile::tempdir().unwrap();
    let marker = col.path().join("ran");
    std::fs::write(
        col.path().join("a.sh"),
        format!(
            "touch '{}'\ncurl -sS http://127.0.0.1:9/\n",
            marker.display().to_string().replace('\\', "/")
        ),
    )
    .unwrap();
    let mut mcp = Mcp::start(cfg.path(), &[col.path()], &[]);

    let (_, listed) = mcp.call("list_collections", json!({}));
    let listed: Value = serde_json::from_str(&listed).unwrap();
    let entry = &listed["collections"][0];
    assert_eq!(entry["trust"]["state"], "untrusted", "{listed}");
    let id = entry["id"].as_str().unwrap().to_string();

    let (is_error, text) = mcp.call("run", json!({ "collection": id }));
    assert!(is_error, "{text}");
    assert!(text.contains("is not trusted"), "{text}");
    assert!(text.contains("sankh trust"), "{text}");
    assert!(!text.contains("--trust"), "{text}");
    assert!(!marker.exists(), "untrusted request ran");
}

#[test]
fn runs_petstore_with_redaction_and_rejects_escapes() {
    let base = petstore::spawn();
    let cfg = tempfile::tempdir().unwrap();
    trust(cfg.path(), &example());
    let mut mcp = Mcp::start(
        cfg.path(),
        &[&example()],
        &[("BASE_URL", &base), ("PET_PASSWORD", petstore::PASSWORD)],
    );

    let (is_error, tree) = mcp.call(
        "list_requests",
        json!({ "collection": "petstore", "tags": ["smoke"] }),
    );
    assert!(!is_error, "{tree}");
    assert!(tree.contains("auth/01-login.sh"), "{tree}");

    let (is_error, shown) = mcp.call(
        "show_request",
        json!({ "collection": "petstore", "path": "auth/01-login.sh" }),
    );
    assert!(!is_error, "{shown}");
    let shown: Value = serde_json::from_str(&shown).unwrap();
    assert_eq!(shown["request"]["name"], "Log in");

    for bad in ["../../Cargo.toml", "/etc/passwd.sh", "../outside.sh"] {
        let (is_error, text) = mcp.call(
            "show_request",
            json!({ "collection": "petstore", "path": bad }),
        );
        assert!(is_error, "{bad} was accepted: {text}");
    }
    let (is_error, _) = mcp.call("run", json!({ "collection": "petstore", "path": "../" }));
    assert!(is_error, "run escaped the collection root");

    let (is_error, envs) = mcp.call("list_environments", json!({ "collection": "petstore" }));
    assert!(!is_error, "{envs}");
    let envs: Value = serde_json::from_str(&envs).unwrap();
    assert_eq!(envs["default"], "dev");

    let (is_error, text) = mcp.call("run", json!({ "collection": "petstore", "env": "ci" }));
    assert!(!is_error, "{text}");
    assert!(text.starts_with("petstore (env ci): 6 passed"), "{text}");
    assert!(
        !text.contains(petstore::TOKEN),
        "token leaked into the MCP result"
    );
    assert!(
        !text.contains(petstore::PASSWORD),
        "password leaked into the MCP result"
    );
    let report = report(&text);
    assert_eq!(report["summary"]["passed"], 6);
    assert_eq!(report["results"][0]["captures"][0]["value"], "***");

    let (is_error, text) = mcp.call("run", json!({ "collection": "nope" }));
    assert!(is_error);
    assert!(text.contains("petstore"), "{text}");
}
