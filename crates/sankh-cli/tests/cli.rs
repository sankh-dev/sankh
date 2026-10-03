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
        .env("SANKH_DATA_DIR", config_dir.join("data"))
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
        &format!(
            "touch '{}'\ncurl -sS http://127.0.0.1:9/\n",
            marker.display().to_string().replace('\\', "/")
        ),
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
    // Git for Windows' native curl decodes argv in the ANSI code page, so
    // non-ASCII arguments cannot round-trip there.
    let unicode = if cfg!(windows) {
        ""
    } else {
        " ünïcødé 🐚"
    };
    let hostile = format!(
        "a\"b 'c' $HOME `touch {m}` $(touch {m}) \\ ; | &{unicode}\nline2",
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

fn postman_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sankh-core/tests/fixtures/postman")
        .join(name)
}

#[test]
fn imported_postman_petstore_runs() {
    let base = petstore::spawn();
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let col = dir.path().join("petstore");
    sankh(cfg.path())
        .args(["import", "postman"])
        .arg(postman_fixture("petstore.postman_collection.json"))
        .arg("--env")
        .arg(postman_fixture("petstore.postman_environment.json"))
        .arg("-o")
        .arg(&col)
        .assert()
        .success()
        .stdout(predicate::str::contains("6 request(s) in 2 folder(s)"))
        .stdout(predicate::str::contains("PET_PASSWORD"));

    sankh(cfg.path())
        .args(["run", "--trust"])
        .arg(&col)
        .env("BASE_URL", &base)
        .env("PET_PASSWORD", petstore::PASSWORD)
        .assert()
        .success()
        .stdout(predicate::str::contains("6 passed"));

    sankh(cfg.path())
        .args(["import", "postman"])
        .arg(postman_fixture("petstore.postman_collection.json"))
        .arg("-o")
        .arg(&col)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--force"));
}

#[test]
fn imported_postman_requests_send_the_same_bytes() {
    let server = httpmock::MockServer::start();
    let token = server.mock(|when, then| {
        when.method("POST")
            .path("/oauth/token")
            .header("authorization", "Basic YWRtaW46aHVudGVyMg==")
            .body_matches(r"^grant_type=password&scope=read(\+|%20)write$");
        then.status(200).body("{}");
    });
    let echo = server.mock(|when, then| {
        when.method("POST")
            .path("/echo")
            .query_param("debug", "1")
            .header("x-price", "$5 and EUR")
            .body(
                "{\n  \"text\": \"it's $HOME `whoami` \\\\ ünïcødé 🐚\",\n  \"user\": \"u-1\"\n}",
            );
        then.status(200).body("{}");
    });
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let col = dir.path().join("messy");
    sankh(cfg.path())
        .args(["import", "postman", "--json"])
        .arg(postman_fixture("messy.postman_collection.json"))
        .arg("-o")
        .arg(&col)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"requests\": 9"));

    sankh(cfg.path())
        .args(["run", "--trust"])
        .arg(col.join("01-forms-files/02-login-form.sh"))
        .env("BASE_URL", server.base_url())
        .env("BASIC_USER", "admin")
        .env("BASIC_PASSWORD", "hunter2")
        .assert()
        .success();
    token.assert();

    if cfg!(not(windows)) {
        sankh(cfg.path())
            .args(["run", "--trust"])
            .arg(col.join("02-quoting-it-s-5-quoted/01-echo-tricky-body.sh"))
            .env("BASE_URL", server.base_url())
            .env("USER_ID", "u-1")
            .assert()
            .success();
        echo.assert();
    }
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

#[test]
fn workspace_add_list_and_unlink() {
    let cfg = tempfile::tempdir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let users = dir.path().join("users");
    let payments = dir.path().join("payments");
    write(&users, "sankh.toml", "name = \"Users API\"\n");
    write(&payments, "environments/dev.env", "BASE_URL=x\n");

    sankh(cfg.path())
        .args(["workspace", "add"])
        .arg(&users)
        .arg(&payments)
        .assert()
        .success()
        .stdout(predicate::str::contains("added users-api"))
        .stdout(predicate::str::contains("added payments"));
    sankh(cfg.path())
        .args(["workspace", "add"])
        .arg(&users)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("already in the workspace"));
    assert!(cfg.path().join("workspace.toml").is_file());

    let out = sankh(cfg.path())
        .args(["workspace", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rows: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let ids: Vec<&str> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["scratch", "users-api", "payments"]);
    assert_eq!(rows[1]["trust"], "untrusted");

    sankh(cfg.path())
        .args(["workspace", "remove", "scratch"])
        .assert()
        .code(2);
    sankh(cfg.path())
        .args(["workspace", "remove", "users-api"])
        .assert()
        .success()
        .stdout(predicate::str::contains("unlinked users-api"));
    assert!(users.join("sankh.toml").is_file());
    sankh(cfg.path())
        .args(["workspace", "unlink"])
        .arg(&payments)
        .assert()
        .success();
    sankh(cfg.path())
        .args(["workspace", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("scratch"))
        .stdout(predicate::str::contains("users-api").not());
}
