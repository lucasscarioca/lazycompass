use anyhow::{Context, Result};
use lazycompass_core::{Config, SavedAggregation, SavedQuery};
use std::fs;
use std::path::{Path, PathBuf};

const APP_DIR: &str = "lazycompass";

#[derive(Debug, Clone)]
pub struct ConfigPaths {
    pub global_root: PathBuf,
    pub repo_root: Option<PathBuf>,
}

impl ConfigPaths {
    pub fn resolve_from(cwd: impl AsRef<Path>) -> Result<Self> {
        let cwd = cwd.as_ref();
        let global_root = dirs::config_dir()
            .map(|path| path.join(APP_DIR))
            .context("unable to resolve user config directory")?;
        let repo_root = find_repo_root(cwd);

        Ok(Self {
            global_root,
            repo_root,
        })
    }

    pub fn global_config_path(&self) -> PathBuf {
        self.global_root.join("config.toml")
    }

    pub fn global_queries_dir(&self) -> PathBuf {
        self.global_root.join("queries")
    }

    pub fn global_aggregations_dir(&self) -> PathBuf {
        self.global_root.join("aggregations")
    }

    pub fn repo_config_root(&self) -> Option<PathBuf> {
        self.repo_root
            .as_ref()
            .map(|root| root.join(".lazycompass"))
    }

    pub fn repo_config_path(&self) -> Option<PathBuf> {
        self.repo_config_root().map(|root| root.join("config.toml"))
    }

    pub fn repo_queries_dir(&self) -> Option<PathBuf> {
        self.repo_config_root().map(|root| root.join("queries"))
    }

    pub fn repo_aggregations_dir(&self) -> Option<PathBuf> {
        self.repo_config_root()
            .map(|root| root.join("aggregations"))
    }
}

#[derive(Debug, Clone)]
pub struct StorageSnapshot {
    pub config: Config,
    pub queries: Vec<SavedQuery>,
    pub aggregations: Vec<SavedAggregation>,
}

pub fn load_storage(paths: &ConfigPaths) -> Result<StorageSnapshot> {
    Ok(StorageSnapshot {
        config: load_config(paths)?,
        queries: load_saved_queries(paths)?,
        aggregations: load_saved_aggregations(paths)?,
    })
}

pub fn load_config(paths: &ConfigPaths) -> Result<Config> {
    let global = read_config(&paths.global_config_path())?;
    let repo = match paths.repo_config_path() {
        Some(path) => read_config(&path)?,
        None => Config::default(),
    };

    Ok(merge_config(global, repo))
}

pub fn load_saved_queries(paths: &ConfigPaths) -> Result<Vec<SavedQuery>> {
    let Some(dir) = paths.repo_queries_dir() else {
        return Ok(Vec::new());
    };

    load_queries_from_dir(&dir)
}

pub fn load_saved_aggregations(paths: &ConfigPaths) -> Result<Vec<SavedAggregation>> {
    let Some(dir) = paths.repo_aggregations_dir() else {
        return Ok(Vec::new());
    };

    load_aggregations_from_dir(&dir)
}

fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);

    while let Some(dir) = current {
        if dir.join(".lazycompass").is_dir() {
            return Some(dir.to_path_buf());
        }
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }

    None
}

fn read_config(path: &Path) -> Result<Config> {
    if !path.is_file() {
        return Ok(Config::default());
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("unable to read config file {}", path.display()))?;
    let config: Config = toml::from_str(&contents)
        .with_context(|| format!("invalid TOML in config file {}", path.display()))?;
    validate_config(&config)
        .with_context(|| format!("invalid config data in {}", path.display()))?;
    Ok(config)
}

fn validate_config(config: &Config) -> Result<()> {
    for (index, connection) in config.connections.iter().enumerate() {
        if connection.name.trim().is_empty() {
            anyhow::bail!("connection at index {} has empty name", index);
        }
        if connection.uri.trim().is_empty() {
            anyhow::bail!("connection '{}' has empty uri", connection.name);
        }
    }
    Ok(())
}

fn merge_config(global: Config, repo: Config) -> Config {
    let mut connections = global.connections;
    for repo_connection in repo.connections {
        if let Some(existing) = connections
            .iter_mut()
            .find(|connection| connection.name == repo_connection.name)
        {
            *existing = repo_connection;
        } else {
            connections.push(repo_connection);
        }
    }

    Config { connections }
}

fn load_queries_from_dir(dir: &Path) -> Result<Vec<SavedQuery>> {
    let paths = collect_toml_paths(dir)?;
    let mut queries = Vec::with_capacity(paths.len());

    for path in paths {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("unable to read saved query file {}", path.display()))?;
        let query: SavedQuery = toml::from_str(&contents)
            .with_context(|| format!("invalid TOML in saved query {}", path.display()))?;
        query
            .validate()
            .with_context(|| format!("invalid saved query {}", path.display()))?;
        queries.push(query);
    }

    Ok(queries)
}

fn load_aggregations_from_dir(dir: &Path) -> Result<Vec<SavedAggregation>> {
    let paths = collect_toml_paths(dir)?;
    let mut aggregations = Vec::with_capacity(paths.len());

    for path in paths {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("unable to read saved aggregation file {}", path.display()))?;
        let aggregation: SavedAggregation = toml::from_str(&contents)
            .with_context(|| format!("invalid TOML in saved aggregation {}", path.display()))?;
        aggregation
            .validate()
            .with_context(|| format!("invalid saved aggregation {}", path.display()))?;
        aggregations.push(aggregation);
    }

    Ok(aggregations)
}

fn collect_toml_paths(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("unable to read directory {}", dir.display()))?
    {
        let entry = entry
            .with_context(|| format!("unable to read directory entry in {}", dir.display()))?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("toml") {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::collections::HashMap;
    use std::path::Path;

    fn temp_root(prefix: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pid = std::process::id();
        dir.push(format!("lazycompass_test_{prefix}_{pid}_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn load_config_merges_repo_overrides() -> Result<()> {
        let root = temp_root("config_merge");
        let global_root = root.join("global");
        let repo_root = root.join("repo");

        write_file(
            &global_root.join("config.toml"),
            r#"[[connections]]
name = "shared"
uri = "mongodb://global"
default_database = "global_db"

[[connections]]
name = "global_only"
uri = "mongodb://global_only"
"#,
        );
        write_file(
            &repo_root.join(".lazycompass/config.toml"),
            r#"[[connections]]
name = "shared"
uri = "mongodb://repo"
default_database = "repo_db"

[[connections]]
name = "repo_only"
uri = "mongodb://repo_only"
"#,
        );

        let paths = ConfigPaths {
            global_root,
            repo_root: Some(repo_root),
        };
        let config = load_config(&paths)?;
        let connections: HashMap<_, _> = config
            .connections
            .into_iter()
            .map(|connection| (connection.name.clone(), connection))
            .collect();

        assert_eq!(connections.len(), 3);
        assert_eq!(connections.get("shared").unwrap().uri, "mongodb://repo");
        assert_eq!(
            connections
                .get("shared")
                .unwrap()
                .default_database
                .as_deref(),
            Some("repo_db")
        );
        assert!(connections.contains_key("global_only"));
        assert!(connections.contains_key("repo_only"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_saved_specs_from_repo() -> Result<()> {
        let root = temp_root("saved_specs");
        let repo_root = root.join("repo");

        write_file(
            &repo_root.join(".lazycompass/queries/active_users.toml"),
            r#"name = "active_users"
connection = "local"
database = "lazycompass"
collection = "users"
filter = "{ \"active\": true }"
"#,
        );
        write_file(
            &repo_root.join(".lazycompass/aggregations/orders_by_user.toml"),
            r#"name = "orders_by_user"
connection = "local"
database = "lazycompass"
collection = "orders"
pipeline = "[ { \"$group\": { \"_id\": \"$userId\" } } ]"
"#,
        );

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let queries = load_saved_queries(&paths)?;
        let aggregations = load_saved_aggregations(&paths)?;

        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].name, "active_users");
        assert_eq!(aggregations.len(), 1);
        assert_eq!(aggregations[0].name, "orders_by_user");

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
