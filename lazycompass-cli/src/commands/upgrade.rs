use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::UpgradeArgs;

const DEFAULT_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/lucasscarioca/lazycompass/main/install.sh";

pub(crate) fn run_upgrade(args: UpgradeArgs) -> Result<()> {
    let mut installer_args = Vec::new();
    if let Some(version) = args.version {
        installer_args.push("--version".to_string());
        installer_args.push(version);
    }
    if let Some(repo) = args.repo {
        installer_args.push("--repo".to_string());
        installer_args.push(repo);
    }
    if args.from_source {
        installer_args.push("--from-source".to_string());
    }
    if args.no_modify_path {
        installer_args.push("--no-modify-path".to_string());
    }

    if Path::new("install.sh").is_file() {
        let status = Command::new("bash")
            .arg("install.sh")
            .args(&installer_args)
            .status()
            .context("failed to run install.sh")?;
        if !status.success() {
            anyhow::bail!("install.sh exited with non-zero status");
        }
        return Ok(());
    }

    let url = std::env::var("LAZYCOMPASS_INSTALL_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_INSTALL_URL.to_string());

    eprintln!(
        "Warning: running installer from {url}. For stricter verification, download install.sh and run locally."
    );
    eprintln!("Release assets are verified when checksum files are available.");

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let installer_path = env::temp_dir().join(format!("lazycompass_install_{nonce}.sh"));
    let status = Command::new("curl")
        .arg("-fsSL")
        .arg("-o")
        .arg(&installer_path)
        .arg(&url)
        .status()
        .context("failed to download installer script")?;
    if !status.success() {
        anyhow::bail!("failed to download installer script");
    }
    let status = Command::new("bash")
        .arg(&installer_path)
        .args(&installer_args)
        .status()
        .context("failed to run installer from URL")?;
    let _ = fs::remove_file(&installer_path);
    if !status.success() {
        anyhow::bail!("installer exited with non-zero status");
    }
    Ok(())
}
