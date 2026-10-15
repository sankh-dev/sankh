//! Merges converted environments into an existing collection.

use super::*;
use crate::collection::{Collection, ENV_DIR};
use crate::env::{self, ENV_LOCAL, EnvEditError};

/// What importing one environment does to its file.
#[derive(Debug, Clone, Serialize)]
pub struct EnvFilePlan {
    /// Name in the source export.
    pub source: String,
    /// Environment name in the collection.
    pub env: String,
    pub file: String,
    /// `"new"` or `"merge"`.
    pub status: &'static str,
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub unchanged: usize,
    #[serde(skip)]
    vars: Vec<(String, String)>,
    #[serde(skip)]
    header: Option<String>,
}

/// Every file an environment import writes, and what changes in each.
#[derive(Debug, Clone, Serialize)]
pub struct EnvImportPlan {
    pub envs: Vec<EnvFilePlan>,
    pub renamed: Vec<Rename>,
    /// Secret variables, left out of the env files.
    pub secrets: Vec<String>,
    /// Secrets that would be added to or changed in `.env.local`.
    pub local_added: Vec<String>,
    pub local_updated: Vec<String>,
    #[serde(skip)]
    local_vars: Vec<(String, String)>,
}

/// Works out the merge of `imported` into `collection` without writing anything.
pub fn plan_env_import(
    collection: &Collection,
    imported: &ImportedEnvs,
) -> Result<EnvImportPlan, EnvEditError> {
    let mut envs: Vec<EnvFilePlan> = Vec::new();
    for (i, e) in imported.environments.iter().enumerate() {
        let mut base = slug(&e.name);
        if !env::valid_env_name(&base) {
            base = format!("env-{}", i + 1);
        }
        let mut name = base.clone();
        let mut n = 2;
        while envs.iter().any(|p| p.env == name) {
            name = format!("{base}-{n}");
            n += 1;
        }
        let path = env::env_path(collection, &name);
        let exists = path.is_file();
        let existing = if exists {
            env::read_env_raw(&path)?
        } else {
            Vec::new()
        };
        let incoming = env::parse_env_raw(&dotenv_text(&e.vars));
        let merged = env::merge_env_vars(&existing, &incoming);
        envs.push(EnvFilePlan {
            source: e.name.clone(),
            file: format!("{ENV_DIR}/{name}.env"),
            env: name,
            status: if exists { "merge" } else { "new" },
            added: merged.added,
            updated: merged.updated,
            unchanged: merged.unchanged,
            vars: merged.vars,
            header: (!exists).then(|| {
                format!(
                    "# Imported from environment \"{}\"",
                    e.name.split_whitespace().collect::<Vec<_>>().join(" ")
                )
            }),
        });
    }
    let local = env::read_env_raw(&collection.root.join(ENV_LOCAL))?;
    let merged = env::merge_env_vars(&local, &env::parse_env_raw(&dotenv_text(&imported.secrets)));
    Ok(EnvImportPlan {
        envs,
        renamed: imported.renamed.clone(),
        secrets: imported.secrets.iter().map(|(k, _)| k.clone()).collect(),
        local_added: merged.added,
        local_updated: merged.updated,
        local_vars: merged.vars,
    })
}

/// Writes a plan from [`plan_env_import`]. Secret values go to `.env.local`
/// only with `secrets_to_local`. Sets `default_env` when the collection has none.
pub fn write_env_import(
    collection: &Collection,
    plan: &EnvImportPlan,
    secrets_to_local: bool,
) -> Result<(), EnvEditError> {
    for p in &plan.envs {
        let path = env::env_path(collection, &p.env);
        if let Some(header) = &p.header {
            std::fs::create_dir_all(collection.root.join(ENV_DIR))?;
            std::fs::write(&path, format!("{header}\n"))?;
        }
        env::write_env_file(&path, &p.vars)?;
    }
    if secrets_to_local && !plan.secrets.is_empty() {
        env::write_env_file(&collection.root.join(ENV_LOCAL), &plan.local_vars)?;
    }
    let default_ok = collection
        .config
        .default_env
        .as_deref()
        .is_some_and(|d| env::env_path(collection, d).is_file());
    if !default_ok {
        if let Some(first) = plan.envs.first() {
            env::set_default_env(collection, Some(&first.env))?;
        }
    }
    Ok(())
}

fn dotenv_text(vars: &[(String, String)]) -> String {
    vars.iter().map(|(k, v)| format!("{k}={v}\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV: &str = r#"{"name": "Dev", "values": [
        {"key": "baseUrl", "value": "http://new", "enabled": true},
        {"key": "petId", "value": "7", "enabled": true},
        {"key": "apiKey", "value": "s3cret", "type": "secret", "enabled": true},
        {"key": "off", "value": "x", "enabled": false}
    ]}"#;

    #[test]
    fn merges_into_existing_env_and_creates_new() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("sankh.toml"), "name = \"X\"\n").unwrap();
        std::fs::create_dir_all(root.join("environments")).unwrap();
        std::fs::write(
            root.join("environments/dev.env"),
            "# mine\nBASE_URL=http://old\nKEEP=1\n",
        )
        .unwrap();
        std::fs::write(root.join(".env.local"), "API_KEY=changeme\n").unwrap();
        let c = Collection::open(root).unwrap();

        let prod = r#"{"name": "Prod", "values": [{"key": "baseUrl", "value": "https://p"}]}"#;
        let imported = postman::read_envs(&[DEV, prod]).unwrap();
        let plan = plan_env_import(&c, &imported).unwrap();
        assert_eq!(plan.envs[0].env, "dev");
        assert_eq!(plan.envs[0].status, "merge");
        assert_eq!(plan.envs[0].added, vec!["PET_ID"]);
        assert_eq!(plan.envs[0].updated, vec!["BASE_URL"]);
        assert_eq!(plan.envs[1].status, "new");
        assert_eq!(plan.secrets, vec!["API_KEY"]);
        assert_eq!(plan.local_updated, vec!["API_KEY"]);

        write_env_import(&c, &plan, false).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("environments/dev.env")).unwrap(),
            "# mine\nBASE_URL=http://new\nKEEP=1\nPET_ID=7\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("environments/prod.env")).unwrap(),
            "# Imported from environment \"Prod\"\nBASE_URL=https://p\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(".env.local")).unwrap(),
            "API_KEY=changeme\n"
        );
        let c = Collection::open(root).unwrap();
        assert_eq!(c.config.default_env.as_deref(), Some("dev"));

        let plan = plan_env_import(&c, &postman::read_envs(&[DEV]).unwrap()).unwrap();
        assert!(plan.envs[0].added.is_empty() && plan.envs[0].updated.is_empty());
        write_env_import(&c, &plan, true).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join(".env.local")).unwrap(),
            "API_KEY=s3cret\n"
        );
    }

    #[test]
    fn rejects_a_collection_as_environment() {
        let e = postman::read_envs(&[r#"{"info": {}, "item": []}"#]).unwrap_err();
        assert!(e.to_string().contains("not a Postman environment"), "{e}");
    }
}
