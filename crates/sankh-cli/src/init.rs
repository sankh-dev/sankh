//! `sankh init`: scaffold a collection.

use anyhow::{Result, bail};
use std::path::Path;

const FILES: &[(&str, &str)] = &[
    (
        "sankh.toml",
        "# Sankh collection settings (all optional)\nformat = 1\ndefault_env = \"dev\"\n# timeout = 30\n",
    ),
    ("environments/dev.env", "BASE_URL=http://localhost:8080\n"),
    (
        ".env.example",
        "# Copy to .env.local (gitignored) for secrets\n# TOKEN=\n",
    ),
    (".gitignore", ".env.local\nsankh-report.*\n"),
    (
        "health/01-ping.sh",
        "#!/usr/bin/env bash\n# @name Ping\n# @tags smoke\n# @expect status 2xx\ncurl -sS \"$BASE_URL/\"\n",
    ),
];

pub fn run(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    let mut created = 0;
    for (rel, content) in FILES {
        let p = path.join(rel);
        if p.exists() {
            println!("skip   {rel} (exists)");
            continue;
        }
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&p, content)?;
        println!("create {rel}");
        created += 1;
    }
    if created == 0 {
        bail!("{} already looks like a collection", path.display());
    }
    println!(
        "\nNext: edit environments/dev.env, then `sankh trust {0}` and `sankh run {0}`",
        path.display()
    );
    Ok(())
}
