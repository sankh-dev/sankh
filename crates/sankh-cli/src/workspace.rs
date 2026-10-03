//! `sankh workspace`: the saved list of collections `sankh serve` opens.

use crate::output;
use anyhow::Result;
use sankh_core::Collection;
use sankh_core::scratch;
use sankh_core::trust::{TrustStatus, TrustStore};
use sankh_core::workspace::Workspace;
use std::path::PathBuf;

#[derive(clap::Subcommand)]
pub enum Action {
    /// Add collection folders to the saved workspace
    Add {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Unlink a collection (by id or path); its files stay on disk
    #[command(alias = "unlink")]
    Remove { collection: String },
    /// Show the saved workspace
    List {
        /// Print as JSON
        #[arg(long)]
        json: bool,
    },
}

pub fn run(action: Action) -> Result<()> {
    match action {
        Action::Add { paths } => {
            let mut ws = Workspace::load()?;
            let mut added = Vec::new();
            for p in &paths {
                added.push(ws.add(p)?);
            }
            ws.save()?;
            for e in added {
                println!(
                    "{} added {}  {}",
                    output::green("✓"),
                    e.id,
                    output::dim(&e.path.display().to_string())
                );
            }
        }
        Action::Remove { collection } => {
            let mut ws = Workspace::load()?;
            let e = ws.remove(&collection)?;
            ws.save()?;
            println!(
                "unlinked {}  {}",
                e.id,
                output::dim(&format!("({} left in place)", e.path.display()))
            );
        }
        Action::List { json } => list(json)?,
    }
    Ok(())
}

fn list(json: bool) -> Result<()> {
    let ws = Workspace::load()?;
    let store = TrustStore::load().unwrap_or_default();
    let mut rows = vec![serde_json::json!({
        "id": scratch::ID,
        "path": scratch::dir().display().to_string(),
        "scratch": true,
        "missing": false,
        "trust": "trusted",
    })];
    for e in &ws.collections {
        let missing = Collection::open(&e.path).is_err();
        let trust = match store.status(&e.path) {
            TrustStatus::Trusted { .. } => "trusted",
            TrustStatus::Untrusted => "untrusted",
            TrustStatus::Changed { .. } => "changed",
        };
        rows.push(serde_json::json!({
            "id": e.id,
            "path": e.path.display().to_string(),
            "scratch": false,
            "missing": missing,
            "trust": if missing { "-" } else { trust },
        }));
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    for r in &rows {
        let id = r["id"].as_str().unwrap_or_default();
        let path = r["path"].as_str().unwrap_or_default();
        let note = if r["scratch"] == true {
            output::dim("(built in)")
        } else if r["missing"] == true {
            output::red("missing")
        } else {
            match r["trust"].as_str() {
                Some("trusted") => output::dim("trusted"),
                Some(t) => output::yellow(t),
                None => String::new(),
            }
        };
        println!("{} {path}  {note}", output::bold(&format!("{id:<20}")));
    }
    if ws.collections.is_empty() {
        println!(
            "\n{}",
            output::dim("add folders with `sankh workspace add PATH`")
        );
    }
    Ok(())
}
