//! Postman Collection v2.0 / v2.1 reader.

use super::postman_script::{self, Translation};
use super::*;
use crate::redact::is_secret_name;
use crate::request::Header;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct PmCollection {
    info: Option<PmInfo>,
    #[serde(default)]
    item: Vec<PmItem>,
    #[serde(default)]
    event: Vec<PmEvent>,
    #[serde(default)]
    variable: Vec<PmVariable>,
    auth: Option<Value>,
}

#[derive(Deserialize)]
struct PmInfo {
    name: Option<String>,
    schema: Option<String>,
}

#[derive(Deserialize)]
struct PmItem {
    name: Option<String>,
    item: Option<Vec<PmItem>>,
    request: Option<Value>,
    #[serde(default)]
    event: Vec<PmEvent>,
    description: Option<Value>,
    auth: Option<Value>,
    #[serde(default)]
    variable: Vec<PmVariable>,
}

#[derive(Deserialize)]
struct PmEvent {
    listen: Option<String>,
    script: Option<PmScript>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
struct PmScript {
    exec: Option<Value>,
}

#[derive(Deserialize)]
struct PmVariable {
    key: Option<String>,
    id: Option<String>,
    value: Option<Value>,
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize, Default)]
struct PmRequest {
    method: Option<String>,
    url: Option<Value>,
    header: Option<Value>,
    body: Option<PmBody>,
    auth: Option<Value>,
    description: Option<Value>,
}

#[derive(Deserialize)]
struct PmBody {
    mode: Option<String>,
    raw: Option<String>,
    #[serde(default)]
    urlencoded: Vec<PmKv>,
    #[serde(default)]
    formdata: Vec<PmKv>,
    file: Option<PmFile>,
    graphql: Option<Value>,
    options: Option<Value>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Deserialize)]
struct PmKv {
    key: Option<String>,
    value: Option<Value>,
    #[serde(default)]
    disabled: bool,
    #[serde(rename = "type")]
    kind: Option<String>,
    src: Option<Value>,
}

#[derive(Deserialize)]
struct PmFile {
    src: Option<String>,
    content: Option<String>,
}

#[derive(Deserialize)]
struct PmEnvironment {
    name: Option<String>,
    #[serde(default)]
    values: Vec<PmEnvValue>,
}

#[derive(Deserialize)]
struct PmEnvValue {
    key: Option<String>,
    value: Option<Value>,
    enabled: Option<bool>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

/// Reads a Postman collection export plus optional environment exports.
pub fn read(collection: &str, environments: &[&str]) -> Result<ImportedCollection, ImportError> {
    let json: Value = serde_json::from_str(collection)
        .map_err(|e| ImportError::Invalid(format!("not valid JSON: {e}")))?;
    if json.get("info").is_none() {
        let hint = if json.get("requests").is_some() || json.get("order").is_some() {
            "this looks like a Postman v1 collection; re-export it as Collection v2.1"
        } else if json.get("values").is_some() {
            "this looks like a Postman environment; pass it with --env"
        } else {
            "missing `info`; expected a Postman Collection v2.0 or v2.1 export"
        };
        return Err(ImportError::Invalid(hint.into()));
    }
    let pm: PmCollection = serde_json::from_value(json)
        .map_err(|e| ImportError::Invalid(format!("unexpected collection shape: {e}")))?;
    if let Some(schema) = pm.info.as_ref().and_then(|i| i.schema.as_deref()) {
        if !schema.contains("v2.0") && !schema.contains("v2.1") {
            return Err(ImportError::Invalid(format!(
                "unsupported Postman schema `{schema}`; re-export as Collection v2.1"
            )));
        }
    }

    let mut r = Reader::default();
    r.out.name = pm
        .info
        .as_ref()
        .and_then(|i| i.name.clone())
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "imported".into());
    r.out.root.name = r.out.name.clone();

    let mut root_warnings = Vec::new();
    scripts_warning(&pm.event, "collection", &mut root_warnings);
    let auth = pm.auth.clone();
    let items = r.items(&pm.item, auth.as_ref());
    r.out.root.items = items;
    if r.blank_auth > 0 {
        root_warnings.push(format!(
            "{} item(s) set auth with empty credentials; used the inherited auth instead",
            r.blank_auth
        ));
    }
    r.out.root.warnings.extend(root_warnings);

    let collection_vars: Vec<(String, String, bool)> = pm
        .variable
        .iter()
        .filter(|v| !v.disabled)
        .filter_map(|v| {
            let key = v.key.clone().or_else(|| v.id.clone())?;
            let secret = v.kind.as_deref() == Some("secret");
            Some((key, value_str(v.value.as_ref()), secret))
        })
        .collect();

    if environments.is_empty() {
        if !collection_vars.is_empty() {
            r.environment("default", &collection_vars, &[]);
        }
    } else {
        for (i, text) in environments.iter().enumerate() {
            let env: PmEnvironment = serde_json::from_str(text).map_err(|e| {
                ImportError::Invalid(format!(
                    "environment #{}: not a Postman environment: {e}",
                    i + 1
                ))
            })?;
            let values: Vec<(String, String, bool)> = env
                .values
                .iter()
                .filter(|v| v.enabled != Some(false))
                .filter_map(|v| {
                    Some((
                        v.key.clone()?,
                        value_str(v.value.as_ref()),
                        v.kind.as_deref() == Some("secret"),
                    ))
                })
                .collect();
            let name = env
                .name
                .filter(|n| !n.trim().is_empty())
                .unwrap_or_else(|| format!("env-{}", i + 1));
            r.environment(&name, &collection_vars, &values);
        }
    }
    Ok(r.out)
}

#[derive(Default)]
struct Reader {
    out: ImportedCollection,
    /// (generated variable, literal value) for credentials moved out of files.
    credentials: Vec<(String, String)>,
    /// Items whose own auth had empty credentials and fell back to the parent's.
    blank_auth: usize,
}

impl Reader {
    fn items(&mut self, items: &[PmItem], auth: Option<&Value>) -> Vec<ImportedItem> {
        items.iter().map(|item| self.item(item, auth)).collect()
    }

    fn item(&mut self, item: &PmItem, parent_auth: Option<&Value>) -> ImportedItem {
        let name = one_line(item.name.as_deref().unwrap_or(""));
        if let Some(children) = &item.item {
            let auth = self.effective_auth(item.auth.as_ref(), parent_auth);
            let mut folder = ImportedFolder {
                name: if name.is_empty() {
                    "folder".into()
                } else {
                    name
                },
                ..Default::default()
            };
            scripts_warning(&item.event, "folder", &mut folder.warnings);
            if item.variable.iter().any(|v| !v.disabled) {
                folder
                    .warnings
                    .push("folder variables are not imported; define them in an env file".into());
            }
            folder.items = self.items(children, auth);
            return ImportedItem::Folder(folder);
        }
        let mut req = ImportedRequest {
            name: if name.is_empty() {
                "request".into()
            } else {
                name
            },
            ..Default::default()
        };
        let pm_req = match &item.request {
            Some(Value::String(url)) => PmRequest {
                url: Some(Value::String(url.clone())),
                ..Default::default()
            },
            Some(v) => serde_json::from_value(v.clone()).unwrap_or_else(|e| {
                req.warnings.push(format!(
                    "could not read the request ({e}); wrote an empty GET"
                ));
                PmRequest::default()
            }),
            None => {
                req.warnings
                    .push("item has no request; wrote an empty GET".into());
                PmRequest::default()
            }
        };
        req.description = item
            .description
            .as_ref()
            .or(pm_req.description.as_ref())
            .and_then(first_line);
        self.request(&pm_req, &mut req);
        if let Some(auth) = self.effective_auth(pm_req.auth.as_ref(), parent_auth) {
            self.auth(auth, &mut req);
        }
        self.events(&item.event, &mut req);
        ImportedItem::Request(Box::new(req))
    }

    /// An item's own auth, or the parent's when the own one is missing or has
    /// only empty credentials (a common export artifact).
    fn effective_auth<'a>(
        &mut self,
        own: Option<&'a Value>,
        parent: Option<&'a Value>,
    ) -> Option<&'a Value> {
        match own {
            Some(a) if parent.is_some() && is_blank_auth(a) => {
                self.blank_auth += 1;
                parent
            }
            Some(a) => Some(a),
            None => parent,
        }
    }

    fn request(&mut self, pm: &PmRequest, req: &mut ImportedRequest) {
        let vars = &mut self.out.vars;
        req.curl.flags.push("-sS".into());
        req.curl.method = pm
            .method
            .as_deref()
            .map(|m| m.trim().to_ascii_uppercase())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| "GET".into());
        let raw_url = pm
            .url
            .as_ref()
            .map(|u| url_text(u, &mut req.warnings))
            .unwrap_or_default();
        if raw_url.is_empty() {
            req.warnings.push("request has no URL".into());
        }
        req.curl.url = vars.word(&raw_url);
        let globs = vars
            .segments(&raw_url)
            .iter()
            .any(|s| matches!(s, Segment::Text(t) if t.contains(['[', ']', '{', '}'])));
        if globs {
            req.curl.flags.push("-g".into());
        }

        for (key, value, disabled) in header_list(pm.header.as_ref()) {
            if disabled {
                if !key.trim().is_empty() {
                    req.warnings
                        .push(format!("disabled header `{key}` was dropped"));
                }
                continue;
            }
            req.curl.headers.push(Header {
                name: vars.word(key.trim()),
                value: vars.word(value.trim()),
            });
        }

        if let Some(body) = &pm.body {
            self.body(body, req);
        }
        let has_form = req
            .curl
            .flags
            .iter()
            .any(|f| f == "--form-string" || f == "-F" || f == "--data-urlencode");
        if has_form && req.curl.method == "GET" {
            req.warnings
                .push("GET request with a form body: curl will send it as POST".into());
        }
    }

    fn body(&mut self, body: &PmBody, req: &mut ImportedRequest) {
        let vars = &mut self.out.vars;
        if body.disabled {
            req.warnings.push("disabled body was dropped".into());
            return;
        }
        match body.mode.as_deref().unwrap_or("raw") {
            "raw" => {
                let raw = body.raw.clone().unwrap_or_default();
                if raw.is_empty() {
                    return;
                }
                let language = body
                    .options
                    .as_ref()
                    .and_then(|o| o.pointer("/raw/language"))
                    .and_then(Value::as_str);
                let content_type = match language {
                    Some("json") => Some("application/json"),
                    Some("xml") => Some("application/xml"),
                    Some("html") => Some("text/html"),
                    Some("javascript") => Some("application/javascript"),
                    Some("text") => Some("text/plain"),
                    _ => None,
                };
                if let Some(ct) = content_type {
                    set_default_header(req, "Content-Type", ct);
                }
                if raw.starts_with('@') {
                    req.curl.body_flag = Some("--data-raw".into());
                }
                req.curl.body = Some(vars.word(&raw));
            }
            "urlencoded" => {
                for kv in &body.urlencoded {
                    let key = kv.key.clone().unwrap_or_default();
                    if kv.disabled {
                        if !key.trim().is_empty() {
                            req.warnings
                                .push(format!("disabled form field `{key}` was dropped"));
                        }
                        continue;
                    }
                    let value = value_str(kv.value.as_ref());
                    req.curl.flags.push("--data-urlencode".into());
                    req.curl.flags.push(vars.word(&format!("{key}={value}")));
                }
            }
            "formdata" => {
                for kv in &body.formdata {
                    let key = kv.key.clone().unwrap_or_default();
                    if kv.disabled {
                        if !key.trim().is_empty() {
                            req.warnings
                                .push(format!("disabled form field `{key}` was dropped"));
                        }
                        continue;
                    }
                    if kv.kind.as_deref() == Some("file") {
                        let srcs: Vec<String> = match &kv.src {
                            Some(Value::String(s)) => vec![s.clone()],
                            Some(Value::Array(a)) => a
                                .iter()
                                .filter_map(|s| s.as_str().map(String::from))
                                .collect(),
                            _ => Vec::new(),
                        };
                        if srcs.is_empty() {
                            req.warnings
                                .push(format!("file field `{key}` has no file; add its path"));
                        }
                        for src in srcs {
                            req.curl.flags.push("-F".into());
                            req.curl.flags.push(vars.word(&format!("{key}=@{src}")));
                            req.warnings.push(format!(
                                "file field `{key}` points to `{src}`; check the path exists here"
                            ));
                        }
                    } else {
                        let value = value_str(kv.value.as_ref());
                        req.curl.flags.push("--form-string".into());
                        req.curl.flags.push(vars.word(&format!("{key}={value}")));
                    }
                }
            }
            "file" => {
                let src = body.file.as_ref().and_then(|f| f.src.clone());
                match src.filter(|s| !s.is_empty()) {
                    Some(src) => {
                        req.curl.body_flag = Some("--data-binary".into());
                        req.curl.body = Some(vars.word(&format!("@{src}")));
                        req.warnings.push(format!(
                            "binary body points to `{src}`; check the path exists here"
                        ));
                    }
                    None => {
                        if let Some(content) = body.file.as_ref().and_then(|f| f.content.clone()) {
                            req.curl.body_flag = Some("--data-binary".into());
                            req.curl.body = Some(vars.word(&content));
                        } else {
                            req.warnings
                                .push("binary body has no file; add its path".into());
                        }
                    }
                }
            }
            "graphql" => {
                let gql = body.graphql.clone().unwrap_or(Value::Null);
                let query = gql.get("query").and_then(Value::as_str).unwrap_or("");
                let mut payload = serde_json::Map::new();
                payload.insert("query".into(), Value::String(query.to_string()));
                match gql.get("variables") {
                    Some(Value::String(s)) if !s.trim().is_empty() => {
                        match serde_json::from_str::<Value>(s) {
                            Ok(v) => {
                                payload.insert("variables".into(), v);
                            }
                            Err(_) => {
                                payload.insert("variables".into(), Value::String(s.clone()));
                                req.warnings.push(
                                    "GraphQL variables are not valid JSON; sent as a string".into(),
                                );
                            }
                        }
                    }
                    Some(v @ Value::Object(_)) => {
                        payload.insert("variables".into(), v.clone());
                    }
                    _ => {}
                }
                set_default_header(req, "Content-Type", "application/json");
                let text = serde_json::to_string(&Value::Object(payload)).unwrap();
                req.curl.body = Some(vars.word(&text));
            }
            other => req.warnings.push(format!(
                "body mode `{other}` is not supported; body dropped"
            )),
        }
    }

    fn auth(&mut self, auth: &Value, req: &mut ImportedRequest) {
        let kind = auth.get("type").and_then(Value::as_str).unwrap_or("noauth");
        match kind {
            "noauth" | "inherit" => {}
            "bearer" => {
                let token = auth_param(auth, "bearer", "token").unwrap_or_default();
                let token = self.credential(&token, "BEARER_TOKEN", "bearer token", req);
                set_default_header(req, "Authorization", &format!("Bearer {token}"));
            }
            "basic" => {
                let user = auth_param(auth, "basic", "username").unwrap_or_default();
                let pass = auth_param(auth, "basic", "password").unwrap_or_default();
                let user = self.credential(&user, "BASIC_USER", "basic auth username", req);
                let pass = self.credential(&pass, "BASIC_PASSWORD", "basic auth password", req);
                if !req.curl.flags.iter().any(|f| f == "-u" || f == "--user") {
                    req.curl.flags.push("-u".into());
                    req.curl.flags.push(format!("{user}:{pass}"));
                }
            }
            "apikey" => {
                let key = auth_param(auth, "apikey", "key")
                    .filter(|k| !k.is_empty())
                    .unwrap_or_else(|| "X-API-Key".into());
                let value = auth_param(auth, "apikey", "value").unwrap_or_default();
                let value = self.credential(&value, "API_KEY", "API key", req);
                let key = self.out.vars.word(&key);
                if auth_param(auth, "apikey", "in").as_deref() == Some("query") {
                    let sep = if req.curl.url.contains('?') { '&' } else { '?' };
                    req.curl.url.push_str(&format!("{sep}{key}={value}"));
                } else {
                    set_default_header(req, &key, &value);
                }
            }
            other => {
                req.comments.push(format!(
                    "# TODO: `{other}` auth from Postman was not converted; add the header or curl flag by hand"
                ));
                req.warnings.push(format!(
                    "`{other}` auth is not converted; added a TODO comment"
                ));
            }
        }
    }

    /// Returns a word for a credential. Literal values are moved into a
    /// generated variable so they are never written into request files.
    fn credential(
        &mut self,
        raw: &str,
        base: &str,
        what: &str,
        req: &mut ImportedRequest,
    ) -> String {
        if raw.contains("{{") {
            return self.out.vars.word(raw);
        }
        let existing = self
            .credentials
            .iter()
            .find(|(_, v)| v == raw)
            .map(|(n, _)| n.clone());
        let name = match existing {
            Some(n) => n,
            None => {
                let n = self.out.vars.fresh(base);
                self.credentials.push((n.clone(), raw.to_string()));
                let note = if raw.is_empty() {
                    format!("{what} (empty in the Postman collection)")
                } else {
                    format!("{what}; the literal value from Postman was not written to disk")
                };
                self.out.add_placeholder(&n, &note);
                n
            }
        };
        if !raw.is_empty() {
            req.warnings.push(format!(
                "literal {what} replaced with ${name}; set it in .env.local"
            ));
        }
        format!("${{{name}}}")
    }

    fn events(&mut self, events: &[PmEvent], req: &mut ImportedRequest) {
        for ev in events.iter().filter(|e| !e.disabled) {
            let script = script_text(ev);
            if script.trim().is_empty() {
                continue;
            }
            match ev.listen.as_deref() {
                Some("test") => {
                    let Translation {
                        expects,
                        captures,
                        untranslated,
                    } = postman_script::translate(&script, &mut self.out.vars);
                    req.expects.extend(expects);
                    req.captures.extend(captures);
                    if untranslated {
                        keep_script(req, "test", &script);
                    }
                }
                Some(listen) => keep_script(req, listen, &script),
                None => {}
            }
        }
    }

    fn environment(
        &mut self,
        name: &str,
        base: &[(String, String, bool)],
        overlay: &[(String, String, bool)],
    ) {
        let mut merged: Vec<(String, String, bool)> = base.to_vec();
        for (k, v, s) in overlay {
            match merged.iter_mut().find(|(mk, _, _)| mk == k) {
                Some(e) => *e = (k.clone(), v.clone(), *s || e.2),
                None => merged.push((k.clone(), v.clone(), *s)),
            }
        }
        let mut env = ImportedEnv {
            name: name.to_string(),
            vars: Vec::new(),
        };
        for (key, value, secret) in merged {
            let renamed = self.out.vars.rename(&key);
            if !value.is_empty() && (secret || is_secret_name(&renamed)) {
                let only_refs = self
                    .out
                    .vars
                    .segments(&value)
                    .iter()
                    .all(|s| matches!(s, Segment::Var(_)));
                let note = if only_refs {
                    format!("secret; was `{value}` in Postman (set it in .env.local or CI)")
                } else {
                    "secret; its value was not written to environments/ (set it in .env.local or CI)"
                        .into()
                };
                self.out.add_placeholder(&renamed, &note);
                continue;
            }
            let value = self.out.vars.env_value(&value);
            env.vars.push((renamed, value));
        }
        self.out.environments.push(env);
    }
}

fn keep_script(req: &mut ImportedRequest, listen: &str, script: &str) {
    let label = if listen == "prerequest" {
        "pre-request"
    } else {
        listen
    };
    req.comments
        .push(format!("# [imported {label} script, not executed]"));
    for line in script.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            req.comments.push("# |".into());
        } else {
            req.comments.push(format!("# | {line}"));
        }
    }
    req.warnings.push(format!(
        "{label} script could not be fully converted; kept as comments"
    ));
}

fn scripts_warning(events: &[PmEvent], level: &str, warnings: &mut Vec<String>) {
    for ev in events.iter().filter(|e| !e.disabled) {
        let script = script_text(ev);
        if !script.trim().is_empty() {
            warnings.push(format!(
                "{level}-level {} script ({} lines) is not imported; Sankh has no shared scripts",
                ev.listen.as_deref().unwrap_or("unknown"),
                script.lines().count()
            ));
        }
    }
}

fn script_text(ev: &PmEvent) -> String {
    match ev.script.as_ref().and_then(|s| s.exec.as_ref()) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(lines)) => lines
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn set_default_header(req: &mut ImportedRequest, name: &str, value: &str) {
    if !req
        .curl
        .headers
        .iter()
        .any(|h| h.name.eq_ignore_ascii_case(name))
    {
        req.curl.headers.push(Header {
            name: name.to_string(),
            value: value.to_string(),
        });
    }
}

fn is_blank_auth(auth: &Value) -> bool {
    let kind = auth.get("type").and_then(Value::as_str).unwrap_or("");
    let fields: &[&str] = match kind {
        "basic" => &["username", "password"],
        "bearer" => &["token"],
        "apikey" => &["value"],
        _ => return false,
    };
    fields.iter().all(|f| {
        auth_param(auth, kind, f)
            .unwrap_or_default()
            .trim()
            .is_empty()
    })
}

/// Reads an auth parameter in v2.1 (`[{key, value}]`) or v2.0 (`{key: value}`) form.
fn auth_param(auth: &Value, kind: &str, key: &str) -> Option<String> {
    match auth.get(kind)? {
        Value::Array(params) => params
            .iter()
            .find(|p| p.get("key").and_then(Value::as_str) == Some(key))
            .map(|p| value_str(p.get("value"))),
        Value::Object(o) => o.get(key).map(|v| value_str(Some(v))),
        _ => None,
    }
}

/// Header entries as (key, value, disabled), from an array or a v2.0 string.
fn header_list(header: Option<&Value>) -> Vec<(String, String, bool)> {
    match header {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|h| {
                let key = h.get("key").and_then(Value::as_str)?;
                Some((
                    key.to_string(),
                    value_str(h.get("value")),
                    h.get("disabled").and_then(Value::as_bool).unwrap_or(false),
                ))
            })
            .collect(),
        Some(Value::String(s)) => s
            .lines()
            .filter_map(|l| {
                let (k, v) = l.split_once(':')?;
                Some((k.trim().to_string(), v.trim().to_string(), false))
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The request URL as source text, with `:name` path variables filled in.
fn url_text(url: &Value, warnings: &mut Vec<String>) -> String {
    let obj = match url {
        Value::String(s) => return s.clone(),
        Value::Object(o) => o,
        _ => return String::new(),
    };
    let mut text = match obj.get("raw").and_then(Value::as_str) {
        Some(raw) => raw.to_string(),
        None => {
            let join = |v: Option<&Value>, sep: &str| match v {
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|p| match p {
                        Value::String(s) => s.clone(),
                        other => value_str(other.get("value")),
                    })
                    .collect::<Vec<_>>()
                    .join(sep),
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            let mut t = String::new();
            if let Some(p) = obj.get("protocol").and_then(Value::as_str) {
                t.push_str(&format!("{p}://"));
            }
            t.push_str(&join(obj.get("host"), "."));
            if let Some(port) = obj.get("port").and_then(Value::as_str) {
                t.push_str(&format!(":{port}"));
            }
            let path = join(obj.get("path"), "/");
            if !path.is_empty() {
                t.push('/');
                t.push_str(&path);
            }
            let query: Vec<String> = match obj.get("query") {
                Some(Value::Array(q)) => q
                    .iter()
                    .filter(|p| !p.get("disabled").and_then(Value::as_bool).unwrap_or(false))
                    .filter_map(|p| {
                        let k = p.get("key").and_then(Value::as_str)?;
                        Some(format!("{k}={}", value_str(p.get("value"))))
                    })
                    .collect(),
                _ => Vec::new(),
            };
            if !query.is_empty() {
                t.push('?');
                t.push_str(&query.join("&"));
            }
            t
        }
    };
    if let Some(Value::Array(q)) = obj.get("query") {
        for p in q {
            if p.get("disabled").and_then(Value::as_bool).unwrap_or(false) {
                let key = p.get("key").and_then(Value::as_str).unwrap_or("");
                if !key.trim().is_empty() {
                    warnings.push(format!("disabled query parameter `{key}` was dropped"));
                }
            }
        }
    }
    if let Some(Value::Array(vars)) = obj.get("variable") {
        for v in vars {
            let Some(key) = v.get("key").and_then(Value::as_str) else {
                continue;
            };
            let value = value_str(v.get("value"));
            let replacement = if value.is_empty() {
                warnings.push(format!(
                    "path variable `:{key}` has no value; using {{{{{key}}}}} instead"
                ));
                format!("{{{{{key}}}}}")
            } else {
                value
            };
            text = replace_path_var(&text, key, &replacement);
        }
    }
    brace_placeholders(&text, warnings)
}

static BRACE_SEGMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/\{([A-Za-z_][A-Za-z0-9_]*)\}([/?#]|$)").unwrap());

/// Turns documentation-style `/{name}` path segments into `{{name}}` variables.
fn brace_placeholders(url: &str, warnings: &mut Vec<String>) -> String {
    let mut text = url.to_string();
    while let Some(c) = BRACE_SEGMENT.captures(&text) {
        let whole = c.get(0).unwrap().range();
        let name = c[1].to_string();
        let tail = c[2].to_string();
        warnings.push(format!(
            "URL placeholder `{{{name}}}` became the variable {{{{{name}}}}}"
        ));
        text.replace_range(whole, &format!("/{{{{{name}}}}}{tail}"));
    }
    text
}

/// Replaces `:key` path segments (followed by `/`, `?`, `#` or the end).
fn replace_path_var(url: &str, key: &str, value: &str) -> String {
    let needle = format!(":{key}");
    let mut out = String::new();
    let mut rest = url;
    while let Some(pos) = rest.find(&needle) {
        let after = &rest[pos + needle.len()..];
        let at_boundary = pos > 0 && rest[..pos].ends_with('/');
        let ends = after.is_empty() || after.starts_with(['/', '?', '#']);
        out.push_str(&rest[..pos]);
        if at_boundary && ends {
            out.push_str(value);
        } else {
            out.push_str(&needle);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn value_str(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn first_line(desc: &Value) -> Option<String> {
    let text = match desc {
        Value::String(s) => s.as_str(),
        Value::Object(o) => o.get("content").and_then(Value::as_str)?,
        _ => return None,
    };
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(line.chars().take(200).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_vars() {
        assert_eq!(
            replace_path_var("{{base}}/pets/:id/x?y=:id", "id", "{{petId}}"),
            "{{base}}/pets/{{petId}}/x?y=:id"
        );
        assert_eq!(replace_path_var("/a/:idx", "id", "1"), "/a/:idx");
    }

    #[test]
    fn brace_segments_become_variables() {
        let mut w = Vec::new();
        assert_eq!(
            brace_placeholders(
                "https://x/v2/accounts/{account_id}/docs/{doc}?a={b}",
                &mut w
            ),
            "https://x/v2/accounts/{{account_id}}/docs/{{doc}}?a={b}"
        );
        assert_eq!(w.len(), 2);
        assert_eq!(
            brace_placeholders("{{base}}/x/{{id}}", &mut w),
            "{{base}}/x/{{id}}"
        );
        assert_eq!(w.len(), 2);
    }

    #[test]
    fn rejects_other_formats() {
        let e = read(r#"{"values": []}"#, &[]).unwrap_err();
        assert!(e.to_string().contains("--env"), "{e}");
        let e = read(r#"{"requests": [], "order": []}"#, &[]).unwrap_err();
        assert!(e.to_string().contains("v1"), "{e}");
        let e = read("nope", &[]).unwrap_err();
        assert!(e.to_string().contains("JSON"), "{e}");
    }
}
