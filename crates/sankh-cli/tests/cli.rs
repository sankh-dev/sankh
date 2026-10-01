mod common;

use assert_cmd::Command;
use common::petstore;
use predicates::prelude::*;
use std::path::{Path, PathBuf};

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/petstore")
}

/// A `sankh` command isolated from the developer's trust store and env.
fn sankh(config_dir: &Path) -> Command {
    let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("sankh");
    cmd.env("SANKH_CONFIG_DIR", config_dir)
        .env_remove("SANKH_TRUST")
        .env_remove("TOKEN")
        .env("NO_COLOR", "1");
    cmd
}

fn write(root: &Path, rel: &str, content: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, content).unwrap();
}

#[test]
fn petstore_flow_passes_and_reports() {
    let base = petstore::spawn();
    let cfg = tempfile::tempdir().unwrap();
    let report = cfg.path().join("report.json");
    sankh(cfg.path())
        .args([
            "run",
            "--env",
            "ci",
            "--trust",
            "--report",
            "json",
            "--report-file",
        ])
        .arg(&report)
        .arg(example())
        .env("BASE_URL", &base)
        .env("PET_PASSWORD", petstore::PASSWORD)
        .assert()
        .success()
        .stdout(predicate::str::contains("6 passed"))
        .stdout(predicate::str::contains("captured TOKEN=***"))
        .stdout(predicate::str::contains(petstore::TOKEN).not());

    let text = std::fs::read_to_string(&report).unwrap();
    assert!(!text.contains(petstore::TOKEN), "token leaked into report");
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(json["summary"]["passed"], 6);
    assert_eq!(json["results"][2]["captures"][0]["name"], "PET_ID");
}

#[test]
fn untrusted_folder_never_runs() {
    let cfg = tempfile::tempdir().unwrap();
    let col = tempfile::tempdir().unwrap();
    let marker = col.path().join("ran");
    write(
        col.path(),
        "a.sh",
        &format!("touch {}\ncurl -sS http://127.0.0.1:9/\n", marker.display()),
    );
    sankh(cfg.path())
        .arg("run")
        .arg(col.path())
        .assert()
        .code(3)
        .stderr(predicate::str::contains("is not trusted"));
    assert!(!marker.exists());

    sankh(cfg.path())
        .arg("trust")
        .arg(col.path())
        .assert()
        .success();
    sankh(cfg.path())
        .arg("run")
        .arg(col.path())
        .assert()
        .code(1);
    assert!(marker.exists());
}

#[test]
fn failing_assertion_exits_non_zero() {
    let base = petstore::spawn();
    let cfg = tempfile::tempdir().unwrap();
    let col = tempfile::tempdir().unwrap();
    write(
        col.path(),
        "01-wrong.sh",
        "# @expect status 200\n# @expect json .error == \"nope\"\ncurl -sS \"$BASE_URL/pets\"\n",
    );
    sankh(cfg.path())
        .args(["run", "--trust"])
        .arg(col.path())
        .env("BASE_URL", &base)
        .assert()
        .code(1)
        .stdout(predicate::str::contains("expected status 200, got 401"))
        .stdout(predicate::str::contains(
            r#"expected .error == "nope", got "unauthorized""#,
        ));
}

#[test]
fn hostile_env_values_are_neither_broken_nor_injected() {
    let base = petstore::spawn();
    let cfg = tempfile::tempdir().unwrap();
    let col = tempfile::tempdir().unwrap();
    let marker = col.path().join("pwned");
    let hostile = format!(
        "a\"b 'c' $HOME `touch {m}` $(touch {m}) \\ ; | & ünïcødé 🐚\nline2",
        m = marker.display()
    );
    write(
        col.path(),
        "echo.sh",
        "# @expect json .body == \"$HOSTILE\"\n# @expect json .headers[\"x-val\"] == \"$HOSTILE_HEADER\"\ncurl -sS \"$BASE_URL/echo\" -H \"X-Val: $HOSTILE_HEADER\" -d \"$HOSTILE\"\n",
    );
    sankh(cfg.path())
        .args(["run", "--trust"])
        .arg(col.path())
        .env("BASE_URL", &base)
        .env("HOSTILE", &hostile)
        .env("HOSTILE_HEADER", hostile.replace('\n', " "))
        .assert()
        .success();
    assert!(!marker.exists(), "command substitution was executed");
}

#[test]
fn filters_by_folder_and_tag() {
    let base = petstore::spawn();
    let cfg = tempfile::tempdir().unwrap();
    sankh(cfg.path())
        .args(["run", "--trust", "--tag", "smoke"])
        .arg(example())
        .env("BASE_URL", &base)
        .env("PET_PASSWORD", petstore::PASSWORD)
        .assert()
        .success()
        .stdout(predicate::str::contains("3 passed"));
    sankh(cfg.path())
        .args(["run", "--trust", "--folder", "auth"])
        .arg(example())
        .env("BASE_URL", &base)
        .env("PET_PASSWORD", petstore::PASSWORD)
        .assert()
        .success()
        .stdout(predicate::str::contains("1 passed"));
}

#[test]
fn httpmock_status_class_and_header_capture() {
    let server = httpmock::MockServer::start();
    server.mock(|when, then| {
        when.method("GET").path("/a");
        then.status(202).header("X-Trace", "trace-123").body("ok");
    });
    server.mock(|when, then| {
        when.method("GET").path("/b").header("x-trace", "trace-123");
        then.status(200)
            .json_body(serde_json::json!({ "ok": true }));
    });
    let cfg = tempfile::tempdir().unwrap();
    let col = tempfile::tempdir().unwrap();
    write(
        col.path(),
        "01-a.sh",
        "# @expect status 2xx\n# @capture TRACE=header x-trace\ncurl -sS \"$BASE_URL/a\"\n",
    );
    write(
        col.path(),
        "02-b.sh",
        "# @expect json .ok == true\ncurl -sS \"$BASE_URL/b\" -H \"X-Trace: $TRACE\"\n",
    );
    sankh(cfg.path())
        .args(["run", "--trust"])
        .arg(col.path())
        .env("BASE_URL", server.base_url())
        .assert()
        .success()
        .stdout(predicate::str::contains("2 passed"));
}

#[test]
fn parse_errors_are_reported_with_line_numbers() {
    let cfg = tempfile::tempdir().unwrap();
    let col = tempfile::tempdir().unwrap();
    write(
        col.path(),
        "bad.sh",
        "# @name Bad\n# @expect json .a = 1\ncurl -sS http://127.0.0.1:9/\n",
    );
    sankh(cfg.path())
        .args(["run", "--trust"])
        .arg(col.path())
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "line 2: unknown operator `=`, did you mean `==`?",
        ));
}

#[test]
fn init_creates_a_runnable_collection() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let col = dir.path().join("api");
    sankh(cfg.path()).arg("init").arg(&col).assert().success();
    assert!(col.join("sankh.toml").is_file());
    assert!(col.join("health/01-ping.sh").is_file());
    sankh(cfg.path())
        .arg("list")
        .arg(&col)
        .assert()
        .success()
        .stdout(predicate::str::contains("Ping"));
}
