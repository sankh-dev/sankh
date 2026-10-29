mod import;
mod init;
mod output;
mod workspace;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use sankh_core::collection::{Collection, Node};
use sankh_core::redact::Redactor;
use sankh_core::report::{self, Summary};
use sankh_core::request::Severity;
use sankh_core::select::{self, Filters};
use sankh_core::{Env, RunContext, RunOptions, diag, runner};
use std::path::PathBuf;
use std::process::ExitCode;

/// Exit code for usage, parse and config errors; the others come from
/// [`report::exit_code`] and are documented in the README.
const EXIT_USAGE: u8 = 2;

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
    /// Check every selected file (flags, variables, referenced files) and
    /// send nothing
    #[arg(long)]
    check: bool,
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
    /// Only send requests to these hosts (repeatable): a name, `*.example.com`,
    /// an IP address or a CIDR range such as `10.0.0.0/8`
    #[arg(long = "allow-target")]
    allow_targets: Vec<String>,
    /// Never send requests to these hosts (repeatable; same forms as --allow-target)
    #[arg(long = "deny-target")]
    deny_targets: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Run(args) => cmd_run(args),
        Command::List { path, tags, json } => cmd_list(path, tags, json),
        Command::Init { path } => init::run(&path).map(|_| ExitCode::SUCCESS),
        Command::Serve(args) => cmd_serve(args),
        Command::Import(source) => import::run(source).map(|_| ExitCode::SUCCESS),
        Command::Workspace(action) => workspace::run(action).map(|_| ExitCode::SUCCESS),
        Command::Mcp { paths } => cmd_mcp(paths),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{} {e:#}", output::red("error:"));
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn cmd_run(args: RunArgs) -> Result<ExitCode> {
    let roots = Collection::roots_for(&args.path)?;
    if roots.len() > 1 {
        let mut lines = Vec::new();
        for root in &roots {
            let label = Collection::open(root)
                .map(|c| format!("{} ({})", c.name(), root.display()))
                .unwrap_or_else(|_| root.display().to_string());
            lines.push(format!("  {label}"));
        }
        bail!(
            "{} contains {} collections:\n{}\nRun each path on its own.",
            args.path.display(),
            roots.len(),
            lines.join("\n")
        );
    }
    let collection = Collection::open(&roots[0])?;
    let target = args.path.canonicalize()?;
    let env = Env::load(&collection, args.env.as_deref())?;
    let filters = Filters {
        folders: args.folders,
        tags: args.tags,
    };
    let files = select::select(&collection, &target, &filters)?;
    if files.is_empty() {
        bail!("no requests matched");
    }

    let checks = runner::check(&collection, &files, &env);
    let redactor = Redactor::from_vars(&env.resolved());
    let mut errors = 0;
    let mut bad_files = 0;
    for c in &checks {
        let shown: Vec<_> = c
            .diagnostics
            .iter()
            .filter(|d| args.check || d.severity == Severity::Error)
            .collect();
        for d in &shown {
            let text = diag::render(d, &c.path, &c.content, output::color_enabled());
            eprintln!("{}", redactor.redact(&text));
        }
        let n = shown
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        errors += n;
        bad_files += usize::from(n > 0);
    }
    if errors > 0 {
        eprintln!(
            "{} {errors} error{} in {bad_files} file{}; nothing was sent",
            output::red("error:"),
            if errors == 1 { "" } else { "s" },
            if bad_files == 1 { "" } else { "s" },
        );
        return Ok(ExitCode::from(EXIT_USAGE));
    }
    if args.check {
        println!(
            "{} {} request{} checked, no errors",
            output::green("✓"),
            files.len(),
            if files.len() == 1 { "" } else { "s" }
        );
        return Ok(ExitCode::SUCCESS);
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
        let result = runner::run_request(&collection, file, &mut ctx);
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

    Ok(ExitCode::from(report::exit_code(&results)))
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
        target_policy: Some(sankh_core::http::HostPolicy {
            allow: args.allow_targets,
            deny: args.deny_targets,
        }),
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
