//! The tokenizer agrees with `sh` on quoting, escapes, line continuations and
//! variables. Variable values here have no spaces or glob characters: Sankh
//! never splits or globs a value, which is the one deliberate difference.

#![cfg(unix)]

mod common;

use common::vars;
use proptest::prelude::*;
use sankh_core::parse::{expand, tokenize};
use std::process::Command;

const VARS: [(&str, &str); 3] = [("X", "x1"), ("E", ""), ("Q", "it's \"q\"")];

fn ours(input: &str) -> Option<Vec<String>> {
    let tokens = tokenize::tokenize(input, 0).ok()?;
    let v = vars(&VARS);
    tokens
        .words
        .iter()
        .map(|w| expand::expand(w, &v).ok().map(|e| e.text))
        .collect()
}

fn sh(input: &str) -> Vec<String> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(format!("printf '%s\\0' start {input}"));
    for (k, v) in VARS {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("run sh");
    assert!(
        out.status.success(),
        "sh failed on {input:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    text.split_terminator('\0')
        .skip(1)
        .map(str::to_string)
        .collect()
}

fn double_quoted() -> impl Strategy<Value = String> {
    let inner = prop_oneof![
        "[a-z ';|&()<>*?~#]{1,4}",
        Just("\\\\".to_string()),
        Just("\\\"".to_string()),
        Just("\\$".to_string()),
        Just("\\`".to_string()),
        Just("\\a".to_string()),
        Just("${X}".to_string()),
        Just("$X ".to_string()),
        Just("${Q}".to_string()),
        Just("${E:-dflt}".to_string()),
        Just("\\\n".to_string()),
    ];
    prop::collection::vec(inner, 0..5).prop_map(|parts| format!("\"{}\"", parts.concat()))
}

fn fragment() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => "[a-zA-Z0-9=:/.,@%+_-]{1,6}",
        3 => Just(" ".to_string()),
        1 => Just("\t ".to_string()),
        2 => "'[a-z \"\\\\$`;|&()<>*?#]{0,5}'",
        3 => double_quoted(),
        2 => "\\\\[a-z ;|&\"'$*?<>()#~\\\\]",
        1 => Just("${X}".to_string()),
        1 => Just("$X ".to_string()),
        1 => Just("\"$Q\"".to_string()),
        1 => Just("${E:-dflt}".to_string()),
        1 => Just(" \\\n  ".to_string()),
        1 => Just("''".to_string()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn matches_sh(parts in prop::collection::vec(fragment(), 1..10)) {
        let input = parts.concat();
        if let Some(words) = ours(&input) {
            prop_assert_eq!(words, sh(&input), "input: {:?}", input);
        }
    }
}

#[test]
fn table() {
    for input in [
        "a b  c",
        "'a b' \"c d\" e\\ f",
        "\"a\\\"b\" 'a\\b' a\\\\b",
        "\"$X\"y ${X}y \"${E:-d}\" ${E:-d}",
        "'$X' \"\\$X\" \\$X",
        "-H\"X-A: $X\" --data-raw \"$Q\"",
        "one \\\n  two",
        "\"multi \\\n line\"",
        "'' \"\" x''y",
    ] {
        assert_eq!(ours(input).expect(input), sh(input), "{input:?}");
    }
}
