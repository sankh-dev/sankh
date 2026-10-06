//! Run summaries and machine-readable reports.

use crate::runner::{Outcome, RequestResult};
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Summary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub cancelled: usize,
    pub duration_ms: f64,
}

impl Summary {
    pub fn of(results: &[RequestResult]) -> Summary {
        let mut s = Summary {
            total: results.len(),
            ..Default::default()
        };
        for r in results {
            match r.outcome {
                Outcome::Passed => s.passed += 1,
                Outcome::Failed => s.failed += 1,
                Outcome::Error => s.errors += 1,
                Outcome::Cancelled => s.cancelled += 1,
            }
            s.duration_ms += r.duration_ms;
        }
        s
    }

    pub fn success(&self) -> bool {
        self.failed == 0 && self.errors == 0 && self.cancelled == 0
    }
}

#[derive(Serialize)]
struct JsonReport<'a> {
    collection: &'a str,
    summary: Summary,
    results: &'a [RequestResult],
}

pub fn json(collection: &str, results: &[RequestResult]) -> String {
    serde_json::to_string_pretty(&JsonReport {
        collection,
        summary: Summary::of(results),
        results,
    })
    .unwrap_or_default()
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\n' | '\r' | '\t') => {}
            c => out.push(c),
        }
    }
    out
}

/// JUnit XML: one test suite per folder, one test case per request.
pub fn junit(collection: &str, results: &[RequestResult]) -> String {
    let mut suites: Vec<(String, Vec<&RequestResult>)> = Vec::new();
    for r in results {
        let folder = r
            .path
            .rsplit_once('/')
            .map(|(f, _)| f)
            .unwrap_or("")
            .to_string();
        match suites.iter_mut().find(|(f, _)| *f == folder) {
            Some((_, list)) => list.push(r),
            None => suites.push((folder, vec![r])),
        }
    }
    let total = Summary::of(results);
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(&format!(
        "<testsuites name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"{}\" time=\"{:.3}\">\n",
        xml_escape(collection),
        total.total,
        total.failed,
        total.errors,
        total.duration_ms / 1000.0
    ));
    for (folder, list) in suites {
        let owned: Vec<RequestResult> = list.iter().map(|r| (*r).clone()).collect();
        let s = Summary::of(&owned);
        let name = if folder.is_empty() {
            collection.to_string()
        } else {
            folder
        };
        out.push_str(&format!(
            "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"{}\" time=\"{:.3}\">\n",
            xml_escape(&name),
            s.total,
            s.failed,
            s.errors,
            s.duration_ms / 1000.0
        ));
        for r in list {
            out.push_str(&format!(
                "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\"",
                xml_escape(&r.name),
                xml_escape(&r.path),
                r.duration_ms / 1000.0
            ));
            let details = failure_details(r);
            match r.outcome {
                Outcome::Passed => out.push_str(" />\n"),
                Outcome::Failed => out.push_str(&format!(
                    ">\n      <failure message=\"{}\">{}</failure>\n    </testcase>\n",
                    xml_escape(details.lines().next().unwrap_or("failed")),
                    xml_escape(&details)
                )),
                Outcome::Error => out.push_str(&format!(
                    ">\n      <error message=\"{}\">{}</error>\n    </testcase>\n",
                    xml_escape(details.lines().next().unwrap_or("error")),
                    xml_escape(&details)
                )),
                Outcome::Cancelled => {
                    out.push_str(">\n      <skipped message=\"cancelled\" />\n    </testcase>\n")
                }
            }
        }
        out.push_str("  </testsuite>\n");
    }
    out.push_str("</testsuites>\n");
    out
}

pub fn failure_details(r: &RequestResult) -> String {
    let mut lines: Vec<String> = r
        .assertions
        .iter()
        .filter(|a| !a.passed)
        .map(|a| format!("{}: {}", a.label, a.message.as_deref().unwrap_or("failed")))
        .collect();
    if let Some(e) = &r.error {
        lines.push(e.clone());
    }
    lines.join("\n")
}
