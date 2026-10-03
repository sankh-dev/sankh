//! Renders a request back to file text (used by the UI form view and curl import).

use crate::parser;
use crate::request::{CurlCommand, Request};
use crate::shell::quote;
use serde::{Deserialize, Serialize};

/// Editable form of a request. Expectations and captures are kept as the
/// annotation argument text so the UI can edit them as plain strings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestForm {
    #[serde(default)]
    pub shebang: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// e.g. `status 201`, `json .data.id exists`
    #[serde(default)]
    pub expects: Vec<String>,
    /// e.g. `TOKEN=.data.token`
    #[serde(default)]
    pub captures: Vec<String>,
    #[serde(default)]
    pub extra_header_lines: Vec<String>,
    pub curl: CurlCommand,
}

impl RequestForm {
    /// Builds a form from a parsed request; `None` for raw-mode files.
    pub fn from_request(req: &Request) -> Option<RequestForm> {
        let curl = req.curl.clone()?;
        Some(RequestForm {
            shebang: req.shebang.clone(),
            name: req.name.clone(),
            description: req.description.clone(),
            tags: req.tags.clone(),
            expects: req.expects.iter().map(|e| e.to_string()).collect(),
            captures: req.captures.iter().map(|c| c.to_string()).collect(),
            extra_header_lines: req.extra_header_lines.clone(),
            curl,
        })
    }
}

/// Renders a form into canonical file text.
pub fn render(form: &RequestForm) -> String {
    let mut out = String::new();
    out.push_str(form.shebang.as_deref().unwrap_or("#!/usr/bin/env bash"));
    out.push('\n');
    if !form.name.trim().is_empty() {
        out.push_str(&format!("# @name {}\n", form.name.trim()));
    }
    if let Some(d) = form.description.as_deref().filter(|d| !d.trim().is_empty()) {
        out.push_str(&format!("# @description {}\n", d.trim()));
    }
    let tags: Vec<&str> = form
        .tags
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if !tags.is_empty() {
        out.push_str(&format!("# @tags {}\n", tags.join(" ")));
    }
    for line in &form.extra_header_lines {
        out.push_str(line);
        out.push('\n');
    }
    for e in form.expects.iter().filter(|e| !e.trim().is_empty()) {
        out.push_str(&format!("# @expect {}\n", e.trim()));
    }
    for c in form.captures.iter().filter(|c| !c.trim().is_empty()) {
        out.push_str(&format!("# @capture {}\n", c.trim()));
    }
    out.push_str(&render_curl(&form.curl));
    out.push('\n');
    out
}

const BODY_VALUE_FLAGS: &[&str] = &["-F", "--form", "--form-string", "--data-urlencode"];

/// Splits flags into those rendered on the `curl` line and body-like value
/// flags (`-F`, `--data-urlencode`, ...) rendered one per line after the
/// headers. Rendering emits them in that order, so it is the canonical order.
pub fn split_flags(flags: &[String]) -> (Vec<String>, Vec<String>) {
    let mut line = Vec::new();
    let mut body = Vec::new();
    let mut it = flags.iter();
    while let Some(f) = it.next() {
        if BODY_VALUE_FLAGS.contains(&f.as_str()) {
            body.push(f.clone());
            body.extend(it.next().cloned());
        } else {
            line.push(f.clone());
        }
    }
    (line, body)
}

/// Renders a curl command over multiple lines with continuations.
pub fn render_curl(c: &CurlCommand) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut first = String::from("curl");
    let (mut flags, body_flags) = split_flags(&c.flags);
    if flags.is_empty() {
        flags.push("-sS".into());
    }
    for f in &flags {
        first.push(' ');
        first.push_str(&quote(f));
    }
    let method = c.method.trim().to_ascii_uppercase();
    let implied = if c.body.is_some() { "POST" } else { "GET" };
    if !method.is_empty() && method != implied {
        first.push_str(&format!(" -X {method}"));
    }
    first.push(' ');
    first.push_str(&quote(&c.url));
    parts.push(first);
    for h in &c.headers {
        parts.push(format!("-H {}", quote(&format!("{}: {}", h.name, h.value))));
    }
    for pair in body_flags.chunks(2) {
        parts.push(pair.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" "));
    }
    if let Some(body) = &c.body {
        let flag = c.body_flag.as_deref().unwrap_or("-d");
        parts.push(format!("{flag} {}", quote(body)));
    }
    parts.join(" \\\n  ")
}

/// Builds a new request file from a pasted curl command.
pub fn import_curl(cmd: &str, name: &str) -> Result<String, String> {
    let curl = parser::parse_curl(cmd)?;
    Ok(render(&RequestForm {
        name: name.to_string(),
        curl,
        ..Default::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::Mode;

    #[test]
    fn round_trips_through_form() {
        let src = "#!/usr/bin/env bash\n# @name Create user\n# @tags smoke\n# keep me\n# @expect status 201\n# @expect json .data.name == \"test\"\n# @capture USER_ID=.data.id\ncurl -sS -X PUT \"$BASE_URL/users\" \\\n  -H \"Authorization: Bearer $TOKEN\" \\\n  -d '{\"name\":\"it'\\''s\"}'\n";
        let req = parser::parse(src, "x.sh");
        assert_eq!(req.mode, Mode::Form, "{:?}", req.raw_reason);
        let form = RequestForm::from_request(&req).unwrap();
        let rendered = render(&form);
        assert_eq!(rendered, src);
        let again = parser::parse(&rendered, "x.sh");
        assert_eq!(again, req);
    }

    #[test]
    fn form_fields_render_on_their_own_lines() {
        let src = "#!/usr/bin/env bash\n# @name Upload\ncurl -sS -u \"$U:$P\" -X POST \"$BASE_URL/up\" \\\n  -H 'Accept: */*' \\\n  -F file=@./a.png \\\n  --form-string 'note=a;b'\n";
        let req = parser::parse(src, "x.sh");
        assert_eq!(req.mode, Mode::Form, "{:?}", req.raw_reason);
        let form = RequestForm::from_request(&req).unwrap();
        assert_eq!(render(&form), src);
    }

    #[test]
    fn imports_curl() {
        let text = import_curl("curl https://x.dev/a -H 'Accept: */*'", "Get A").unwrap();
        assert_eq!(
            text,
            "#!/usr/bin/env bash\n# @name Get A\ncurl -sS https://x.dev/a \\\n  -H 'Accept: */*'\n"
        );
    }
}
