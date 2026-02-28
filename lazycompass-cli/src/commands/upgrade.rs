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
    let plan = plan_upgrade(args, UpgradeContext::from_env());
    if let Some(url) = plan.warning_url() {
        eprintln!(
            "Warning: running installer from {url}. For stricter verification, download install.sh and run locally."
        );
        eprintln!("Release assets are verified when checksum files are available.");
    }

    match plan {
        UpgradePlan::Local {
            script,
            installer_args,
        } => {
            let status = Command::new("bash")
                .arg(script)
                .args(&installer_args)
                .status()
                .context("failed to run install.sh")?;
            if !status.success() {
                anyhow::bail!("install.sh exited with non-zero status");
            }
        }
        UpgradePlan::Remote {
            url,
            installer_path,
            installer_args,
        } => {
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
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct UpgradeContext {
    local_install_sh_exists: bool,
    install_url_override: Option<String>,
    temp_dir: std::path::PathBuf,
    nonce: u128,
}

impl UpgradeContext {
    fn from_env() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        Self {
            local_install_sh_exists: Path::new("install.sh").is_file(),
            install_url_override: std::env::var("LAZYCOMPASS_INSTALL_URL")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            temp_dir: env::temp_dir(),
            nonce,
        }
    }
}

#[derive(Debug, Clone)]
enum UpgradePlan {
    Local {
        script: &'static str,
        installer_args: Vec<String>,
    },
    Remote {
        url: String,
        installer_path: std::path::PathBuf,
        installer_args: Vec<String>,
    },
}

impl UpgradePlan {
    fn warning_url(&self) -> Option<&str> {
        match self {
            UpgradePlan::Remote { url, .. } => Some(url.as_str()),
            UpgradePlan::Local { .. } => None,
        }
    }
}

fn build_installer_args(args: &UpgradeArgs) -> Vec<String> {
    let mut installer_args = Vec::new();
    if let Some(version) = args.version.as_ref() {
        installer_args.push("--version".to_string());
        installer_args.push(version.clone());
    }
    if let Some(repo) = args.repo.as_ref() {
        installer_args.push("--repo".to_string());
        installer_args.push(repo.clone());
    }
    if args.from_source {
        installer_args.push("--from-source".to_string());
    }
    if args.no_modify_path {
        installer_args.push("--no-modify-path".to_string());
    }
    installer_args
}

fn plan_upgrade(args: UpgradeArgs, ctx: UpgradeContext) -> UpgradePlan {
    let installer_args = build_installer_args(&args);
    if ctx.local_install_sh_exists {
        return UpgradePlan::Local {
            script: "install.sh",
            installer_args,
        };
    }

    let url = ctx
        .install_url_override
        .unwrap_or_else(|| DEFAULT_INSTALL_URL.to_string());
    let installer_path = ctx
        .temp_dir
        .join(format!("lazycompass_install_{}.sh", ctx.nonce));
    UpgradePlan::Remote {
        url,
        installer_path,
        installer_args,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{UpgradeContext, UpgradePlan, build_installer_args, plan_upgrade};
    use crate::cli::UpgradeArgs;

    fn base_args() -> UpgradeArgs {
        UpgradeArgs {
            version: Some("1.2.3".to_string()),
            repo: Some("owner/repo".to_string()),
            from_source: true,
            no_modify_path: true,
        }
    }

    #[test]
    fn build_installer_args_preserves_flag_order() {
        assert_eq!(
            build_installer_args(&base_args()),
            vec![
                "--version",
                "1.2.3",
                "--repo",
                "owner/repo",
                "--from-source",
                "--no-modify-path",
            ]
        );
    }

    #[test]
    fn plan_upgrade_prefers_local_install_script() {
        let plan = plan_upgrade(
            base_args(),
            UpgradeContext {
                local_install_sh_exists: true,
                install_url_override: Some("https://example.com/install.sh".to_string()),
                temp_dir: PathBuf::from("/tmp"),
                nonce: 42,
            },
        );
        assert!(matches!(
            plan,
            UpgradePlan::Local {
                script: "install.sh",
                ..
            }
        ));
    }

    #[test]
    fn plan_upgrade_uses_remote_url_when_local_script_missing() {
        let plan = plan_upgrade(
            base_args(),
            UpgradeContext {
                local_install_sh_exists: false,
                install_url_override: Some("https://example.com/install.sh".to_string()),
                temp_dir: PathBuf::from("/tmp"),
                nonce: 42,
            },
        );
        match plan {
            UpgradePlan::Remote {
                url,
                installer_path,
                installer_args,
            } => {
                assert_eq!(url, "https://example.com/install.sh");
                assert_eq!(
                    installer_path,
                    PathBuf::from("/tmp/lazycompass_install_42.sh")
                );
                assert_eq!(installer_args.len(), 6);
            }
            UpgradePlan::Local { .. } => panic!("expected remote plan"),
        }
    }
}
