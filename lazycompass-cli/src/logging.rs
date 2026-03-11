use anyhow::{Context, Result};
use lazycompass_core::{Config, LoggingConfig};
use lazycompass_storage::{
    ConfigPaths, ensure_not_symlinked_file, ensure_secure_dir, log_file_path,
};
use std::fs;
use std::path::Path;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::layer::SubscriberExt;

pub(crate) fn init_logging(paths: &ConfigPaths, config: &Config) -> Result<()> {
    let (level, warning) = parse_log_level(config.logging.level.as_deref());
    if let Some(warning) = warning {
        eprintln!("warning: {warning}");
    }
    let filter = Targets::new()
        .with_target("lazycompass", level)
        .with_target("lazycompass_tui", level)
        .with_target("lazycompass_storage", level)
        .with_target("lazycompass_mongo", level)
        .with_target("lazycompass_core", level)
        .with_default(LevelFilter::WARN);
    let log_path = log_file_path(paths, config);
    if let Some(parent) = log_path.parent() {
        ensure_secure_dir(parent)
            .with_context(|| format!("unable to prepare log directory {}", parent.display()))?;
    }
    ensure_not_symlinked_file(&log_path)?;
    rotate_logs_if_needed(&log_path, &config.logging)?;
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("unable to open log file {}", log_path.display()))?;
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("unable to open log file {}", log_path.display()))?;
    let writer = BoxMakeWriter::new(file);
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(false)
        .with_writer(writer);
    let subscriber = tracing_subscriber::registry().with(filter).with(fmt_layer);
    tracing::subscriber::set_global_default(subscriber).context("unable to initialize logging")?;
    Ok(())
}

pub(crate) fn apply_cli_overrides(config: &mut Config, allow_insecure: bool) {
    if allow_insecure {
        config.allow_insecure = Some(true);
    }
}

fn rotate_logs_if_needed(path: &Path, logging: &LoggingConfig) -> Result<()> {
    let max_size = logging.max_size_bytes();
    let max_backups = logging.max_backups();
    if max_backups == 0 {
        return Ok(());
    }

    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("unable to read log metadata {}", path.display()));
        }
    };

    if metadata.len() < max_size {
        return Ok(());
    }

    rotate_log_files(path, max_backups)
}

fn rotate_log_files(path: &Path, max_backups: u64) -> Result<()> {
    if max_backups == 0 {
        return Ok(());
    }

    let oldest = rotated_log_path(path, max_backups);
    if oldest.exists() {
        fs::remove_file(&oldest)
            .with_context(|| format!("unable to remove log file {}", oldest.display()))?;
    }

    for index in (1..max_backups).rev() {
        let from = rotated_log_path(path, index);
        if from.exists() {
            let to = rotated_log_path(path, index + 1);
            fs::rename(&from, &to)
                .with_context(|| format!("unable to rotate log file {}", from.display()))?;
        }
    }

    if path.exists() {
        let first = rotated_log_path(path, 1);
        fs::rename(path, &first)
            .with_context(|| format!("unable to rotate log file {}", path.display()))?;
    }

    Ok(())
}

fn rotated_log_path(path: &Path, index: u64) -> std::path::PathBuf {
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "lazycompass.log".to_string());
    path.with_file_name(format!("{name}.{index}"))
}

fn parse_log_level(level: Option<&str>) -> (LevelFilter, Option<String>) {
    let raw = level.unwrap_or("warn");
    let normalized = raw.trim().to_ascii_lowercase();
    let parsed = match normalized.as_str() {
        "trace" => LevelFilter::TRACE,
        "debug" => LevelFilter::DEBUG,
        "info" => LevelFilter::INFO,
        "warn" | "warning" => LevelFilter::WARN,
        "error" => LevelFilter::ERROR,
        _ => {
            return (
                LevelFilter::WARN,
                Some(format!("invalid log level '{raw}', using warn")),
            );
        }
    };
    (parsed, None)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use super::init_logging;
    use super::{parse_log_level, rotate_log_files, rotated_log_path};
    #[cfg(unix)]
    use lazycompass_core::Config;
    #[cfg(unix)]
    use lazycompass_storage::ConfigPaths;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use tracing_subscriber::filter::LevelFilter;

    fn canonical_temp_dir() -> PathBuf {
        let temp_dir = std::env::temp_dir();
        fs::canonicalize(&temp_dir).unwrap_or(temp_dir)
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = canonical_temp_dir().join(format!("lazycompass_logging_{prefix}_{nonce}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn parse_log_level_accepts_supported_levels() {
        assert_eq!(parse_log_level(Some("trace")).0, LevelFilter::TRACE);
        assert_eq!(parse_log_level(Some("DEBUG")).0, LevelFilter::DEBUG);
        assert_eq!(parse_log_level(Some("info")).0, LevelFilter::INFO);
        assert_eq!(parse_log_level(Some("warning")).0, LevelFilter::WARN);
        assert_eq!(parse_log_level(Some("error")).0, LevelFilter::ERROR);
        assert_eq!(parse_log_level(None).0, LevelFilter::WARN);
    }

    #[test]
    fn parse_log_level_falls_back_to_warn_for_invalid_values() {
        let (level, warning) = parse_log_level(Some("verbose"));
        assert_eq!(level, LevelFilter::WARN);
        let warning = warning.expect("warning");
        assert!(warning.contains("using warn"));
    }

    #[test]
    fn rotated_log_path_appends_numeric_suffix() {
        let path = std::path::Path::new("/tmp/lazycompass.log");
        assert_eq!(
            rotated_log_path(path, 3),
            std::path::Path::new("/tmp/lazycompass.log.3")
        );
    }

    #[test]
    fn rotate_log_files_rotates_and_limits_backups() {
        let dir = temp_dir("rotate");
        let log = dir.join("lazycompass.log");
        fs::write(&log, "current").expect("write current");
        fs::write(log.with_file_name("lazycompass.log.1"), "backup1").expect("write .1");
        fs::write(log.with_file_name("lazycompass.log.2"), "backup2").expect("write .2");

        rotate_log_files(&log, 2).expect("rotate logs");

        assert!(!log.exists());
        assert_eq!(
            fs::read_to_string(log.with_file_name("lazycompass.log.1")).expect("read .1"),
            "current"
        );
        assert_eq!(
            fs::read_to_string(log.with_file_name("lazycompass.log.2")).expect("read .2"),
            "backup1"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn init_logging_rejects_symlinked_log_file() {
        let dir = temp_dir("symlinked_log");
        let target = dir.join("target.log");
        fs::write(&target, "existing").expect("write target");
        let root = dir.join("global");
        fs::create_dir_all(&root).expect("create global root");
        symlink(&target, root.join("lazycompass.log")).expect("create symlink");

        let paths = ConfigPaths {
            global_root: root,
            repo_root: None,
        };
        let err =
            init_logging(&paths, &Config::default()).expect_err("expected symlinked log rejection");
        assert!(err.to_string().contains("symlinked file"));

        let _ = fs::remove_dir_all(&dir);
    }
}
