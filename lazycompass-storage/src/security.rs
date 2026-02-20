use anyhow::{Context, Result};
use lazycompass_core::redact_sensitive_text;
use std::fs;
use std::io::Write;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use crate::{ConfigPaths, paths::APP_DIR, saved_common::collect_json_paths};

const DIR_MODE: u32 = 0o700;
const FILE_MODE: u32 = 0o600;

#[cfg(unix)]
pub(crate) fn normalize_permissions(paths: &ConfigPaths) {
    let _ = normalize_dir_if_exists(&paths.global_root);
    let _ = normalize_file_if_exists(&paths.global_config_path());

    if let Some(repo_root) = paths.repo_config_root() {
        let _ = normalize_dir_if_exists(&repo_root);

        let config_path = repo_root.join("config.toml");
        let _ = normalize_file_if_exists(&config_path);

        let queries_dir = repo_root.join("queries");
        let _ = normalize_dir_if_exists(&queries_dir);
        let _ = normalize_json_files_in_dir(&queries_dir);

        let aggregations_dir = repo_root.join("aggregations");
        let _ = normalize_dir_if_exists(&aggregations_dir);
        let _ = normalize_json_files_in_dir(&aggregations_dir);
    }
}

#[cfg(not(unix))]
pub(crate) fn normalize_permissions(_paths: &ConfigPaths) {}

pub(crate) fn ensure_secure_dir(path: &Path) -> Result<()> {
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

pub(crate) fn write_secure_file(path: &Path, contents: &str) -> Result<()> {
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
fn normalize_dir_if_exists(path: &Path) -> Result<()> {
    if path.is_dir() {
        set_dir_permissions(path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn normalize_file_if_exists(path: &Path) -> Result<()> {
    if path.is_file() {
        set_file_permissions(path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn normalize_json_files_in_dir(path: &Path) -> Result<()> {
    let paths = collect_json_paths(path)?;
    for json_path in paths {
        normalize_file_if_exists(&json_path)?;
    }
    Ok(())
}

#[cfg(unix)]
pub(crate) fn permission_warnings(paths: &ConfigPaths) -> Vec<String> {
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
pub(crate) fn permission_warnings(_paths: &ConfigPaths) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::fs;

    use super::{normalize_permissions, permission_warnings};
    use crate::{ConfigPaths, test_support::write_file};

    #[cfg(unix)]
    #[test]
    fn permission_warnings_detect_permissive_modes() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let root = crate::test_support::temp_root("perm_warn");
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

    #[cfg(unix)]
    #[test]
    fn normalize_permissions_clears_permission_warnings() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let root = crate::test_support::temp_root("perm_normalize");
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
        fs::set_permissions(
            &repo_config_root.join("aggregations"),
            fs::Permissions::from_mode(0o755),
        )?;
        let query_path = repo_config_root.join("queries/query.json");
        write_file(&query_path, "{}\n");
        fs::set_permissions(&query_path, fs::Permissions::from_mode(0o644))?;
        let aggregation_path = repo_config_root.join("aggregations/agg.json");
        write_file(&aggregation_path, "[]\n");
        fs::set_permissions(&aggregation_path, fs::Permissions::from_mode(0o644))?;

        let paths = ConfigPaths {
            global_root,
            repo_root: Some(repo_root),
        };
        normalize_permissions(&paths);
        let warnings = permission_warnings(&paths);

        assert!(warnings.is_empty());

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
