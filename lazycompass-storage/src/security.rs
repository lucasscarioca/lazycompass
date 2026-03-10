use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use lazycompass_core::redact_sensitive_text;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[cfg(unix)]
use crate::saved_common::collect_json_paths;
use crate::{ConfigPaths, paths::APP_DIR};

#[cfg(unix)]
const DIR_MODE: u32 = 0o700;
#[cfg(unix)]
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

pub fn ensure_secure_dir(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                let file_type = metadata.file_type();
                if file_type.is_symlink() {
                    anyhow::bail!("refusing to use symlinked directory {}", current.display());
                }
                if !file_type.is_dir() {
                    anyhow::bail!("path {} exists and is not a directory", current.display());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .with_context(|| format!("unable to create directory {}", current.display()))?;
                set_dir_permissions(&current)?;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("unable to inspect directory {}", current.display()));
            }
        }
    }

    if let Some(parent) = path.parent()
        && is_config_root_dir(parent)
        && !is_symlink(parent)
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

pub fn write_secure_file(path: &Path, contents: &str, overwrite: bool) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        anyhow::anyhow!("unable to resolve parent directory for {}", path.display())
    })?;
    ensure_secure_dir(parent)?;
    validate_target_file(path, overwrite)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let temp_path = unique_temp_path(parent, path);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(FILE_MODE)
            .open(&temp_path)
            .with_context(|| format!("unable to open file {}", temp_path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("unable to write file {}", temp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("unable to sync file {}", temp_path.display()))?;
        drop(file);
        set_file_permissions(&temp_path)?;
        rename_atomic(&temp_path, path, overwrite)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let temp_path = unique_temp_path(parent, path);
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .with_context(|| format!("unable to open file {}", temp_path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("unable to write file {}", temp_path.display()))?;
        drop(file);
        rename_atomic(&temp_path, path, overwrite)?;
        Ok(())
    }
}

pub fn ensure_not_symlinked_file(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!("refusing to use symlinked file {}", path.display());
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("unable to inspect file {}", path.display()))
        }
    }
}

fn validate_target_file(path: &Path, overwrite: bool) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                anyhow::bail!("refusing to use symlinked file {}", path.display());
            }
            if !file_type.is_file() {
                anyhow::bail!("path {} exists and is not a file", path.display());
            }
            if !overwrite {
                anyhow::bail!("file {} already exists", path.display());
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("unable to inspect file {}", path.display()))
        }
    }
}

fn unique_temp_path(parent: &Path, target: &Path) -> PathBuf {
    let file_name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("lazycompass.tmp");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    parent.join(format!(".{file_name}.{pid}.{nonce}.tmp"))
}

fn rename_atomic(temp_path: &Path, path: &Path, overwrite: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let _ = overwrite;
        fs::rename(temp_path, path).with_context(|| {
            format!(
                "unable to move temporary file {} into {}",
                temp_path.display(),
                path.display()
            )
        })?;
        Ok(())
    }

    #[cfg(not(unix))]
    {
        if overwrite && path.exists() {
            fs::remove_file(path)
                .with_context(|| format!("unable to replace existing file {}", path.display()))?;
        }
        fs::rename(temp_path, path).with_context(|| {
            format!(
                "unable to move temporary file {} into {}",
                temp_path.display(),
                path.display()
            )
        })?;
        Ok(())
    }
}

fn is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
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

#[cfg(unix)]
fn normalize_dir_if_exists(path: &Path) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && metadata.is_dir()
        && !metadata.file_type().is_symlink()
    {
        set_dir_permissions(path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn normalize_file_if_exists(path: &Path) -> Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && metadata.is_file()
        && !metadata.file_type().is_symlink()
    {
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
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        warnings.push(redact_sensitive_text(&format!(
            "permission warning: directory {} is a symlink",
            path.display()
        )));
        return;
    }
    if !metadata.is_dir() {
        return;
    }
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

#[cfg(unix)]
fn append_permission_warning_file(path: &Path, warnings: &mut Vec<String>) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        warnings.push(redact_sensitive_text(&format!(
            "permission warning: file {} is a symlink",
            path.display()
        )));
        return;
    }
    if !metadata.is_file() {
        return;
    }
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

    use super::{ensure_secure_dir, normalize_permissions, permission_warnings, write_secure_file};
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
        write_file(&global_config, "[logging]\nlevel = \"info\"\n");
        fs::set_permissions(&global_config, fs::Permissions::from_mode(0o644))?;

        let repo_root = root.join("repo");
        let repo_config_root = repo_root.join(".lazycompass");
        let queries_dir = repo_config_root.join("queries");
        let aggregations_dir = repo_config_root.join("aggregations");
        fs::create_dir_all(&queries_dir)?;
        fs::create_dir_all(&aggregations_dir)?;
        fs::set_permissions(&repo_config_root, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&queries_dir, fs::Permissions::from_mode(0o755))?;
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
        write_file(&global_config, "[logging]\nlevel = \"info\"\n");
        fs::set_permissions(&global_config, fs::Permissions::from_mode(0o644))?;

        let repo_root = root.join("repo");
        let repo_config_root = repo_root.join(".lazycompass");
        let queries_dir = repo_config_root.join("queries");
        let aggregations_dir = repo_config_root.join("aggregations");
        fs::create_dir_all(&queries_dir)?;
        fs::create_dir_all(&aggregations_dir)?;
        fs::set_permissions(&repo_config_root, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&queries_dir, fs::Permissions::from_mode(0o755))?;
        fs::set_permissions(&aggregations_dir, fs::Permissions::from_mode(0o755))?;
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

    #[cfg(unix)]
    #[test]
    fn ensure_secure_dir_rejects_symlinked_directories() -> Result<()> {
        use std::os::unix::fs::symlink;

        let root = crate::test_support::temp_root("secure_dir_symlink");
        let target = root.join("target");
        fs::create_dir_all(&target)?;
        let link = root.join("link");
        symlink(&target, &link)?;

        let err = ensure_secure_dir(&link).expect_err("expected symlink rejection");
        assert!(err.to_string().contains("symlinked directory"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn write_secure_file_rejects_symlinked_targets() -> Result<()> {
        use std::os::unix::fs::symlink;

        let root = crate::test_support::temp_root("secure_file_symlink");
        let target = root.join("target.txt");
        write_file(&target, "before");
        let link = root.join("config.toml");
        symlink(&target, &link)?;

        let err = write_secure_file(&link, "after", true).expect_err("expected symlink rejection");
        assert!(err.to_string().contains("symlinked file"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
