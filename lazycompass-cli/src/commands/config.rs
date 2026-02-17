use anyhow::{Context, Result};
use lazycompass_storage::{
    ConfigPaths, append_connection_to_global_config, append_connection_to_repo_config,
};
use std::env;
use std::fs;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::{ConfigArgs, ConfigCommands};
use crate::editor::open_in_editor;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ConfigScope {
    Global,
    Repo,
}

pub(crate) fn run_config(args: ConfigArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let scope = resolve_config_scope(&paths, args.global, args.repo);

    match args.command {
        ConfigCommands::Edit => run_config_edit(&paths, scope),
        ConfigCommands::AddConnection => run_config_add_connection(&paths, scope),
    }
}

pub(crate) fn resolve_config_scope(paths: &ConfigPaths, global: bool, repo: bool) -> ConfigScope {
    if global {
        return ConfigScope::Global;
    }
    if repo || paths.repo_config_path().is_some() {
        return ConfigScope::Repo;
    }
    ConfigScope::Global
}

fn run_config_edit(paths: &ConfigPaths, scope: ConfigScope) -> Result<()> {
    let config_path = match scope {
        ConfigScope::Global => paths.global_config_path(),
        ConfigScope::Repo => paths.repo_config_path().ok_or_else(|| {
            anyhow::anyhow!(
                "no repository config found; run inside a repo with .lazycompass or use --global"
            )
        })?,
    };

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("unable to create config directory {}", parent.display()))?;
    }

    if !config_path.exists() {
        let default_config = r#"# LazyCompass Configuration
# Global config: ~/.config/lazycompass/config.toml
# Repo config: .lazycompass/config.toml (overrides global)

# Example connection (remove if not needed):
# [[connections]]
# name = "local"
# uri = "mongodb://localhost:27017"
# default_database = "mydb"

[theme]
# name = "classic"

[logging]
# level = "info"
# file = "lazycompass.log"
# max_size_mb = 10
# max_backups = 3

[timeouts]
# connect_ms = 10000
# query_ms = 30000

# read_only = true
# allow_pipeline_writes = false
# allow_insecure = false
"#;
        fs::write(&config_path, default_config)
            .with_context(|| format!("unable to create config file {}", config_path.display()))?;
    }

    open_in_editor(&config_path)?;

    println!("config opened in editor: {}", config_path.display());
    Ok(())
}

pub(crate) fn run_config_add_connection(paths: &ConfigPaths, scope: ConfigScope) -> Result<()> {
    if let ConfigScope::Repo = scope {
        paths.repo_config_path().ok_or_else(|| {
            anyhow::anyhow!(
                "no repository config found; run inside a repo with .lazycompass or use --global"
            )
        })?;
    }

    let template = r#"# Edit the connection details below and save the file.
# Lines starting with # are comments and will be ignored.
# Remove the comments and fill in your values.

name = "my-connection"
uri = "mongodb://localhost:27017"
default_database = "mydb"
"#;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp_path = env::temp_dir().join(format!("lazycompass_connection_{nonce}.toml"));

    {
        let mut file = fs::File::create(&temp_path)
            .with_context(|| format!("unable to create temp file {}", temp_path.display()))?;
        file.write_all(template.as_bytes())
            .with_context(|| format!("unable to write temp file {}", temp_path.display()))?;
    }

    open_in_editor(&temp_path)?;

    let edited_content = fs::read_to_string(&temp_path)
        .with_context(|| format!("unable to read edited file {}", temp_path.display()))?;

    let _ = fs::remove_file(&temp_path);

    let toml_content: String = edited_content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    let connection: lazycompass_core::ConnectionSpec = toml::from_str(&toml_content).with_context(
        || "invalid TOML in connection definition; expected fields: name, uri, default_database",
    )?;

    if connection.name.trim().is_empty() {
        anyhow::bail!("connection name cannot be empty");
    }
    if connection.uri.trim().is_empty() {
        anyhow::bail!("connection uri cannot be empty");
    }

    let runtime = tokio::runtime::Runtime::new().context("unable to start async runtime")?;

    let config_path = match scope {
        ConfigScope::Global => {
            runtime.block_on(append_connection_to_global_config(paths, &connection))?
        }
        ConfigScope::Repo => {
            runtime.block_on(append_connection_to_repo_config(paths, &connection))?
        }
    };

    println!(
        "connection '{}' added to {}",
        connection.name,
        config_path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use lazycompass_storage::ConfigPaths;

    use super::{ConfigScope, resolve_config_scope};

    #[test]
    fn resolve_config_scope_defaults_to_repo_when_available() {
        let paths = ConfigPaths {
            global_root: "/tmp/global".into(),
            repo_root: Some("/tmp/repo".into()),
        };
        assert!(matches!(
            resolve_config_scope(&paths, false, false),
            ConfigScope::Repo
        ));
    }

    #[test]
    fn resolve_config_scope_defaults_to_global_without_repo() {
        let paths = ConfigPaths {
            global_root: "/tmp/global".into(),
            repo_root: None,
        };
        assert!(matches!(
            resolve_config_scope(&paths, false, false),
            ConfigScope::Global
        ));
    }
}
