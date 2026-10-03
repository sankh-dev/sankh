//! Best-effort translation of common Postman test-script statements into
//! `@expect` and `@capture`. There is no JavaScript engine: statements are
//! matched with regular expressions, and anything else is reported.

use super::VarMap;
use crate::parser;
use regex::Regex;
use std::sync::LazyLock;

pub struct Translation {
    pub expects: Vec<String>,
    pub captures: Vec<String>,
    /// True when at least one statement could not be translated.
    pub untranslated: bool,
}

const EQ_OPS: &str = r"eql|equal|equals|eq|be\.equal|deep\.equal";

static EQ_CHAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^(?:{EQ_OPS})$")).unwrap());
static TEST_OPEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^pm\.test\(\s*(?:"[^"]*"|'[^']*'|`[^`]*`)\s*,\s*(?:function\s*\([^)]*\)|\([^)]*\)\s*=>|[A-Za-z_$][\w$]*\s*=>)\s*\{?"#,
    )
    .unwrap()
});
static CLOSE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\s}\)]*;?\s*$").unwrap());
static ALIAS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:var|let|const)\s+([A-Za-z_$][\w$]*)\s*=\s*pm\.response\.json\(\)$").unwrap()
});
static STATUS_HAVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^pm\.response\.to\.have\.status\(\s*(\d{3})\s*\)$").unwrap());
static STATUS_EXPECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^pm\.expect\(\s*pm\.response\.(?:code|status)\s*\)\.to\.(?:{EQ_OPS})\(\s*(\d{{3}})\s*\)$"
    ))
    .unwrap()
});
static STATUS_ONE_OF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^pm\.expect\(\s*pm\.response\.code\s*\)\.to\.be\.oneOf\(\s*\[([\d\s,]+)\]\s*\)$")
        .unwrap()
});
static SET_VAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^pm\.(?:environment|collectionVariables|globals|variables)\.set\(\s*["']([^"']+)["']\s*,\s*(.+?)\s*\)$"#,
    )
    .unwrap()
});
static EXPECT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^pm\.expect\(\s*(.+?)\s*\)\.to\.([A-Za-z.]+?)(?:\(\s*(.*?)\s*\))?$").unwrap()
});
static HEADER_GET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^pm\.response\.headers\.get\(\s*["']([^"']+)["']\s*\)$"#).unwrap()
});
static PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^(?:\.[A-Za-z_][\w]*|\[\d+\]|\[\s*["'][^"']+["']\s*\])*$"#).unwrap()
});
static PATH_PART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\.([A-Za-z_][\w]*)|\[(\d+)\]|\[\s*["']([^"']+)["']\s*\]"#).unwrap()
});

pub fn translate(script: &str, vars: &mut VarMap) -> Translation {
    let mut t = Translation {
        expects: Vec::new(),
        captures: Vec::new(),
        untranslated: false,
    };
    let mut aliases: Vec<String> = Vec::new();
    for line in script.lines() {
        for stmt in statements(line) {
            if !statement(&stmt, &mut aliases, vars, &mut t) {
                t.untranslated = true;
            }
        }
    }
    t
}

/// Splits a line into statements, dropping `pm.test(...)` wrappers and
/// closing braces.
fn statements(line: &str) -> Vec<String> {
    let mut line = line.trim().to_string();
    if line.starts_with("//") {
        return Vec::new();
    }
    if let Some(m) = TEST_OPEN.find(&line) {
        let mut rest = line[m.end()..].to_string();
        loop {
            let before = rest.len();
            rest = rest.trim_end().trim_end_matches(';').trim_end().to_string();
            if rest.ends_with('}')
                || (rest.ends_with(')') && rest.matches(')').count() > rest.matches('(').count())
            {
                rest.pop();
            }
            if rest.len() == before {
                break;
            }
        }
        line = rest;
    }
    split_outside_quotes(&line)
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && !CLOSE.is_match(s))
        .collect()
}

fn split_outside_quotes(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' | '`' => {
                    quote = Some(c);
                    cur.push(c);
                }
                ';' => out.push(std::mem::take(&mut cur)),
                c => cur.push(c),
            },
        }
    }
    out.push(cur);
    out
}

/// Handles one statement; false when it could not be translated.
fn statement(
    stmt: &str,
    aliases: &mut Vec<String>,
    vars: &mut VarMap,
    t: &mut Translation,
) -> bool {
    let stmt = stmt.trim().trim_end_matches(';').trim();
    if let Some(c) = ALIAS.captures(stmt) {
        aliases.push(c[1].to_string());
        return true;
    }
    if let Some(c) = STATUS_HAVE
        .captures(stmt)
        .or_else(|| STATUS_EXPECT.captures(stmt))
    {
        return push_expect(t, format!("status {}", &c[1]));
    }
    if let Some(c) = STATUS_ONE_OF.captures(stmt) {
        let codes: Vec<&str> = c[1]
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        return push_expect(t, format!("status {}", codes.join("|")));
    }
    match stmt {
        "pm.response.to.be.ok" => return push_expect(t, "status 200".into()),
        "pm.response.to.be.success" => return push_expect(t, "status 2xx".into()),
        _ => {}
    }
    if let Some(c) = SET_VAR.captures(stmt) {
        let name = vars.rename(&c[1]);
        let value = c[2].trim();
        let source = if let Some(path) = json_path(value, aliases) {
            path
        } else if let Some(h) = HEADER_GET.captures(value) {
            format!("header {}", &h[1])
        } else if value == "pm.response.code" {
            "status".into()
        } else {
            return false;
        };
        let capture = format!("{name}={source}");
        if parser::parse_capture(&capture).is_err() {
            return false;
        }
        t.captures.push(capture);
        return true;
    }
    if let Some(c) = EXPECT.captures(stmt) {
        let Some(path) = json_path(&c[1], aliases) else {
            return false;
        };
        let chain = &c[2];
        let arg = c.get(3).map(|m| m.as_str());
        let expect = if EQ_CHAIN.is_match(chain) {
            arg.and_then(literal).map(|v| format!("json {path} == {v}"))
        } else {
            match (chain, arg) {
                ("exist" | "not.be.undefined" | "not.be.null", None | Some("")) => {
                    Some(format!("json {path} exists"))
                }
                ("include" | "contain" | "have.string", Some(a)) => {
                    literal(a).map(|v| format!("json {path} contains {v}"))
                }
                ("be.above", Some(a)) => literal(a).map(|v| format!("json {path} > {v}")),
                ("be.below", Some(a)) => literal(a).map(|v| format!("json {path} < {v}")),
                ("be.at.least", Some(a)) => literal(a).map(|v| format!("json {path} >= {v}")),
                ("be.at.most", Some(a)) => literal(a).map(|v| format!("json {path} <= {v}")),
                _ => None,
            }
        };
        return expect.is_some_and(|e| push_expect(t, e));
    }
    false
}

fn push_expect(t: &mut Translation, expect: String) -> bool {
    match parser::parse_expect(&expect) {
        Ok(Some(_)) => {
            if !t.expects.contains(&expect) {
                t.expects.push(expect);
            }
            true
        }
        _ => false,
    }
}

/// `pm.response.json().data[0]["x-y"]` or `<alias>.data` -> `.data[0]["x-y"]`.
fn json_path(expr: &str, aliases: &[String]) -> Option<String> {
    let expr = expr.trim();
    let rest = expr
        .strip_prefix("pm.response.json()")
        .or_else(|| aliases.iter().find_map(|a| expr.strip_prefix(a.as_str())))?;
    if !PATH.is_match(rest) {
        return None;
    }
    let mut out = String::new();
    for c in PATH_PART.captures_iter(rest) {
        if let Some(field) = c.get(1) {
            out.push('.');
            out.push_str(field.as_str());
        } else if let Some(idx) = c.get(2) {
            if out.is_empty() {
                out.push('.');
            }
            out.push_str(&format!("[{}]", idx.as_str()));
        } else if let Some(key) = c.get(3) {
            if out.is_empty() {
                out.push('.');
            }
            out.push_str(&format!(
                "[{}]",
                serde_json::to_string(key.as_str()).unwrap()
            ));
        }
    }
    Some(if out.is_empty() { ".".into() } else { out })
}

/// JavaScript literal -> JSON literal, for simple values only.
fn literal(js: &str) -> Option<String> {
    let js = js.trim();
    if matches!(js, "true" | "false" | "null") {
        return Some(js.into());
    }
    if js.parse::<f64>().is_ok() && js.chars().all(|c| c.is_ascii_digit() || "-.".contains(c)) {
        return Some(js.into());
    }
    if js.len() >= 2 && js.starts_with('"') && js.ends_with('"') {
        return serde_json::from_str::<String>(js)
            .ok()
            .map(|_| js.to_string());
    }
    if js.len() >= 2 && js.starts_with('\'') && js.ends_with('\'') {
        let inner = &js[1..js.len() - 1];
        if inner.contains('\\') || inner.contains('\'') {
            return None;
        }
        return Some(serde_json::to_string(inner).unwrap());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(script: &str) -> Translation {
        translate(script, &mut VarMap::default())
    }

    #[test]
    fn translates_common_tests() {
        let t = run(r#"pm.test("Status code is 201", function () {
    pm.response.to.have.status(201);
});
var jsonData = pm.response.json();
pm.test("name", () => {
    pm.expect(jsonData.name).to.eql('Rex');
    pm.expect(jsonData.tags).to.include("good-boy");
    pm.expect(jsonData.items[0]["x-y"]).to.exist;
});
pm.environment.set("petId", jsonData.id);
pm.collectionVariables.set("requestId", pm.response.headers.get("X-Request-Id"));
pm.test("one liner", () => pm.expect(pm.response.code).to.be.oneOf([200, 201]));
// a comment
"#);
        assert!(!t.untranslated);
        assert_eq!(
            t.expects,
            vec![
                "status 201",
                r#"json .name == "Rex""#,
                r#"json .tags contains "good-boy""#,
                r#"json .items[0]["x-y"] exists"#,
                "status 200|201",
            ]
        );
        assert_eq!(
            t.captures,
            vec!["PET_ID=.id", "REQUEST_ID=header X-Request-Id"]
        );
    }

    #[test]
    fn reports_untranslatable_statements() {
        let t = run(
            "pm.environment.set(\"token\", pm.response.json().token);\nconsole.log(pm.response.text());\n",
        );
        assert!(t.untranslated);
        assert_eq!(t.captures, vec!["TOKEN=.token"]);
        let t = run("pm.expect(pm.response.responseTime).to.be.below(200);");
        assert!(t.untranslated);
        assert!(t.expects.is_empty());
    }

    #[test]
    fn literals() {
        assert_eq!(literal("'it'").as_deref(), Some("\"it\""));
        assert_eq!(literal("42").as_deref(), Some("42"));
        assert_eq!(literal("\"a\\\"b\"").as_deref(), Some("\"a\\\"b\""));
        assert_eq!(literal("foo"), None);
    }
}
