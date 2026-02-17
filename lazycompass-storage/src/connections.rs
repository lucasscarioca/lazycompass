use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    ConfigPaths,
    security::{ensure_secure_dir, write_secure_file},
};

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

    ensure_secure_dir(&repo_root)?;

    let mut config = if config_path.exists() {
        read_config_for_update(&config_path)?
    } else {
        lazycompass_core::Config::default()
    };

    if config.connections.iter().any(|c| c.name == connection.name) {
        anyhow::bail!(
            "connection '{}' already exists in repo config",
            connection.name
        );
    }

    config.connections.push(connection.clone());

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

    ensure_secure_dir(&paths.global_root)?;

    let mut config = if config_path.exists() {
        read_config_for_update(&config_path)?
    } else {
        lazycompass_core::Config::default()
    };

    if config.connections.iter().any(|c| c.name == connection.name) {
        anyhow::bail!(
            "connection '{}' already exists in global config",
            connection.name
        );
    }

    config.connections.push(connection.clone());

    let contents = toml::to_string_pretty(&config).context("unable to serialize config")?;
    write_secure_file(&config_path, &contents)
        .with_context(|| format!("unable to write config {}", config_path.display()))?;

    Ok(config_path)
}

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
