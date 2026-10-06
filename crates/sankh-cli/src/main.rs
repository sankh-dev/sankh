mod import;
mod init;
mod output;
mod workspace;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use sankh_core::collection::{Collection, Node};
use sankh_core::report::{self, Summary};
use sankh_core::select::{self, Filters};
use sankh_core::trust::{self, TrustError, TrustStatus, TrustStore};
use sankh_core::{Env, RunContext, RunOptions, runner};
use std::path::PathBuf;
use std::process::ExitCode;

/// Exit codes, documented in the README.
const EXIT_FAILED: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_UNTRUSTED: u8 = 3;

#[derive(Parser)]
#[command(name = "sankh", version, about = "Blow the conch. Run your APIs.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a request file or every request in a folder
    Run(RunArgs),
    /// List requests in a collection
    List {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Only requests with any of these tags
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Print the tree as JSON
        #[arg(long)]
        json: bool,
    },
    /// Trust a folder so its request files may run
    Trust {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Remove trust instead
        #[arg(long, conflicts_with = "list")]
        revoke: bool,
        /// Show all trusted folders
        #[arg(long)]
        list: bool,
    },
    /// Create a new collection skeleton
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Start the web UI for the saved workspace, or for the given folders
    Serve(ServeArgs),
    /// Manage the saved workspace that `sankh serve` opens
    #[command(subcommand)]
    Workspace(workspace::Action),
    /// Convert a collection from another API client into a Sankh folder
    #[command(subcommand)]
    Import(import::Source),
    /// Serve collections to AI agents over MCP (stdio); read and run only
    Mcp {
        /// Collection folders to expose; without any, the saved workspace
        paths: Vec<PathBuf>,
    },
}

#[derive(clap::Args)]
struct RunArgs {
    /// Request file or folder
    #[arg(default_value = ".")]
    path: PathBuf,
    /// Environment name (environments/<name>.env)
    #[arg(short, long)]
    env: Option<String>,
    /// Only requests inside these folders (repeatable)
    #[arg(long = "folder", conflicts_with = "all")]
    folders: Vec<String>,
    /// Only requests with any of these tags (repeatable)
    #[arg(long = "tag", conflicts_with = "all")]
    tags: Vec<String>,
    /// Run every request (the default when no filters are given)
    #[arg(long)]
    all: bool,
    /// Run without a trust entry (for CI)
    #[arg(long, env = "SANKH_TRUST", value_parser = clap::builder::BoolishValueParser::new(), default_value_t = false)]
    trust: bool,
    /// Write a machine-readable report
    #[arg(long, value_enum)]
    report: Option<ReportFormat>,
    /// Report destination (`-` for stdout); defaults to sankh-report.<ext>
    #[arg(long, requires = "report")]
    report_file: Option<PathBuf>,
    /// Stop at the first failure
    #[arg(long)]
    bail: bool,
    /// Print response bodies for every request, not only failures
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum ReportFormat {
    Junit,
    Json,
}

#[derive(clap::Args)]
struct ServeArgs {
    /// Collection folders for this session only; without any, the saved
    /// workspace is opened
    paths: Vec<PathBuf>,
    /// Address to bind; anything other than loopback requires --token
    #[arg(long, default_value = "127.0.0.1")]
    listen: String,
    #[arg(short, long, default_value_t = 4747)]
    port: u16,
    /// Bearer token required for API access
    #[arg(long, env = "SANKH_TOKEN", hide_env_values = true)]
    token: Option<String>,
    /// Extra Host header values to accept (e.g. a reverse proxy hostname)
    #[arg(long = "allow-host")]
    allow_hosts: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(args) => cmd_run(args),
        Command::List { path, tags, json } => cmd_list(path, tags, json),
        Command::Trust { path, revoke, list } => cmd_trust(path, revoke, list),
        Command::Init { path } => init::run(&path).map(|_| ExitCode::SUCCESS),
        Command::Serve(args) => cmd_serve(args),
        Command::Import(source) => import::run(source).map(|_| ExitCode::SUCCESS),
        Command::Workspace(action) => workspace::run(action).map(|_| ExitCode::SUCCESS),
        Command::Mcp { paths } => cmd_mcp(paths),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            if let Some(t) = e.downcast_ref::<TrustError>() {
                eprintln!("{} {t}", output::red("error:"));
                return ExitCode::from(EXIT_UNTRUSTED);
            }
            eprintln!("{} {e:#}", output::red("error:"));
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn cmd_run(args: RunArgs) -> Result<ExitCode> {
    let (collection, target) = Collection::discover(&args.path)?;
    let trusted = trust::ensure(&collection, args.trust)?;
    let env = Env::load(&collection, args.env.as_deref())?;
    let filters = Filters {
        folders: args.folders,
        tags: args.tags,
    };
    let files = select::select(&collection, &target, &filters)?;
    if files.is_empty() {
        bail!("no requests matched");
    }

    let report_to_stdout = args.report_file.as_deref() == Some(std::path::Path::new("-"));
    let out = output::Printer::new(report_to_stdout, args.verbose);
    out.header(
        &collection.name(),
        args.env
            .as_deref()
            .or(collection.config.default_env.as_deref()),
        files.len(),
    );

    let mut ctx = RunContext::new(env, RunOptions::for_collection(&collection));
    let mut results = Vec::new();
    for file in &files {
        let result = runner::run_request(&trusted, &collection, file, &mut ctx);
        out.result(&result);
        let failed = result.outcome != sankh_core::Outcome::Passed;
        results.push(result);
        if failed && args.bail {
            break;
        }
    }
    let summary = Summary::of(&results);
    out.summary(&summary);

    if let Some(format) = args.report {
        let (text, ext) = match format {
            ReportFormat::Junit => (report::junit(&collection.name(), &results), "xml"),
            ReportFormat::Json => (report::json(&collection.name(), &results), "json"),
        };
        if report_to_stdout {
            print!("{text}");
        } else {
            let path = args
                .report_file
                .unwrap_or_else(|| PathBuf::from(format!("sankh-report.{ext}")));
            std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
            out.note(&format!("report written to {}", path.display()));
        }
    }

    Ok(if summary.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_FAILED)
    })
}

fn cmd_list(path: PathBuf, tags: Vec<String>, json: bool) -> Result<ExitCode> {
    let (collection, _) = Collection::discover(&path)?;
    let tree = collection.tree()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&tree)?);
        return Ok(ExitCode::SUCCESS);
    }
    fn walk(node: &Node, depth: usize, tags: &[String]) {
        let indent = "  ".repeat(depth);
        match node {
            Node::Folder { name, children, .. } => {
                println!("{indent}{}", output::bold(&format!("{name}/")));
                for c in children {
                    walk(c, depth + 1, tags);
                }
            }
            Node::Request {
                name,
                path,
                tags: t,
                method,
                raw,
                errors,
            } => {
                if !tags.is_empty() && !tags.iter().any(|x| t.contains(x)) {
                    return;
                }
                let method = method.as_deref().unwrap_or(if *raw { "RAW" } else { "?" });
                let mut line = format!("{indent}{:<6} {name}  {}", method, output::dim(path));
                if !t.is_empty() {
                    line.push_str(&format!("  {}", output::dim(&format!("[{}]", t.join(" ")))));
                }
                if *errors > 0 {
                    line.push_str(&format!("  {}", output::red(&format!("{errors} error(s)"))));
                }
                println!("{line}");
            }
        }
    }
    walk(&tree, 0, &tags);
    Ok(ExitCode::SUCCESS)
}

fn cmd_trust(path: PathBuf, revoke: bool, list: bool) -> Result<ExitCode> {
    let mut store = TrustStore::load()?;
    if list {
        if store.folders.is_empty() {
            println!("no trusted folders");
        }
        for (folder, entry) in &store.folders {
            let head = entry
                .head
                .as_deref()
                .map(|h| &h[..h.len().min(8)])
                .unwrap_or("-");
            println!("{folder}  {}", output::dim(&format!("(git {head})")));
        }
        return Ok(ExitCode::SUCCESS);
    }
    let folder = path
        .canonicalize()
        .with_context(|| format!("{} not found", path.display()))?;
    let folder = if folder.is_file() {
        folder.parent().map(PathBuf::from).unwrap_or(folder)
    } else {
        folder
    };
    if revoke {
        if store.revoke(&folder) {
            store.save()?;
            println!("revoked trust for {}", folder.display());
        } else {
            println!("{} was not trusted", folder.display());
        }
        return Ok(ExitCode::SUCCESS);
    }
    if let TrustStatus::Trusted { path } = store.status(&folder) {
        if path != folder.to_string_lossy() {
            println!("already trusted via {path}");
            return Ok(ExitCode::SUCCESS);
        }
    }
    let key = store.trust(&folder)?;
    store.save()?;
    println!("{} trusted {key}", output::green("✓"));
    Ok(ExitCode::SUCCESS)
}

fn cmd_serve(args: ServeArgs) -> Result<ExitCode> {
    let workspace = if args.paths.is_empty() {
        let saved = sankh_core::workspace::Workspace::load()?;
        let cwd = std::path::Path::new(".");
        if saved.collections.is_empty()
            && (cwd.join(sankh_core::collection::CONFIG_FILE).is_file()
                || cwd.join(sankh_core::collection::ENV_DIR).is_dir())
        {
            println!(
                "{}",
                output::dim(
                    "this folder looks like a collection: `sankh workspace add .` keeps it in the workspace, or `sankh serve .` opens it for this session"
                )
            );
        }
        sankh_server::WorkspaceSource::Saved
    } else {
        sankh_server::WorkspaceSource::Session(args.paths)
    };
    let config = sankh_server::ServeConfig {
        workspace,
        listen: args.listen,
        port: args.port,
        token: args.token.filter(|t| !t.is_empty()),
        allow_hosts: args.allow_hosts,
    };
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(sankh_server::serve(config))?;
    Ok(ExitCode::SUCCESS)
}

/// Stdout carries the protocol, so nothing else may print there.
fn cmd_mcp(paths: Vec<PathBuf>) -> Result<ExitCode> {
    let source = if paths.is_empty() {
        sankh_mcp::WorkspaceSource::Saved
    } else {
        sankh_mcp::WorkspaceSource::Session(paths)
    };
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(sankh_mcp::serve_stdio(source))?;
    Ok(ExitCode::SUCCESS)
}
