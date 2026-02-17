use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub(crate) const APP_DIR: &str = "lazycompass";

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
