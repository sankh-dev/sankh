use sankh_core::import::{self, ImportError, ImportOutput, postman};
use sankh_core::parser;
use sankh_core::request::Mode;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/postman")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn import(collection: &str, envs: &[&str]) -> ImportOutput {
    let envs: Vec<String> = envs.iter().map(|e| fixture(e)).collect();
    let refs: Vec<&str> = envs.iter().map(String::as_str).collect();
    import::render(&postman::read(&fixture(collection), &refs).unwrap())
}

fn tree(out: &ImportOutput) -> String {
    out.files
        .iter()
        .map(|(path, content)| format!("=== {path}\n{content}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every request file opens in form view, except files with an intentional TODO.
fn assert_round_trips(out: &ImportOutput) {
    for (path, content) in out.files.iter().filter(|(p, _)| p.ends_with(".sh")) {
        let name = path.rsplit('/').next().unwrap();
        let req = parser::parse(content, name);
        assert_eq!(req.mode, Mode::Form, "{path}: {:?}", req.raw_reason);
        if !content.contains("# TODO:") {
            assert!(!req.has_errors(), "{path}: {:?}", req.diagnostics);
        }
    }
    for w in &out.report.warnings {
        assert!(
            !w.message.contains("raw mode") && !w.message.contains("read back"),
            "{}: {}",
            w.path,
            w.message
        );
    }
}

#[test]
fn petstore_collection() {
    let out = import(
        "petstore.postman_collection.json",
        &["petstore.postman_environment.json"],
    );
    assert_round_trips(&out);
    insta::assert_snapshot!("petstore_files", tree(&out));
    insta::assert_yaml_snapshot!("petstore_report", out.report);
}

#[test]
fn messy_collection() {
    let out = import("messy.postman_collection.json", &[]);
    assert_round_trips(&out);
    insta::assert_snapshot!("messy_files", tree(&out));
    insta::assert_yaml_snapshot!("messy_report", out.report);
}

#[test]
fn secrets_are_never_written() {
    let out = import(
        "messy.postman_collection.json",
        &["petstore.postman_environment.json"],
    );
    let all = tree(&out);
    for secret in [
        "sk_live_should_not_leak",
        "pw-should-not-leak",
        "hunter2",
        "literal-token-123",
        "demo-password",
    ] {
        assert!(!all.contains(secret), "{secret} leaked");
    }
    let names: Vec<&str> = out
        .report
        .placeholders
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    for expected in [
        "API_KEY",
        "DB_PASSWORD",
        "BASIC_USER",
        "BASIC_PASSWORD",
        "BEARER_TOKEN",
        "PET_PASSWORD",
        "GUID",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} missing from {names:?}"
        );
    }
}

#[test]
fn write_refuses_non_empty_folder() {
    let out = import("petstore.postman_collection.json", &[]);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("keep.txt"), "x").unwrap();
    assert!(matches!(
        import::write(&out, dir.path(), false),
        Err(ImportError::NotEmpty(_))
    ));
    import::write(&out, dir.path(), true).unwrap();
    assert!(dir.path().join("02-pets/02-create-pet.sh").is_file());
    assert!(dir.path().join("keep.txt").is_file());

    let fresh = dir.path().join("new");
    import::write(&out, &fresh, false).unwrap();
    assert!(fresh.join("sankh.toml").is_file());
}
