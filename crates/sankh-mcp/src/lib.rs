//! `sankh mcp`: a Model Context Protocol server over stdio, so AI agents can
//! list, inspect and run collections.
//!
//! It is read and run only: it never writes request files, environments or
//! the workspace, and it never grants trust. Running goes through the same
//! `trust::ensure` and redaction as `sankh run`, without the CI override.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ServerHandler, ServiceExt, schemars, tool, tool_handler, tool_router};
use sankh_core::collection::Node;
use sankh_core::env::{self, ENV_LOCAL};
use sankh_core::redact::Redactor;
use sankh_core::report::{self, Summary};
use sankh_core::select::{self, Filters};
use sankh_core::trust::{self, TrustError, TrustStore};
use sankh_core::workspace::{Opened, Workspace};
use sankh_core::{
    Collection, Env, Outcome, RequestResult, RunContext, RunOptions, runner, scratch,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

/// Response bodies longer than this are cut in tool results, to keep them
/// within an agent's context. Assertions still see the full body.
const MAX_BODY: usize = 16 * 1024;
const MAX_STDERR: usize = 4 * 1024;

/// Which collections the server exposes.
pub enum WorkspaceSource {
    /// The saved workspace, re-read on every call, plus Scratch if it exists.
    Saved,
    /// These folders only, plus Scratch if it exists.
    Session(Vec<PathBuf>),
}

enum Source {
    Saved,
    Session(Workspace),
}

#[derive(Clone)]
pub struct SankhMcp {
    source: Arc<Source>,
    tool_router: ToolRouter<SankhMcp>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CollectionArgs {
    /// Collection id from `list_collections`.
    pub collection: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListRequestsArgs {
    /// Collection id from `list_collections`.
    pub collection: String,
    /// Only requests with any of these tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ShowRequestArgs {
    /// Collection id from `list_collections`.
    pub collection: String,
    /// Request file relative to the collection root, e.g. `users/01-list.sh`.
    pub path: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RunArgs {
    /// Collection id from `list_collections`.
    pub collection: String,
    /// Request file or folder relative to the collection root. Default: the whole collection.
    #[serde(default)]
    pub path: Option<String>,
    /// Environment name (`environments/<name>.env`). Default: the collection's `default_env`.
    #[serde(default)]
    pub env: Option<String>,
    /// Only requests inside any of these folders.
    #[serde(default)]
    pub folders: Vec<String>,
    /// Only requests with any of these tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Stop at the first failing request.
    #[serde(default)]
    pub bail: bool,
}

impl SankhMcp {
    pub fn new(source: WorkspaceSource) -> anyhow::Result<SankhMcp> {
        let source = match source {
            WorkspaceSource::Saved => Source::Saved,
            WorkspaceSource::Session(paths) => {
                let mut ws = Workspace::default();
                for p in &paths {
                    ws.add(p)?;
                }
                Source::Session(ws)
            }
        };
        Ok(SankhMcp {
            source: Arc::new(source),
            tool_router: Self::tool_router(),
        })
    }

    /// Scratch (only if it already exists: the server never creates it) and
    /// every workspace entry.
    fn opened(&self) -> Result<Vec<Opened>, String> {
        let entries = match &*self.source {
            Source::Saved => Workspace::load().map_err(|e| e.to_string())?.open_entries(),
            Source::Session(ws) => ws.open_entries(),
        };
        let dir = scratch::dir();
        let scratch = dir.is_dir().then(|| Opened {
            id: scratch::ID.to_string(),
            collection: Collection::open(&dir).map_err(|e| e.to_string()),
            path: dir,
        });
        Ok(scratch.into_iter().chain(entries).collect())
    }

    fn collection(&self, id: &str) -> Result<Collection, String> {
        let opened = self.opened()?;
        let ids: Vec<&str> = opened.iter().map(|o| o.id.as_str()).collect();
        let Some(found) = opened.iter().find(|o| o.id == id) else {
            return Err(format!(
                "no collection `{id}` (available: {}); call list_collections",
                if ids.is_empty() {
                    "none".to_string()
                } else {
                    ids.join(", ")
                }
            ));
        };
        found
            .collection
            .clone()
            .map_err(|e| format!("collection `{id}` cannot be opened: {e}"))
    }
}

#[tool_router]
impl SankhMcp {
    /// List the collections this server exposes, with their id, name, root
    /// folder, environments and trust status. Use the id in other tools.
    #[tool(annotations(
        title = "List collections",
        read_only_hint = true,
        open_world_hint = false
    ))]
    fn list_collections(&self) -> Result<String, String> {
        let store = TrustStore::load().map_err(|e| e.to_string())?;
        let list: Vec<Value> = self
            .opened()?
            .iter()
            .map(|o| match &o.collection {
                Ok(c) => json!({
                    "id": o.id,
                    "name": c.name(),
                    "root": c.root.display().to_string(),
                    "missing": false,
                    "trust": store.status(&c.root),
                    "default_env": c.config.default_env,
                    "envs": env::list_envs(c),
                }),
                Err(e) => json!({
                    "id": o.id,
                    "name": o.id,
                    "root": o.path.display().to_string(),
                    "missing": true,
                    "error": e,
                }),
            })
            .collect();
        pretty(&json!({ "collections": list }))
    }

    /// The request tree of a collection: folders and request files with
    /// name, path, tags, HTTP method, raw mode and parse error count.
    #[tool(annotations(
        title = "List requests",
        read_only_hint = true,
        open_world_hint = false
    ))]
    fn list_requests(
        &self,
        Parameters(args): Parameters<ListRequestsArgs>,
    ) -> Result<String, String> {
        let c = self.collection(&args.collection)?;
        let tree = c.tree().map_err(|e| e.to_string())?;
        let tree = if args.tags.is_empty() {
            tree
        } else {
            filter_tags(tree, &args.tags, true).unwrap_or(Node::Folder {
                name: c.name(),
                path: String::new(),
                children: vec![],
            })
        };
        pretty(&tree)
    }

    /// A request file's text and its parsed annotations (name, tags,
    /// expectations, captures, warnings and errors). Secret values from the
    /// default environment are masked.
    #[tool(annotations(title = "Show request", read_only_hint = true, open_world_hint = false))]
    fn show_request(
        &self,
        Parameters(args): Parameters<ShowRequestArgs>,
    ) -> Result<String, String> {
        let c = self.collection(&args.collection)?;
        if !args.path.ends_with(".sh") {
            return Err("request files must end in .sh".into());
        }
        let abs = c.resolve(&args.path).map_err(|e| e.to_string())?;
        let content =
            std::fs::read_to_string(&abs).map_err(|e| format!("cannot read {}: {e}", args.path))?;
        let content = match Env::load(&c, None) {
            Ok(env) => Redactor::from_vars(&env.resolved()).redact(&content),
            Err(_) => content,
        };
        let req = c.parse_file(&abs, &content);
        pretty(&json!({ "path": args.path, "content": content, "request": req }))
    }

    /// Environment names, the default environment, and each environment's
    /// variables (file values merged with `.env.local`; secrets masked).
    /// Process environment variables also apply at run time but are not listed.
    #[tool(annotations(
        title = "List environments",
        read_only_hint = true,
        open_world_hint = false
    ))]
    fn list_environments(
        &self,
        Parameters(args): Parameters<CollectionArgs>,
    ) -> Result<String, String> {
        let c = self.collection(&args.collection)?;
        let envs: Vec<Value> = env::list_envs(&c)
            .into_iter()
            .map(|name| match Env::load(&c, Some(&name)) {
                Ok(e) => {
                    let r = Redactor::from_vars(&e.file_vars);
                    let vars: Vec<Value> = e
                        .file_vars
                        .iter()
                        .map(|(k, v)| json!({ "name": k, "value": r.display_var(k, v) }))
                        .collect();
                    json!({ "name": name, "vars": vars })
                }
                Err(err) => json!({ "name": name, "error": err.to_string() }),
            })
            .collect();
        pretty(&json!({
            "default": c.config.default_env,
            "env_local": c.root.join(ENV_LOCAL).is_file(),
            "envs": envs,
        }))
    }

    /// Run a request file, a folder, or the whole collection, and return a
    /// summary plus the JSON report (outcome, assertions, captures and the
    /// response for each request). Requests in one call chain captured values
    /// like `sankh run`; each call starts fresh. Refuses collections the user
    /// has not trusted with `sankh trust`.
    #[tool(annotations(
        title = "Run requests",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = true
    ))]
    async fn run(&self, Parameters(args): Parameters<RunArgs>) -> Result<CallToolResult, String> {
        let c = self.collection(&args.collection)?;
        tokio::task::spawn_blocking(move || run_blocking(&c, args))
            .await
            .map_err(|e| format!("run failed: {e}"))?
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "sankh",
    instructions = "Sankh runs API collections: folders of .sh files, each one curl command with # @expect / # @capture annotations. Call list_collections first and use its ids. Request files are shell scripts: a collection only runs after the user runs `sankh trust <path>`; this server never grants trust. Secret values appear as ***."
)]
impl ServerHandler for SankhMcp {}

fn run_blocking(c: &Collection, args: RunArgs) -> Result<CallToolResult, String> {
    let trusted = trust::ensure(c, false).map_err(|e| trust_message(&e))?;
    let rel = args.path.as_deref().unwrap_or("");
    let target = c.resolve(rel).map_err(|e| e.to_string())?;
    if !target.exists() {
        return Err(format!("{rel} not found in the collection"));
    }
    let env_name = args.env.filter(|e| !e.is_empty());
    let env = Env::load(c, env_name.as_deref()).map_err(|e| e.to_string())?;
    let filters = Filters {
        folders: args.folders,
        tags: args.tags,
    };
    let files = select::select(c, &target, &filters).map_err(|e| e.to_string())?;
    if files.is_empty() {
        return Err("no requests matched".into());
    }

    let mut ctx = RunContext::new(env, RunOptions::for_collection(c));
    let mut results = Vec::new();
    for file in &files {
        let mut result = runner::run_request(&trusted, c, file, &mut ctx);
        let failed = result.outcome != Outcome::Passed;
        shrink(&mut result);
        results.push(result);
        if failed && args.bail {
            break;
        }
    }

    let used_env = env_name.or_else(|| c.config.default_env.clone());
    let summary = Summary::of(&results);
    let report = json!({
        "collection": c.name(),
        "env": used_env,
        "summary": summary,
        "results": results,
    });
    let text = summary_text(&c.name(), used_env.as_deref(), &summary, &results);
    Ok(CallToolResult::success(vec![
        ContentBlock::text(text),
        ContentBlock::text(pretty(&report)?),
    ]))
}

fn trust_message(e: &TrustError) -> String {
    const NOTE: &str = "Request files are shell scripts: ask the user to review them and run the command themselves. This server never grants trust.";
    match e {
        TrustError::Untrusted(path) => {
            format!("{path} is not trusted. {NOTE} Command: `sankh trust {path}`")
        }
        TrustError::Changed {
            path,
            old,
            new,
            changed,
        } => format!(
            "{path} changed since it was trusted (git {old} -> {new}{}). {NOTE} Command: `sankh trust {path}`",
            trust::changed_summary(changed)
        ),
        TrustError::Store { .. } => e.to_string(),
    }
}

fn summary_text(name: &str, env: Option<&str>, s: &Summary, results: &[RequestResult]) -> String {
    let mut out = format!(
        "{name} (env {}): {} passed, {} failed, {} errors of {} in {:.0} ms",
        env.unwrap_or("none"),
        s.passed,
        s.failed,
        s.errors,
        s.total,
        s.duration_ms
    );
    for r in results.iter().filter(|r| r.outcome != Outcome::Passed) {
        let details = report::failure_details(r);
        out.push_str(&format!(
            "\n- {} ({}): {}",
            r.path,
            r.name,
            details.replace('\n', "; ")
        ));
    }
    out
}

/// Cuts long bodies and stderr so results fit an agent's context.
fn shrink(r: &mut RequestResult) {
    if let Some(resp) = r.response.as_mut() {
        if truncate(&mut resp.body, MAX_BODY) {
            resp.body_truncated = true;
        }
    }
    truncate(&mut r.stderr, MAX_STDERR);
}

fn truncate(s: &mut String, max: usize) -> bool {
    if s.len() <= max {
        return false;
    }
    let mut cut = max;
    while !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
    true
}

/// Keeps requests with any of `tags` and the folders that contain them.
fn filter_tags(node: Node, tags: &[String], root: bool) -> Option<Node> {
    match node {
        Node::Folder {
            name,
            path,
            children,
        } => {
            let children: Vec<Node> = children
                .into_iter()
                .filter_map(|c| filter_tags(c, tags, false))
                .collect();
            (root || !children.is_empty()).then_some(Node::Folder {
                name,
                path,
                children,
            })
        }
        Node::Request { tags: ref t, .. } => tags.iter().any(|x| t.contains(x)).then_some(node),
    }
}

fn pretty(v: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string_pretty(v).map_err(|e| e.to_string())
}

/// Serves MCP on stdin/stdout until the client disconnects.
pub async fn serve_stdio(source: WorkspaceSource) -> anyhow::Result<()> {
    let server = SankhMcp::new(source)?;
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(path: &str, tags: &[&str]) -> Node {
        Node::Request {
            name: path.into(),
            path: path.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            method: Some("GET".into()),
            raw: false,
            errors: 0,
        }
    }

    #[test]
    fn truncates_on_char_boundaries() {
        let mut s = "aé".repeat(10);
        assert!(truncate(&mut s, 4));
        assert_eq!(s, "aéa");
        let mut short = "abc".to_string();
        assert!(!truncate(&mut short, 4));
        assert_eq!(short, "abc");
    }

    #[test]
    fn tag_filter_drops_empty_folders_but_keeps_root() {
        let tree = Node::Folder {
            name: "api".into(),
            path: String::new(),
            children: vec![
                Node::Folder {
                    name: "users".into(),
                    path: "users".into(),
                    children: vec![
                        req("users/01-list.sh", &["smoke"]),
                        req("users/02-create.sh", &[]),
                    ],
                },
                Node::Folder {
                    name: "admin".into(),
                    path: "admin".into(),
                    children: vec![req("admin/01-x.sh", &["slow"])],
                },
            ],
        };
        let out =
            serde_json::to_value(filter_tags(tree, &["smoke".into()], true).unwrap()).unwrap();
        assert_eq!(out["children"].as_array().unwrap().len(), 1);
        assert_eq!(
            out["children"][0]["children"][0]["path"],
            "users/01-list.sh"
        );
        assert_eq!(out["children"][0]["children"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn trust_errors_never_suggest_the_ci_override() {
        let msg = trust_message(&TrustError::Untrusted("/x".into()));
        assert!(msg.contains("sankh trust /x"));
        assert!(!msg.contains("--trust"));
    }
}
