//! `@expect` evaluation.

use crate::jq;
use crate::request::{JsonOp, StatusPattern};
use crate::shell;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssertionResult {
    /// Annotation text, e.g. `status 201` or `json .data.id exists`.
    pub label: String,
    pub passed: bool,
    pub message: Option<String>,
}

impl AssertionResult {
    fn pass(label: String) -> Self {
        AssertionResult {
            label,
            passed: true,
            message: None,
        }
    }
    fn fail(label: String, message: impl Into<String>) -> Self {
        AssertionResult {
            label,
            passed: false,
            message: Some(message.into()),
        }
    }
}

pub fn check_status(patterns: &[StatusPattern], status: u16) -> AssertionResult {
    let shown: Vec<String> = patterns.iter().map(|p| p.to_string()).collect();
    let label = format!("status {}", shown.join("|"));
    if patterns.iter().any(|p| p.matches(status)) {
        AssertionResult::pass(label)
    } else {
        AssertionResult::fail(
            label,
            format!("expected status {}, got {status}", shown.join(" or ")),
        )
    }
}

/// Resolves the expected value literal, expanding `$VAR` inside strings.
pub fn resolve_value(src: &str, lookup: &impl Fn(&str) -> Option<String>) -> Result<Value, String> {
    let value: Value =
        serde_json::from_str(src).map_err(|_| format!("`{src}` is not a JSON literal"))?;
    Ok(match value {
        Value::String(s) => Value::String(shell::expand(&s, lookup)),
        other => other,
    })
}

pub fn check_json(
    expr: &str,
    op: JsonOp,
    value_src: Option<&str>,
    body: &[u8],
    lookup: &impl Fn(&str) -> Option<String>,
) -> AssertionResult {
    let label = match value_src {
        Some(v) => format!("json {expr} {} {v}", op.symbol()),
        None => format!("json {expr} {}", op.symbol()),
    };
    let outputs = match jq::run(expr, body) {
        Ok(o) => o,
        Err(e) => return AssertionResult::fail(label, e.to_string()),
    };
    if op == JsonOp::Exists {
        return if outputs.iter().any(|v| !v.is_null()) {
            AssertionResult::pass(label)
        } else {
            AssertionResult::fail(label, format!("`{expr}` is null or missing"))
        };
    }
    let actual = match outputs.as_slice() {
        [one] => one,
        [] => return AssertionResult::fail(label, format!("`{expr}` produced no value")),
        many => {
            return AssertionResult::fail(
                label,
                format!("`{expr}` produced {} values; expected one", many.len()),
            );
        }
    };
    let expected = match resolve_value(value_src.unwrap_or("null"), lookup) {
        Ok(v) => v,
        Err(e) => return AssertionResult::fail(label, e),
    };
    let shown_actual = compact(actual);
    let ok = match op {
        JsonOp::Eq => json_eq(actual, &expected),
        JsonOp::Ne => !json_eq(actual, &expected),
        JsonOp::Gt | JsonOp::Ge | JsonOp::Lt | JsonOp::Le => {
            let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) else {
                return AssertionResult::fail(
                    label,
                    format!("`{expr}` is {shown_actual}, not a number"),
                );
            };
            match op {
                JsonOp::Gt => a > b,
                JsonOp::Ge => a >= b,
                JsonOp::Lt => a < b,
                _ => a <= b,
            }
        }
        JsonOp::Matches => {
            let (Some(s), Some(pat)) = (actual.as_str(), expected.as_str()) else {
                return AssertionResult::fail(
                    label,
                    format!("`{expr}` is {shown_actual}, not a string"),
                );
            };
            match regex::Regex::new(pat) {
                Ok(re) => re.is_match(s),
                Err(e) => return AssertionResult::fail(label, format!("invalid regex: {e}")),
            }
        }
        JsonOp::Contains => match (actual, &expected) {
            (Value::String(s), Value::String(sub)) => s.contains(sub.as_str()),
            (Value::Array(items), e) => items.iter().any(|i| json_eq(i, e)),
            _ => {
                return AssertionResult::fail(
                    label,
                    format!("`contains` needs a string or array, got {shown_actual}"),
                );
            }
        },
        JsonOp::Exists => unreachable!(),
    };
    if ok {
        AssertionResult::pass(label)
    } else {
        AssertionResult::fail(
            label,
            format!(
                "expected {expr} {} {}, got {shown_actual}",
                op.symbol(),
                compact(&expected)
            ),
        )
    }
}

fn compact(v: &Value) -> String {
    let s = v.to_string();
    if s.chars().count() > 200 {
        format!("{}…", s.chars().take(200).collect::<String>())
    } else {
        s
    }
}

fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<String> {
        (name == "USER_ID").then(|| "42".to_string())
    }

    fn check(expr: &str, op: JsonOp, v: Option<&str>) -> AssertionResult {
        let body = br#"{"data":{"id":"42","qty":3,"name":"test","email":"a@b.c","tags":["new"],"deleted_at":null,"n":1.0}}"#;
        check_json(expr, op, v, body, &lookup)
    }

    #[test]
    fn evaluates_operators() {
        assert!(check(".data.name", JsonOp::Eq, Some("\"test\"")).passed);
        assert!(check(".data.qty", JsonOp::Ge, Some("1")).passed);
        assert!(check(".data.n", JsonOp::Eq, Some("1")).passed);
        assert!(check(".data.id", JsonOp::Exists, None).passed);
        assert!(!check(".data.nope", JsonOp::Exists, None).passed);
        assert!(check(".data.email", JsonOp::Matches, Some("\"^.+@.+$\"")).passed);
        assert!(check(".data.tags", JsonOp::Contains, Some("\"new\"")).passed);
        assert!(check(".data.deleted_at", JsonOp::Eq, Some("null")).passed);
        assert!(check(".data.id", JsonOp::Eq, Some("\"$USER_ID\"")).passed);
        let r = check(".data.qty", JsonOp::Lt, Some("2"));
        assert_eq!(r.message.as_deref(), Some("expected .data.qty < 2, got 3"));
        let r = check(".data.tags[]", JsonOp::Eq, Some("1"));
        assert!(!r.passed);
    }

    #[test]
    fn array_examples_from_docs() {
        use crate::parser::parse_expect;
        use crate::request::Expect;

        let body = br#"{"data":[{"id":"u_1","name":"Ann"},{"id":"u_2","name":"Bob"}]}"#;
        let run = |line: &str| {
            let Ok(Some(Expect::Json { expr, op, value })) = parse_expect(line) else {
                panic!("`{line}` did not parse as a json expectation");
            };
            check_json(&expr, op, value.as_deref(), body, &lookup)
        };
        for line in [
            r#"json .data[0].id == "u_1""#,
            r#"json .data[-1].id == "u_2""#,
            "json .data | length == 2",
            "json .data | length > 0",
            r#"json .data | map(.id) contains "u_2""#,
            r#"json any(.data[]; .name == "Bob") == true"#,
            r#"json all(.data[]; .id | startswith("u_")) == true"#,
        ] {
            let r = run(line);
            assert!(r.passed, "`{line}` failed: {:?}", r.message);
        }
        let r = run(r#"json .data[].id == "u_1""#);
        assert_eq!(
            r.message.as_deref(),
            Some("`.data[].id` produced 2 values; expected one")
        );
    }

    #[test]
    fn status_matching() {
        let p = [StatusPattern::Class { class: 2 }];
        assert!(check_status(&p, 204).passed);
        assert!(!check_status(&p, 404).passed);
    }

    #[test]
    fn non_json_body_fails_clearly() {
        let r = check_json(".a", JsonOp::Exists, None, b"<html>", &lookup);
        assert_eq!(
            r.message.as_deref(),
            Some("response body is not valid JSON")
        );
    }
}
