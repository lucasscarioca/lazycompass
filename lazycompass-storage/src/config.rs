use anyhow::{Context, Result};
use lazycompass_core::{Config, LoggingConfig, TimeoutConfig};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ConfigPaths;

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
    let mut seen = HashSet::new();
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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use lazycompass_core::{Config, LoggingConfig, ThemeConfig, TimeoutConfig};
    use std::collections::HashMap;
    use std::fs;

    use super::{load_config, log_file_path};
    use crate::{
        ConfigPaths,
        test_support::{temp_root, unique_env_suffix, write_file},
    };

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
            theme: ThemeConfig::default(),
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
    fn load_config_rejects_empty_connection_name() -> Result<()> {
        let root = temp_root("config_empty_name");
        let global_root = root.join("global");

        write_file(
            &global_root.join("config.toml"),
            r#"[[connections]]
name = " "
uri = "mongodb://localhost:27017"
"#,
        );

        let paths = ConfigPaths {
            global_root,
            repo_root: None,
        };
        let err = load_config(&paths).expect_err("expected empty name");
        assert!(err.to_string().contains("invalid"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_config_rejects_empty_connection_uri() -> Result<()> {
        let root = temp_root("config_empty_uri");
        let global_root = root.join("global");

        write_file(
            &global_root.join("config.toml"),
            r#"[[connections]]
name = "local"
uri = " "
"#,
        );

        let paths = ConfigPaths {
            global_root,
            repo_root: None,
        };
        let err = load_config(&paths).expect_err("expected empty uri");
        assert!(err.to_string().contains("invalid"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_config_rejects_zero_numeric_settings() -> Result<()> {
        let root = temp_root("config_zero_numbers");
        let global_root = root.join("global");

        write_file(
            &global_root.join("config.toml"),
            r#"[timeouts]
connect_ms = 0
query_ms = 1

[logging]
max_size_mb = 1
max_backups = 1
"#,
        );
        let err = load_config(&ConfigPaths {
            global_root: global_root.clone(),
            repo_root: None,
        })
        .expect_err("expected zero connect timeout");
        assert!(err.to_string().contains("invalid"));

        write_file(
            &global_root.join("config.toml"),
            r#"[timeouts]
connect_ms = 1
query_ms = 0

[logging]
max_size_mb = 1
max_backups = 1
"#,
        );
        let err = load_config(&ConfigPaths {
            global_root: global_root.clone(),
            repo_root: None,
        })
        .expect_err("expected zero query timeout");
        assert!(err.to_string().contains("invalid"));

        write_file(
            &global_root.join("config.toml"),
            r#"[timeouts]
connect_ms = 1
query_ms = 1

[logging]
max_size_mb = 0
max_backups = 1
"#,
        );
        let err = load_config(&ConfigPaths {
            global_root: global_root.clone(),
            repo_root: None,
        })
        .expect_err("expected zero max size");
        assert!(err.to_string().contains("invalid"));

        write_file(
            &global_root.join("config.toml"),
            r#"[timeouts]
connect_ms = 1
query_ms = 1

[logging]
max_size_mb = 1
max_backups = 0
"#,
        );
        let err = load_config(&ConfigPaths {
            global_root,
            repo_root: None,
        })
        .expect_err("expected zero backups");
        assert!(err.to_string().contains("invalid"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_config_reads_repo_dotenv_from_repo_root() -> Result<()> {
        let root = temp_root("config_repo_dotenv");
        let repo_root = root.join("repo");
        let suffix = unique_env_suffix();
        let var_name = format!("LAZYCOMPASS_REPO_DOTENV_{suffix}");

        write_file(
            &repo_root.join(".env"),
            &format!("{var_name}=mongodb://repo-from-dotenv\n"),
        );
        write_file(
            &repo_root.join(".lazycompass/config.toml"),
            &format!(
                r#"[[connections]]
name = "repo"
uri = "${{{var_name}}}"
"#
            ),
        );

        let config = load_config(&ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        })?;
        assert_eq!(config.connections[0].uri, "mongodb://repo-from-dotenv");

        unsafe {
            std::env::remove_var(&var_name);
        }
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_config_reads_global_dotenv_from_global_dir() -> Result<()> {
        let root = temp_root("config_global_dotenv");
        let global_root = root.join("global");
        let suffix = unique_env_suffix();
        let var_name = format!("LAZYCOMPASS_GLOBAL_DOTENV_{suffix}");

        write_file(
            &global_root.join(".env"),
            &format!("{var_name}=mongodb://global-from-dotenv\n"),
        );
        write_file(
            &global_root.join("config.toml"),
            &format!(
                r#"[[connections]]
name = "global"
uri = "${{{var_name}}}"
"#
            ),
        );

        let config = load_config(&ConfigPaths {
            global_root,
            repo_root: None,
        })?;
        assert_eq!(config.connections[0].uri, "mongodb://global-from-dotenv");

        unsafe {
            std::env::remove_var(&var_name);
        }
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
