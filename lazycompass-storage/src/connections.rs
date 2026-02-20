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
    let queries_dir = repo_root.join("queries");
    let aggregations_dir = repo_root.join("aggregations");

    ensure_secure_dir(&repo_root)?;
    ensure_secure_dir(&queries_dir)?;
    ensure_secure_dir(&aggregations_dir)?;

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

#[cfg(test)]
mod tests {
    use super::{
        append_connection_to_global_config, append_connection_to_repo_config,
        read_config_for_update,
    };
    use crate::ConfigPaths;
    use lazycompass_core::ConnectionSpec;
    use std::fs;
    use std::future::Future;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lazycompass_connections_{prefix}_{nonce}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    fn sample_connection(name: &str) -> ConnectionSpec {
        ConnectionSpec {
            name: name.to_string(),
            uri: "mongodb://localhost:27017".to_string(),
            default_database: Some("lazycompass".to_string()),
        }
    }

    fn block_on_ready<F: Future>(future: F) -> F::Output {
        fn raw_waker() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                raw_waker()
            }
            fn wake(_: *const ()) {}
            fn wake_by_ref(_: *const ()) {}
            fn drop(_: *const ()) {}
            RawWaker::new(
                std::ptr::null(),
                &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
            )
        }

        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        match Future::poll(Pin::as_mut(&mut future), &mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("future unexpectedly pending in synchronous test"),
        }
    }

    #[test]
    fn append_connection_to_global_config_creates_and_writes_config() {
        let root = temp_dir("global_append");
        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: None,
        };

        let path = block_on_ready(append_connection_to_global_config(
            &paths,
            &sample_connection("local"),
        ))
        .expect("append global");
        let config = read_config_for_update(&path).expect("read config");
        assert_eq!(config.connections.len(), 1);
        assert_eq!(config.connections[0].name, "local");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn append_connection_to_global_config_rejects_duplicates() {
        let root = temp_dir("global_duplicate");
        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: None,
        };

        block_on_ready(append_connection_to_global_config(
            &paths,
            &sample_connection("local"),
        ))
        .expect("first append");
        let err = block_on_ready(append_connection_to_global_config(
            &paths,
            &sample_connection("local"),
        ))
        .expect_err("expected duplicate error");
        assert!(err.to_string().contains("already exists in global config"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn append_connection_to_repo_config_requires_repo_root() {
        let root = temp_dir("repo_missing");
        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: None,
        };

        let err = block_on_ready(append_connection_to_repo_config(
            &paths,
            &sample_connection("repo"),
        ))
        .expect_err("expected missing repo config");
        assert!(err.to_string().contains("no repo config found"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn append_connection_to_repo_config_creates_saved_spec_dirs() {
        let root = temp_dir("repo_dirs");
        let repo_root = root.join("repo");
        fs::create_dir_all(&repo_root).expect("create repo root");
        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root.clone()),
        };

        block_on_ready(append_connection_to_repo_config(
            &paths,
            &sample_connection("repo"),
        ))
        .expect("append repo");

        assert!(repo_root.join(".lazycompass/queries").is_dir());
        assert!(repo_root.join(".lazycompass/aggregations").is_dir());

        let _ = fs::remove_dir_all(root);
    }
}
