use anyhow::{Context, Result};
use lazycompass_core::{Config, LoggingConfig, SavedAggregation, SavedQuery, TimeoutConfig};
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
    pub warnings: Vec<String>,
}

pub fn load_storage(paths: &ConfigPaths) -> Result<StorageSnapshot> {
    let config = load_config(paths)?;
    let (queries, query_warnings) = load_saved_queries(paths)?;
    let (aggregations, aggregation_warnings) = load_saved_aggregations(paths)?;
    let mut warnings = query_warnings;
    warnings.extend(aggregation_warnings);
    Ok(StorageSnapshot {
        config,
        queries,
        aggregations,
        warnings,
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

pub fn log_file_path(paths: &ConfigPaths, config: &Config) -> PathBuf {
    match config.logging.file.as_deref() {
        Some(path) if Path::new(path).is_absolute() => PathBuf::from(path),
        Some(path) => paths.global_root.join(path),
        None => paths.global_root.join("lazycompass.log"),
    }
}

pub fn load_saved_queries(paths: &ConfigPaths) -> Result<(Vec<SavedQuery>, Vec<String>)> {
    let Some(dir) = paths.repo_queries_dir() else {
        return Ok((Vec::new(), Vec::new()));
    };

    load_queries_from_dir(&dir)
}

pub fn load_saved_aggregations(
    paths: &ConfigPaths,
) -> Result<(Vec<SavedAggregation>, Vec<String>)> {
    let Some(dir) = paths.repo_aggregations_dir() else {
        return Ok((Vec::new(), Vec::new()));
    };

    load_aggregations_from_dir(&dir)
}

pub fn saved_query_path(paths: &ConfigPaths, name: &str) -> Result<PathBuf> {
    let name = normalize_saved_name(name)?;
    let dir = paths.repo_queries_dir().ok_or_else(|| {
        anyhow::anyhow!("repository config not found; run inside a repo with .lazycompass")
    })?;
    Ok(dir.join(format!("{name}.toml")))
}

pub fn saved_aggregation_path(paths: &ConfigPaths, name: &str) -> Result<PathBuf> {
    let name = normalize_saved_name(name)?;
    let dir = paths.repo_aggregations_dir().ok_or_else(|| {
        anyhow::anyhow!("repository config not found; run inside a repo with .lazycompass")
    })?;
    Ok(dir.join(format!("{name}.toml")))
}

pub fn write_saved_query(
    paths: &ConfigPaths,
    query: &SavedQuery,
    overwrite: bool,
) -> Result<PathBuf> {
    query.validate().context("invalid saved query")?;
    let path = saved_query_path(paths, &query.name)?;
    if path.exists() && !overwrite {
        anyhow::bail!("saved query '{}' already exists", query.name);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create directory {}", parent.display()))?;
    }
    let contents = toml::to_string_pretty(query).context("unable to serialize saved query")?;
    fs::write(&path, contents)
        .with_context(|| format!("unable to write saved query {}", path.display()))?;
    Ok(path)
}

pub fn write_saved_aggregation(
    paths: &ConfigPaths,
    aggregation: &SavedAggregation,
    overwrite: bool,
) -> Result<PathBuf> {
    aggregation
        .validate()
        .context("invalid saved aggregation")?;
    let path = saved_aggregation_path(paths, &aggregation.name)?;
    if path.exists() && !overwrite {
        anyhow::bail!("saved aggregation '{}' already exists", aggregation.name);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create directory {}", parent.display()))?;
    }
    let contents =
        toml::to_string_pretty(aggregation).context("unable to serialize saved aggregation")?;
    fs::write(&path, contents)
        .with_context(|| format!("unable to write saved aggregation {}", path.display()))?;
    Ok(path)
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
    let mut config: Config = toml::from_str(&contents)
        .with_context(|| format!("invalid TOML in config file {}", path.display()))?;
    resolve_env_vars(&mut config, path)?;
    validate_config(&config)
        .with_context(|| format!("invalid config data in {}", path.display()))?;
    Ok(config)
}

fn resolve_env_vars(config: &mut Config, path: &Path) -> Result<()> {
    for (index, connection) in config.connections.iter_mut().enumerate() {
        if connection.uri.contains("${") {
            let label = if connection.name.trim().is_empty() {
                format!("connection at index {index}")
            } else {
                format!("connection '{}'", connection.name)
            };
            let resolved = interpolate_env_value(&connection.uri).map_err(|error| {
                anyhow::anyhow!(
                    "config {}: unable to resolve env vars in {label} uri: {error}",
                    path.display()
                )
            })?;
            connection.uri = resolved;
        }
    }

    if let Some(file) = config.logging.file.as_deref()
        && file.contains("${")
    {
        let resolved = interpolate_env_value(file).map_err(|error| {
            anyhow::anyhow!(
                "config {}: unable to resolve env vars in logging.file: {error}",
                path.display()
            )
        })?;
        config.logging.file = Some(resolved);
    }

    Ok(())
}

fn interpolate_env_value(value: &str) -> Result<String> {
    let mut output = String::with_capacity(value.len());
    let mut remainder = value;

    while let Some(start) = remainder.find("${") {
        output.push_str(&remainder[..start]);
        let rest = &remainder[start + 2..];
        let end = rest
            .find('}')
            .ok_or_else(|| anyhow::anyhow!("unterminated env var placeholder"))?;
        let name = &rest[..end];
        if name.trim().is_empty() {
            anyhow::bail!("empty env var placeholder");
        }
        let value = std::env::var(name)
            .map_err(|_| anyhow::anyhow!("missing environment variable '{name}'"))?;
        output.push_str(&value);
        remainder = &rest[end + 1..];
    }

    output.push_str(remainder);
    Ok(output)
}

fn validate_config(config: &Config) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for (index, connection) in config.connections.iter().enumerate() {
        if connection.name.trim().is_empty() {
            anyhow::bail!("connection at index {} has empty name", index);
        }
        if connection.uri.trim().is_empty() {
            anyhow::bail!("connection '{}' has empty uri", connection.name);
        }
        if !seen.insert(connection.name.clone()) {
            anyhow::bail!("duplicate connection name '{}'", connection.name);
        }
    }
    if let Some(timeout) = config.timeouts.connect_ms
        && timeout == 0
    {
        anyhow::bail!("connect timeout must be greater than 0");
    }
    if let Some(timeout) = config.timeouts.query_ms
        && timeout == 0
    {
        anyhow::bail!("query timeout must be greater than 0");
    }
    if let Some(max_size_mb) = config.logging.max_size_mb
        && max_size_mb == 0
    {
        anyhow::bail!("logging max_size_mb must be greater than 0");
    }
    if let Some(max_backups) = config.logging.max_backups
        && max_backups == 0
    {
        anyhow::bail!("logging max_backups must be greater than 0");
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

    let theme = if repo.theme.name.is_some() {
        repo.theme
    } else {
        global.theme
    };
    let logging = LoggingConfig {
        level: repo.logging.level.or(global.logging.level),
        file: repo.logging.file.or(global.logging.file),
        max_size_mb: repo.logging.max_size_mb.or(global.logging.max_size_mb),
        max_backups: repo.logging.max_backups.or(global.logging.max_backups),
    };
    let read_only = repo.read_only.or(global.read_only);
    let timeouts = TimeoutConfig {
        connect_ms: repo.timeouts.connect_ms.or(global.timeouts.connect_ms),
        query_ms: repo.timeouts.query_ms.or(global.timeouts.query_ms),
    };

    Config {
        connections,
        theme,
        logging,
        read_only,
        timeouts,
    }
}

fn normalize_saved_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("name cannot be empty");
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        anyhow::bail!("name cannot contain path separators");
    }
    Ok(trimmed.to_string())
}

fn load_queries_from_dir(dir: &Path) -> Result<(Vec<SavedQuery>, Vec<String>)> {
    let paths = collect_toml_paths(dir)?;
    let mut queries = Vec::with_capacity(paths.len());
    let mut warnings = Vec::new();

    for path in paths {
        let result = (|| -> Result<SavedQuery> {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("unable to read saved query file {}", path.display()))?;
            let query: SavedQuery = toml::from_str(&contents)
                .with_context(|| format!("invalid TOML in saved query {}", path.display()))?;
            query
                .validate()
                .with_context(|| format!("invalid saved query {}", path.display()))?;
            Ok(query)
        })();

        match result {
            Ok(query) => queries.push(query),
            Err(error) => {
                warnings.push(format!("skipping saved query {}: {error}", path.display()))
            }
        }
    }

    Ok((queries, warnings))
}

fn load_aggregations_from_dir(dir: &Path) -> Result<(Vec<SavedAggregation>, Vec<String>)> {
    let paths = collect_toml_paths(dir)?;
    let mut aggregations = Vec::with_capacity(paths.len());
    let mut warnings = Vec::new();

    for path in paths {
        let result = (|| -> Result<SavedAggregation> {
            let contents = fs::read_to_string(&path).with_context(|| {
                format!("unable to read saved aggregation file {}", path.display())
            })?;
            let aggregation: SavedAggregation = toml::from_str(&contents)
                .with_context(|| format!("invalid TOML in saved aggregation {}", path.display()))?;
            aggregation
                .validate()
                .with_context(|| format!("invalid saved aggregation {}", path.display()))?;
            Ok(aggregation)
        })();

        match result {
            Ok(aggregation) => aggregations.push(aggregation),
            Err(error) => warnings.push(format!(
                "skipping saved aggregation {}: {error}",
                path.display()
            )),
        }
    }

    Ok((aggregations, warnings))
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

    fn unique_env_suffix() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string()
    }

    #[test]
    fn load_config_merges_repo_overrides() -> Result<()> {
        let root = temp_root("config_merge");
        let global_root = root.join("global");
        let repo_root = root.join("repo");

        write_file(
            &global_root.join("config.toml"),
            r#"read_only = true

[timeouts]
connect_ms = 5000
query_ms = 25000

[[connections]]
name = "shared"
uri = "mongodb://global"
default_database = "global_db"

[[connections]]
name = "global_only"
uri = "mongodb://global_only"

[theme]
name = "classic"

[logging]
level = "info"
file = "global.log"
"#,
        );
        write_file(
            &repo_root.join(".lazycompass/config.toml"),
            r#"read_only = false

[timeouts]
connect_ms = 8000
query_ms = 40000

[[connections]]
name = "shared"
uri = "mongodb://repo"
default_database = "repo_db"

[[connections]]
name = "repo_only"
uri = "mongodb://repo_only"

[theme]
name = "ember"

[logging]
level = "debug"
file = "repo.log"
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
        assert_eq!(config.theme.name.as_deref(), Some("ember"));
        assert_eq!(config.logging.level.as_deref(), Some("debug"));
        assert_eq!(config.logging.file.as_deref(), Some("repo.log"));
        assert_eq!(config.read_only, Some(false));
        assert_eq!(config.timeouts.connect_ms, Some(8000));
        assert_eq!(config.timeouts.query_ms, Some(40000));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_config_interpolates_env_vars() -> Result<()> {
        let root = temp_root("config_env");
        let global_root = root.join("global");
        let suffix = unique_env_suffix();
        let uri_var = format!("LAZYCOMPASS_TEST_URI_{suffix}");
        let log_var = format!("LAZYCOMPASS_TEST_LOG_{suffix}");

        unsafe {
            std::env::set_var(&uri_var, "mongodb://localhost:27017");
            std::env::set_var(&log_var, "logs");
        }

        write_file(
            &global_root.join("config.toml"),
            &format!(
                r#"[[connections]]
name = "local"
uri = "${{{uri_var}}}"

[logging]
file = "${{{log_var}}}/lazycompass.log"
"#
            ),
        );

        let paths = ConfigPaths {
            global_root,
            repo_root: None,
        };
        let config = load_config(&paths)?;

        assert_eq!(config.connections[0].uri, "mongodb://localhost:27017");
        assert_eq!(config.logging.file.as_deref(), Some("logs/lazycompass.log"));

        unsafe {
            std::env::remove_var(&uri_var);
            std::env::remove_var(&log_var);
        }
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_config_rejects_missing_env_var() -> Result<()> {
        let root = temp_root("config_env_missing");
        let global_root = root.join("global");
        let suffix = unique_env_suffix();
        let missing_var = format!("LAZYCOMPASS_TEST_MISSING_{suffix}");

        write_file(
            &global_root.join("config.toml"),
            &format!(
                r#"[[connections]]
name = "local"
uri = "${{{missing_var}}}"
"#
            ),
        );

        let paths = ConfigPaths {
            global_root,
            repo_root: None,
        };
        let err = load_config(&paths).expect_err("expected config load to fail");

        assert!(err.to_string().contains("missing environment variable"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn log_file_path_uses_global_root_for_relative() -> Result<()> {
        let root = temp_root("log_path");
        let global_root = root.join("global");
        let repo_root = root.join("repo");
        let paths = ConfigPaths {
            global_root: global_root.clone(),
            repo_root: Some(repo_root),
        };
        let config = Config {
            connections: Vec::new(),
            theme: lazycompass_core::ThemeConfig::default(),
            logging: LoggingConfig {
                level: None,
                file: Some("logs/lazycompass.log".to_string()),
                max_size_mb: None,
                max_backups: None,
            },
            read_only: None,
            timeouts: TimeoutConfig::default(),
        };

        let resolved = log_file_path(&paths, &config);
        assert_eq!(resolved, global_root.join("logs/lazycompass.log"));

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
        let (queries, query_warnings) = load_saved_queries(&paths)?;
        let (aggregations, aggregation_warnings) = load_saved_aggregations(&paths)?;

        assert!(query_warnings.is_empty());
        assert!(aggregation_warnings.is_empty());
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].name, "active_users");
        assert_eq!(aggregations.len(), 1);
        assert_eq!(aggregations[0].name, "orders_by_user");

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_saved_specs_skips_invalid_files() -> Result<()> {
        let root = temp_root("saved_specs_invalid");
        let repo_root = root.join("repo");

        write_file(
            &repo_root.join(".lazycompass/queries/valid.toml"),
            r#"name = "valid"
database = "lazycompass"
collection = "users"
"#,
        );
        write_file(
            &repo_root.join(".lazycompass/queries/invalid.toml"),
            r#"name = "invalid"
collection = "users"
"#,
        );

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let (queries, warnings) = load_saved_queries(&paths)?;

        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].name, "valid");
        assert_eq!(warnings.len(), 1);

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_config_rejects_duplicate_connections() -> Result<()> {
        let root = temp_root("config_dupes");
        let global_root = root.join("global");

        write_file(
            &global_root.join("config.toml"),
            r#"[[connections]]
name = "shared"
uri = "mongodb://one"

[[connections]]
name = "shared"
uri = "mongodb://two"
"#,
        );

        let paths = ConfigPaths {
            global_root,
            repo_root: None,
        };

        assert!(load_config(&paths).is_err());

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn write_saved_query_persists_toml() -> Result<()> {
        let root = temp_root("write_saved_query");
        let repo_root = root.join("repo");

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let query = SavedQuery {
            name: "recent_orders".to_string(),
            connection: Some("local".to_string()),
            database: "lazycompass".to_string(),
            collection: "orders".to_string(),
            filter: Some("{ \"status\": \"open\" }".to_string()),
            projection: None,
            sort: None,
            limit: Some(50),
            notes: None,
        };

        let path = write_saved_query(&paths, &query, false)?;
        assert!(path.is_file());
        let contents = fs::read_to_string(&path)?;
        let roundtrip: SavedQuery = toml::from_str(&contents)?;
        assert_eq!(roundtrip.name, "recent_orders");

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn write_saved_aggregation_rejects_overwrite_without_flag() -> Result<()> {
        let root = temp_root("write_saved_aggregation");
        let repo_root = root.join("repo");

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let aggregation = SavedAggregation {
            name: "orders_by_user".to_string(),
            connection: Some("local".to_string()),
            database: "lazycompass".to_string(),
            collection: "orders".to_string(),
            pipeline: "[]".to_string(),
            notes: None,
        };

        let _ = write_saved_aggregation(&paths, &aggregation, false)?;
        assert!(write_saved_aggregation(&paths, &aggregation, false).is_err());

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
