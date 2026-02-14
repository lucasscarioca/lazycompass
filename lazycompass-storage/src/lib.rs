use anyhow::{Context, Result};
use lazycompass_core::{
    Config, LoggingConfig, SavedAggregation, SavedQuery, SavedScope, TimeoutConfig,
    connection_security_warnings, redact_sensitive_text,
};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

const APP_DIR: &str = "lazycompass";
const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

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
    load_storage_with_config(paths, config)
}

pub fn load_storage_with_config(paths: &ConfigPaths, config: Config) -> Result<StorageSnapshot> {
    let mut warnings = connection_security_warnings(&config);
    warnings.extend(permission_warnings(paths));
    let (queries, query_warnings) = load_saved_queries(paths)?;
    let (aggregations, aggregation_warnings) = load_saved_aggregations(paths)?;
    warnings.extend(query_warnings);
    warnings.extend(aggregation_warnings);
    Ok(StorageSnapshot {
        config,
        queries,
        aggregations,
        warnings,
    })
}

pub fn load_config(paths: &ConfigPaths) -> Result<Config> {
    let repo = match paths.repo_config_path() {
        Some(path) => read_config(&path)?,
        None => Config::default(),
    };
    let global = read_config(&paths.global_config_path())?;

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

pub fn saved_query_path(paths: &ConfigPaths, id: &str) -> Result<PathBuf> {
    validate_saved_id(id)?;
    let dir = paths.repo_queries_dir().ok_or_else(|| {
        anyhow::anyhow!("repository config not found; run inside a repo with .lazycompass")
    })?;
    Ok(dir.join(format!("{id}.json")))
}

pub fn saved_aggregation_path(paths: &ConfigPaths, id: &str) -> Result<PathBuf> {
    validate_saved_id(id)?;
    let dir = paths.repo_aggregations_dir().ok_or_else(|| {
        anyhow::anyhow!("repository config not found; run inside a repo with .lazycompass")
    })?;
    Ok(dir.join(format!("{id}.json")))
}

pub fn write_saved_query(
    paths: &ConfigPaths,
    query: &SavedQuery,
    overwrite: bool,
) -> Result<PathBuf> {
    query.validate().context("invalid saved query")?;
    let parsed_scope = parse_scope_from_saved_id(&query.id)?;
    if parsed_scope != query.scope {
        anyhow::bail!("saved query id '{}' does not match its scope", query.id);
    }
    let path = saved_query_path(paths, &query.id)?;
    if path.exists() && !overwrite {
        anyhow::bail!("saved query '{}' already exists", query.id);
    }
    if let Some(parent) = path.parent() {
        ensure_secure_dir(parent)?;
    }
    let contents = serde_json::to_string_pretty(&saved_query_payload(query)?)
        .context("unable to serialize saved query")?;
    write_secure_file(&path, &contents)
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
    let parsed_scope = parse_scope_from_saved_id(&aggregation.id)?;
    if parsed_scope != aggregation.scope {
        anyhow::bail!(
            "saved aggregation id '{}' does not match its scope",
            aggregation.id
        );
    }
    let path = saved_aggregation_path(paths, &aggregation.id)?;
    if path.exists() && !overwrite {
        anyhow::bail!("saved aggregation '{}' already exists", aggregation.id);
    }
    if let Some(parent) = path.parent() {
        ensure_secure_dir(parent)?;
    }
    let pipeline_json: Value = serde_json::from_str(&aggregation.pipeline)
        .context("saved aggregation pipeline must be valid JSON")?;
    if !pipeline_json.is_array() {
        anyhow::bail!("saved aggregation pipeline must be a JSON array");
    }
    let contents = serde_json::to_string_pretty(&pipeline_json)
        .context("unable to serialize saved aggregation")?;
    write_secure_file(&path, &contents)
        .with_context(|| format!("unable to write saved aggregation {}", path.display()))?;
    Ok(path)
}

/// Append a connection to the repo config file.
/// Creates the config file if it doesn't exist.
pub async fn append_connection_to_repo_config(
    paths: &ConfigPaths,
    connection: &lazycompass_core::ConnectionSpec,
) -> Result<PathBuf> {
    let repo_root = paths
        .repo_config_root()
        .ok_or_else(|| anyhow::anyhow!("no repo config found"))?;
    let config_path = repo_root.join("config.toml");

    // Ensure the directory exists
    ensure_secure_dir(&repo_root)?;

    // Read existing config or create default
    let mut config = if config_path.exists() {
        read_config_for_update(&config_path)?
    } else {
        lazycompass_core::Config::default()
    };

    // Check for duplicate names
    if config.connections.iter().any(|c| c.name == connection.name) {
        anyhow::bail!(
            "connection '{}' already exists in repo config",
            connection.name
        );
    }

    // Add the new connection
    config.connections.push(connection.clone());

    // Write back
    let contents = toml::to_string_pretty(&config).context("unable to serialize config")?;
    write_secure_file(&config_path, &contents)
        .with_context(|| format!("unable to write config {}", config_path.display()))?;

    Ok(config_path)
}

/// Append a connection to the global config file.
/// Creates the config file if it doesn't exist.
pub async fn append_connection_to_global_config(
    paths: &ConfigPaths,
    connection: &lazycompass_core::ConnectionSpec,
) -> Result<PathBuf> {
    let config_path = paths.global_config_path();

    // Ensure the directory exists
    ensure_secure_dir(&paths.global_root)?;

    // Read existing config or create default
    let mut config = if config_path.exists() {
        read_config_for_update(&config_path)?
    } else {
        lazycompass_core::Config::default()
    };

    // Check for duplicate names
    if config.connections.iter().any(|c| c.name == connection.name) {
        anyhow::bail!(
            "connection '{}' already exists in global config",
            connection.name
        );
    }

    // Add the new connection
    config.connections.push(connection.clone());

    // Write back
    let contents = toml::to_string_pretty(&config).context("unable to serialize config")?;
    write_secure_file(&config_path, &contents)
        .with_context(|| format!("unable to write config {}", config_path.display()))?;

    Ok(config_path)
}

/// Read config for update (without env var resolution - we keep them as-is)
fn read_config_for_update(path: &Path) -> Result<lazycompass_core::Config> {
    if !path.is_file() {
        return Ok(lazycompass_core::Config::default());
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("unable to read config file {}", path.display()))?;
    let config: lazycompass_core::Config = toml::from_str(&contents)
        .with_context(|| format!("invalid TOML in config file {}", path.display()))?;
    Ok(config)
}

fn ensure_secure_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .with_context(|| format!("unable to create directory {}", path.display()))?;
    set_dir_permissions(path)?;
    if let Some(parent) = path.parent()
        && is_config_root_dir(parent)
    {
        set_dir_permissions(parent)?;
    }
    Ok(())
}

fn is_config_root_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".lazycompass") | Some(APP_DIR)
    )
}

fn write_secure_file(path: &Path, contents: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(FILE_MODE)
            .open(path)
            .with_context(|| format!("unable to open file {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("unable to write file {}", path.display()))?;
        set_file_permissions(path)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        fs::write(path, contents)
            .with_context(|| format!("unable to write file {}", path.display()))?;
        Ok(())
    }
}

#[cfg(unix)]
fn set_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(DIR_MODE))
        .with_context(|| format!("unable to set permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_file_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(FILE_MODE))
        .with_context(|| format!("unable to set permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn permission_warnings(paths: &ConfigPaths) -> Vec<String> {
    let mut warnings = Vec::new();
    append_permission_warning_dir(&paths.global_root, &mut warnings);
    append_permission_warning_file(&paths.global_config_path(), &mut warnings);
    if let Some(repo_root) = paths.repo_config_root() {
        append_permission_warning_dir(&repo_root, &mut warnings);
        let config_path = repo_root.join("config.toml");
        append_permission_warning_file(&config_path, &mut warnings);
        let queries_dir = repo_root.join("queries");
        append_permission_warning_dir(&queries_dir, &mut warnings);
        append_permission_warning_json_files(&queries_dir, &mut warnings);
        let aggregations_dir = repo_root.join("aggregations");
        append_permission_warning_dir(&aggregations_dir, &mut warnings);
        append_permission_warning_json_files(&aggregations_dir, &mut warnings);
    }
    warnings
}

#[cfg(not(unix))]
fn permission_warnings(_paths: &ConfigPaths) -> Vec<String> {
    Vec::new()
}

#[cfg(unix)]
fn append_permission_warning_dir(path: &Path, warnings: &mut Vec<String>) {
    if !path.is_dir() {
        return;
    }
    if let Ok(metadata) = fs::metadata(path) {
        let mode = metadata.mode() & 0o777;
        if (mode & 0o077) != 0 {
            warnings.push(redact_sensitive_text(&format!(
                "permission warning: directory {} has mode {:03o}, expected {:03o}",
                path.display(),
                mode,
                DIR_MODE
            )));
        }
    }
}

#[cfg(unix)]
fn append_permission_warning_file(path: &Path, warnings: &mut Vec<String>) {
    if !path.is_file() {
        return;
    }
    if let Ok(metadata) = fs::metadata(path) {
        let mode = metadata.mode() & 0o777;
        if (mode & 0o077) != 0 {
            warnings.push(redact_sensitive_text(&format!(
                "permission warning: file {} has mode {:03o}, expected {:03o}",
                path.display(),
                mode,
                FILE_MODE
            )));
        }
    }
}

#[cfg(unix)]
fn append_permission_warning_json_files(dir: &Path, warnings: &mut Vec<String>) {
    if let Ok(paths) = collect_json_paths(dir) {
        for path in paths {
            append_permission_warning_file(&path, warnings);
        }
    }
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

    load_dotenv_for_config(path)?;
    let contents = fs::read_to_string(path)
        .with_context(|| format!("unable to read config file {}", path.display()))?;
    let mut config: Config = toml::from_str(&contents)
        .with_context(|| format!("invalid TOML in config file {}", path.display()))?;
    resolve_env_vars(&mut config, path)?;
    validate_config(&config)
        .with_context(|| format!("invalid config data in {}", path.display()))?;
    Ok(config)
}

fn load_dotenv_for_config(path: &Path) -> Result<()> {
    let Some(dotenv_path) = dotenv_path_for_config(path) else {
        return Ok(());
    };

    if dotenv_path.is_file() {
        dotenvy::from_path(&dotenv_path)
            .with_context(|| format!("unable to read .env file {}", dotenv_path.display()))?;
    }

    Ok(())
}

fn dotenv_path_for_config(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    if parent.file_name().and_then(|name| name.to_str()) == Some(".lazycompass") {
        return parent.parent().map(|root| root.join(".env"));
    }
    Some(parent.join(".env"))
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
    let allow_pipeline_writes = repo.allow_pipeline_writes.or(global.allow_pipeline_writes);
    let allow_insecure = repo.allow_insecure.or(global.allow_insecure);
    let timeouts = TimeoutConfig {
        connect_ms: repo.timeouts.connect_ms.or(global.timeouts.connect_ms),
        query_ms: repo.timeouts.query_ms.or(global.timeouts.query_ms),
    };

    Config {
        connections,
        theme,
        logging,
        read_only,
        allow_pipeline_writes,
        allow_insecure,
        timeouts,
    }
}

fn load_queries_from_dir(dir: &Path) -> Result<(Vec<SavedQuery>, Vec<String>)> {
    let paths = collect_json_paths(dir)?;
    let mut queries = Vec::with_capacity(paths.len());
    let mut warnings = Vec::new();

    for path in paths {
        let result = (|| -> Result<SavedQuery> {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("unable to read saved query file {}", path.display()))?;
            let json: Value = serde_json::from_str(&contents)
                .with_context(|| format!("invalid JSON in saved query {}", path.display()))?;
            let id = saved_id_from_path(&path)?;
            let scope = parse_scope_from_saved_id(&id)
                .with_context(|| format!("invalid saved query id '{id}'"))?;
            let query = parse_saved_query_payload(&json, id, scope)
                .with_context(|| format!("invalid saved query {}", path.display()))?;
            Ok(query)
        })();

        match result {
            Ok(query) => queries.push(query),
            Err(error) => {
                let warning = format!("skipping saved query {}: {error}", path.display());
                warnings.push(redact_sensitive_text(&warning));
            }
        }
    }

    Ok((queries, warnings))
}

fn load_aggregations_from_dir(dir: &Path) -> Result<(Vec<SavedAggregation>, Vec<String>)> {
    let paths = collect_json_paths(dir)?;
    let mut aggregations = Vec::with_capacity(paths.len());
    let mut warnings = Vec::new();

    for path in paths {
        let result = (|| -> Result<SavedAggregation> {
            let contents = fs::read_to_string(&path).with_context(|| {
                format!("unable to read saved aggregation file {}", path.display())
            })?;
            let json: Value = serde_json::from_str(&contents)
                .with_context(|| format!("invalid JSON in saved aggregation {}", path.display()))?;
            let id = saved_id_from_path(&path)?;
            let scope = parse_scope_from_saved_id(&id)
                .with_context(|| format!("invalid saved aggregation id '{id}'"))?;
            let aggregation = parse_saved_aggregation_payload(&json, id, scope)
                .with_context(|| format!("invalid saved aggregation {}", path.display()))?;
            Ok(aggregation)
        })();

        match result {
            Ok(aggregation) => aggregations.push(aggregation),
            Err(error) => {
                let warning = format!("skipping saved aggregation {}: {error}", path.display());
                warnings.push(redact_sensitive_text(&warning));
            }
        }
    }

    Ok((aggregations, warnings))
}

fn collect_json_paths(dir: &Path) -> Result<Vec<PathBuf>> {
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
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}

fn saved_id_from_path(path: &Path) -> Result<String> {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow::anyhow!("saved spec filename is not valid UTF-8"))?;
    validate_saved_id(stem)?;
    Ok(stem.to_string())
}

fn validate_saved_id(id: &str) -> Result<()> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        anyhow::bail!("saved id cannot be empty");
    }
    if trimmed != id {
        anyhow::bail!("saved id cannot have leading or trailing whitespace");
    }
    if id.contains('/') || id.contains('\\') {
        anyhow::bail!("saved id cannot contain path separators");
    }

    let segments: Vec<&str> = id.split('.').collect();
    if segments.iter().any(|segment| segment.trim().is_empty()) {
        anyhow::bail!("saved id cannot contain empty segments");
    }
    if segments.len() == 2 {
        anyhow::bail!(
            "saved id with two segments is invalid; use <name> or <db>.<collection>.<name>"
        );
    }
    Ok(())
}

fn parse_scope_from_saved_id(id: &str) -> Result<SavedScope> {
    validate_saved_id(id)?;
    let segments: Vec<&str> = id.split('.').collect();
    if segments.len() == 1 {
        return Ok(SavedScope::Shared);
    }
    let database = segments
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing database segment"))?
        .to_string();
    let name = segments
        .last()
        .ok_or_else(|| anyhow::anyhow!("missing name segment"))?;
    if name.trim().is_empty() {
        anyhow::bail!("missing name segment");
    }
    let collection = segments[1..segments.len() - 1].join(".");
    if collection.trim().is_empty() {
        anyhow::bail!("missing collection segment");
    }
    Ok(SavedScope::Scoped {
        database,
        collection,
    })
}

fn parse_saved_query_payload(json: &Value, id: String, scope: SavedScope) -> Result<SavedQuery> {
    let object = json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("saved query payload must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "filter" | "projection" | "sort" | "limit") {
            anyhow::bail!("unknown field '{key}' in saved query payload");
        }
    }

    let filter = json_field_as_json_string(object, "filter")?;
    let projection = json_field_as_json_string(object, "projection")?;
    let sort = json_field_as_json_string(object, "sort")?;
    let limit = match object.get("limit") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("field 'limit' must be a non-negative integer"))?,
        ),
    };

    let query = SavedQuery {
        id,
        scope,
        filter,
        projection,
        sort,
        limit,
    };
    query.validate().context("invalid saved query data")?;
    Ok(query)
}

fn saved_query_payload(query: &SavedQuery) -> Result<Value> {
    let mut object = serde_json::Map::new();
    if let Some(filter) = query.filter.as_deref() {
        object.insert(
            "filter".to_string(),
            serde_json::from_str(filter).context("saved query filter must be valid JSON")?,
        );
    }
    if let Some(projection) = query.projection.as_deref() {
        object.insert(
            "projection".to_string(),
            serde_json::from_str(projection)
                .context("saved query projection must be valid JSON")?,
        );
    }
    if let Some(sort) = query.sort.as_deref() {
        object.insert(
            "sort".to_string(),
            serde_json::from_str(sort).context("saved query sort must be valid JSON")?,
        );
    }
    if let Some(limit) = query.limit {
        object.insert("limit".to_string(), Value::from(limit));
    }
    Ok(Value::Object(object))
}

fn parse_saved_aggregation_payload(
    json: &Value,
    id: String,
    scope: SavedScope,
) -> Result<SavedAggregation> {
    if !json.is_array() {
        anyhow::bail!("saved aggregation payload must be a JSON array");
    }
    let aggregation = SavedAggregation {
        id,
        scope,
        pipeline: serde_json::to_string(json).context("unable to serialize pipeline JSON")?,
    };
    aggregation
        .validate()
        .context("invalid saved aggregation data")?;
    Ok(aggregation)
}

fn json_field_as_json_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let serialized = serde_json::to_string(value)
        .with_context(|| format!("unable to serialize field '{field}'"))?;
    Ok(Some(serialized))
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
            allow_pipeline_writes: None,
            allow_insecure: None,
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
            &repo_root.join(".lazycompass/queries/lazycompass.users.active_users.json"),
            r#"{
  "filter": { "active": true },
  "projection": { "email": 1 }
}
"#,
        );
        write_file(
            &repo_root.join(".lazycompass/aggregations/orders_by_user.json"),
            r#"[
  { "$group": { "_id": "$userId" } }
]
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
        assert_eq!(queries[0].id, "lazycompass.users.active_users");
        assert!(matches!(
            queries[0].scope,
            SavedScope::Scoped {
                ref database,
                ref collection
            } if database == "lazycompass" && collection == "users"
        ));
        assert_eq!(aggregations.len(), 1);
        assert_eq!(aggregations[0].id, "orders_by_user");
        assert!(matches!(aggregations[0].scope, SavedScope::Shared));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_saved_specs_skips_invalid_files() -> Result<()> {
        let root = temp_root("saved_specs_invalid");
        let repo_root = root.join("repo");

        write_file(
            &repo_root.join(".lazycompass/queries/valid.json"),
            r#"{
  "filter": { "active": true }
}
"#,
        );
        write_file(
            &repo_root.join(".lazycompass/queries/db.name.json"),
            r#"{
  "filter": { "active": true }
}
"#,
        );

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let (queries, warnings) = load_saved_queries(&paths)?;

        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].id, "valid");
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
    fn write_saved_query_persists_json() -> Result<()> {
        let root = temp_root("write_saved_query");
        let repo_root = root.join("repo");

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let query = SavedQuery {
            id: "lazycompass.orders.recent_orders".to_string(),
            scope: SavedScope::Scoped {
                database: "lazycompass".to_string(),
                collection: "orders".to_string(),
            },
            filter: Some("{ \"status\": \"open\" }".to_string()),
            projection: None,
            sort: None,
            limit: Some(50),
        };

        let path = write_saved_query(&paths, &query, false)?;
        assert!(path.is_file());
        let contents = fs::read_to_string(&path)?;
        let json: serde_json::Value = serde_json::from_str(&contents)?;
        assert_eq!(json.get("limit").and_then(|value| value.as_u64()), Some(50));

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
            id: "orders_by_user".to_string(),
            scope: SavedScope::Shared,
            pipeline: "[]".to_string(),
        };

        let _ = write_saved_aggregation(&paths, &aggregation, false)?;
        assert!(write_saved_aggregation(&paths, &aggregation, false).is_err());

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn parse_scope_from_saved_id_supports_multidot_collection() -> Result<()> {
        let scope = parse_scope_from_saved_id("app.foo.bar.users.active")?;
        assert!(matches!(
            scope,
            SavedScope::Scoped {
                ref database,
                ref collection
            } if database == "app" && collection == "foo.bar.users"
        ));
        Ok(())
    }

    #[test]
    fn parse_scope_from_saved_id_rejects_two_segments() {
        let err = parse_scope_from_saved_id("app.users").expect_err("should fail");
        assert!(err.to_string().contains("two segments"));
    }

    #[cfg(unix)]
    #[test]
    fn permission_warnings_detect_permissive_modes() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_root("perm_warn");
        let global_root = root.join("global");
        fs::create_dir_all(&global_root)?;
        fs::set_permissions(&global_root, fs::Permissions::from_mode(0o755))?;
        let global_config = global_root.join("config.toml");
        write_file(&global_config, "read_only = true\n");
        fs::set_permissions(&global_config, fs::Permissions::from_mode(0o644))?;

        let repo_root = root.join("repo");
        let repo_config_root = repo_root.join(".lazycompass");
        fs::create_dir_all(repo_config_root.join("queries"))?;
        fs::create_dir_all(repo_config_root.join("aggregations"))?;
        fs::set_permissions(&repo_config_root, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(
            &repo_config_root.join("queries"),
            fs::Permissions::from_mode(0o755),
        )?;
        let query_path = repo_config_root.join("queries/query.json");
        write_file(&query_path, "{}\n");
        fs::set_permissions(&query_path, fs::Permissions::from_mode(0o644))?;

        let paths = ConfigPaths {
            global_root,
            repo_root: Some(repo_root),
        };
        let warnings = permission_warnings(&paths);

        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("config.toml"))
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains(".lazycompass"))
        );
        assert!(warnings.iter().any(|warning| warning.contains("queries")));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
