//! `sankh import`: convert collections from other API clients.

use crate::output;
use anyhow::{Context, Result};
use sankh_core::import::{self, ImportReport, postman};
use std::path::{Path, PathBuf};

#[derive(clap::Subcommand)]
pub enum Source {
    /// Postman Collection v2.0 / v2.1 export (JSON)
    Postman(PostmanArgs),
}

#[derive(clap::Args)]
pub struct PostmanArgs {
    /// Exported collection file (`*.postman_collection.json`)
    file: PathBuf,
    /// Exported Postman environment to convert (repeatable)
    #[arg(long = "env")]
    envs: Vec<PathBuf>,
    /// Output folder; defaults to the collection name in the current folder
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Write into a non-empty output folder, overwriting files with the same name
    #[arg(long)]
    force: bool,
    /// Print the import report as JSON
    #[arg(long)]
    json: bool,
}

pub fn run(source: Source) -> Result<()> {
    match source {
        Source::Postman(args) => postman_cmd(args),
    }
}

fn postman_cmd(args: PostmanArgs) -> Result<()> {
    let text = read(&args.file)?;
    let envs = args
        .envs
        .iter()
        .map(|p| read(p))
        .collect::<Result<Vec<_>>>()?;
    let env_refs: Vec<&str> = envs.iter().map(String::as_str).collect();
    let collection = postman::read(&text, &env_refs)
        .with_context(|| format!("reading {}", args.file.display()))?;
    let mut rendered = import::render(&collection);
    let dir = args
        .output
        .unwrap_or_else(|| PathBuf::from(import::slug(&collection.name)));
    import::write(&mut rendered, &dir, args.force)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&rendered.report)?);
    } else {
        print_report(&rendered.report, &dir);
    }
    Ok(())
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

fn print_report(r: &ImportReport, dir: &Path) {
    let envs = if r.environments.is_empty() {
        String::new()
    } else {
        format!(", environments: {}", r.environments.join(", "))
    };
    println!(
        "{} imported \"{}\" into {}: {} request(s) in {} folder(s){envs}",
        output::green("✓"),
        r.collection,
        dir.display(),
        r.requests,
        r.folders,
    );
    if !r.renamed.is_empty() {
        let list: Vec<String> = r
            .renamed
            .iter()
            .map(|x| format!("{} -> {}", x.from, x.to))
            .collect();
        println!("\nrenamed variables: {}", list.join(", "));
    }
    if !r.placeholders.is_empty() {
        println!("\nset these in .env.local (see .env.example):");
        for p in &r.placeholders {
            println!("  {}  {}", output::bold(&p.name), output::dim(&p.note));
        }
    }
    if !r.warnings.is_empty() {
        println!(
            "\n{}",
            output::yellow(&format!("{} warning(s):", r.warnings.len()))
        );
        for w in &r.warnings {
            let path = if w.path.is_empty() {
                "collection"
            } else {
                &w.path
            };
            println!("  {}  {}", output::dim(path), w.message);
        }
    }
    println!(
        "\nNext: review the files, then `sankh trust {0}` and `sankh run {0}`",
        dir.display()
    );
}
