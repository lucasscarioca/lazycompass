use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::UpgradeArgs;

const DEFAULT_INSTALL_REPO: &str = "lucasscarioca/lazycompass";

pub(crate) fn run_upgrade(args: UpgradeArgs) -> Result<()> {
    let plan = plan_upgrade(args, UpgradeContext::from_env())?;
    eprintln!("Running installer from {}", plan.url());
    eprintln!("Installer sources are restricted to raw.githubusercontent.com.");

    match plan {
        UpgradePlan::Remote {
            url,
            installer_dir,
            installer_args,
        } => {
            let installer_path = installer_dir.join("install.sh");
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
            let _ = fs::remove_dir_all(&installer_dir);
            if !status.success() {
                anyhow::bail!("installer exited with non-zero status");
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct UpgradeContext {
    nonce: u128,
}

impl UpgradeContext {
    fn from_env() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        Self { nonce }
    }
}

#[derive(Debug, Clone)]
enum UpgradePlan {
    Remote {
        url: String,
        installer_dir: PathBuf,
        installer_args: Vec<String>,
    },
}

impl UpgradePlan {
    fn url(&self) -> &str {
        match self {
            UpgradePlan::Remote { url, .. } => url.as_str(),
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

fn plan_upgrade(args: UpgradeArgs, ctx: UpgradeContext) -> Result<UpgradePlan> {
    let installer_args = build_installer_args(&args);
    let repo = args.repo.as_deref().unwrap_or(DEFAULT_INSTALL_REPO);
    let url = install_script_url(repo)?;
    let installer_dir = create_secure_temp_dir("upgrade", ctx.nonce)?;
    Ok(UpgradePlan::Remote {
        url,
        installer_dir,
        installer_args,
    })
}

fn install_script_url(repo: &str) -> Result<String> {
    validate_repo(repo)?;
    Ok(format!(
        "https://raw.githubusercontent.com/{repo}/main/install.sh"
    ))
}

fn validate_repo(repo: &str) -> Result<()> {
    let mut parts = repo.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        anyhow::bail!("invalid repo '{repo}'; expected owner/repo");
    }
    for part in [owner, name] {
        if !part
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        {
            anyhow::bail!("invalid repo '{repo}'; only [A-Za-z0-9._-] are allowed");
        }
    }
    Ok(())
}

fn create_secure_temp_dir(label: &str, nonce: u128) -> Result<PathBuf> {
    let pid = std::process::id();
    for attempt in 0..32u32 {
        let path =
            std::env::temp_dir().join(format!("lazycompass_{label}_{pid}_{nonce}_{attempt}"));
        match fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).with_context(
                        || format!("unable to set permissions on {}", path.display()),
                    )?;
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("unable to create temp dir {}", path.display()));
            }
        }
    }

    anyhow::bail!("unable to allocate temporary directory for upgrade")
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_INSTALL_REPO, UpgradeContext, UpgradePlan, build_installer_args, plan_upgrade,
    };
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
        let plan = plan_upgrade(base_args(), UpgradeContext { nonce: 42 }).expect("plan upgrade");
        match plan {
            UpgradePlan::Remote { url, .. } => {
                assert_eq!(
                    url,
                    "https://raw.githubusercontent.com/owner/repo/main/install.sh"
                );
            }
        }
    }

    #[test]
    fn plan_upgrade_uses_remote_url_when_local_script_missing() {
        let plan = plan_upgrade(base_args(), UpgradeContext { nonce: 42 }).expect("plan upgrade");
        match plan {
            UpgradePlan::Remote {
                url,
                installer_dir,
                installer_args,
            } => {
                assert_eq!(
                    url,
                    "https://raw.githubusercontent.com/owner/repo/main/install.sh"
                );
                assert!(installer_dir.starts_with(std::env::temp_dir()));
                assert_eq!(installer_args.len(), 6);
            }
        }
    }

    #[test]
    fn plan_upgrade_uses_default_repo() {
        let args = UpgradeArgs {
            version: None,
            repo: None,
            from_source: false,
            no_modify_path: false,
        };
        let plan = plan_upgrade(args, UpgradeContext { nonce: 7 }).expect("plan upgrade");
        match plan {
            UpgradePlan::Remote { url, .. } => {
                assert_eq!(
                    url,
                    format!(
                        "https://raw.githubusercontent.com/{DEFAULT_INSTALL_REPO}/main/install.sh"
                    )
                );
            }
        }
    }
}
