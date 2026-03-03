use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    ConfigPaths,
    security::{ensure_not_symlinked_file, ensure_secure_dir, write_secure_file},
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

    append_connection_to_config_file(
        &config_path,
        connection,
        "connection '{}' already exists in repo config",
    )?;

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

    append_connection_to_config_file(
        &config_path,
        connection,
        "connection '{}' already exists in global config",
    )?;

    Ok(config_path)
}

fn append_connection_to_config_file(
    config_path: &Path,
    connection: &lazycompass_core::ConnectionSpec,
    duplicate_message: &str,
) -> Result<()> {
    let existing = read_config_for_update(config_path)?;
    if existing
        .parsed
        .connections
        .iter()
        .any(|candidate| candidate.name == connection.name)
    {
        let message = duplicate_message.replace("{}", &connection.name);
        anyhow::bail!(message);
    }

    let mut contents = existing.raw;
    if !contents.trim().is_empty() {
        if !contents.ends_with('\n') {
            contents.push('\n');
        }
        contents.push('\n');
    }
    contents.push_str(&render_connection_block(connection)?);
    if !contents.ends_with('\n') {
        contents.push('\n');
    }

    write_secure_file(config_path, &contents, existing.exists)
        .with_context(|| format!("unable to write config {}", config_path.display()))
}

struct ConfigUpdate {
    exists: bool,
    parsed: lazycompass_core::Config,
    raw: String,
}

fn read_config_for_update(path: &Path) -> Result<ConfigUpdate> {
    ensure_not_symlinked_file(path)?;
    if !path.is_file() {
        return Ok(ConfigUpdate {
            exists: false,
            parsed: lazycompass_core::Config::default(),
            raw: String::new(),
        });
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("unable to read config file {}", path.display()))?;
    let config: lazycompass_core::Config = toml::from_str(&contents)
        .with_context(|| format!("invalid TOML in config file {}", path.display()))?;
    Ok(ConfigUpdate {
        exists: true,
        parsed: config,
        raw: contents,
    })
}

fn render_connection_block(connection: &lazycompass_core::ConnectionSpec) -> Result<String> {
    let mut block = String::from("[[connections]]\n");
    block.push_str(&toml::to_string_pretty(connection).context("unable to serialize connection")?);
    Ok(block)
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
        assert_eq!(config.parsed.connections.len(), 1);
        assert_eq!(config.parsed.connections[0].name, "local");

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

    #[test]
    fn append_connection_preserves_existing_comments_and_unknown_keys() {
        let root = temp_dir("preserve");
        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: None,
        };
        fs::create_dir_all(&paths.global_root).expect("create global root");
        let config_path = paths.global_root.join("config.toml");
        fs::write(
            &config_path,
            "# keep me\ncustom = \"value\"\n\n[theme]\nname = \"ember\"\n",
        )
        .expect("write config");

        block_on_ready(append_connection_to_global_config(
            &paths,
            &sample_connection("local"),
        ))
        .expect("append global");

        let contents = fs::read_to_string(&config_path).expect("read config");
        assert!(contents.contains("# keep me"));
        assert!(contents.contains("custom = \"value\""));
        assert!(contents.contains("[theme]"));
        assert!(contents.contains("[[connections]]"));

        let _ = fs::remove_dir_all(root);
    }
}
