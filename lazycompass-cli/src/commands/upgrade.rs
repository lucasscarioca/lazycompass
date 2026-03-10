use anyhow::{Context, Result};
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cli::UpgradeArgs;

const APP: &str = "lazycompass";
const DEFAULT_INSTALL_REPO: &str = "lucasscarioca/lazycompass";

pub(crate) fn run_upgrade(args: UpgradeArgs) -> Result<()> {
    let plan = plan_upgrade(args, UpgradeContext::from_env())?;

    if plan.no_modify_path() {
        eprintln!("--no-modify-path is ignored; upgrade no longer edits shell profiles.");
    }

    match &plan {
        UpgradePlan::Release(plan) => {
            eprintln!(
                "Downloading verified release asset from {}",
                plan.archive_url
            );
            eprintln!("Replacing binary at {}", plan.install_path.display());
        }
        UpgradePlan::Source(plan) => {
            eprintln!("Installing from source via cargo from {}", plan.git_url);
        }
    }

    match plan {
        UpgradePlan::Release(plan) => run_release_upgrade(plan),
        UpgradePlan::Source(plan) => run_source_upgrade(plan),
    }
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
    Release(ReleaseUpgradePlan),
    Source(SourceUpgradePlan),
}

impl UpgradePlan {
    fn no_modify_path(&self) -> bool {
        match self {
            UpgradePlan::Release(plan) => plan.no_modify_path,
            UpgradePlan::Source(plan) => plan.no_modify_path,
        }
    }
}

#[derive(Debug, Clone)]
struct ReleaseUpgradePlan {
    target: ReleaseTarget,
    archive_url: String,
    checksum_url: String,
    checksum_sig_url: String,
    asset_name: String,
    install_path: PathBuf,
    temp_dir: PathBuf,
    no_modify_path: bool,
}

#[derive(Debug, Clone)]
struct SourceUpgradePlan {
    git_url: String,
    install_root: PathBuf,
    version: Option<String>,
    no_modify_path: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReleaseTarget {
    slug: &'static str,
    archive_ext: &'static str,
    binary_name: &'static str,
}

fn plan_upgrade(args: UpgradeArgs, ctx: UpgradeContext) -> Result<UpgradePlan> {
    let install_path = env::current_exe().context("unable to resolve current executable")?;
    plan_upgrade_with_install_path(args, ctx, install_path)
}

fn plan_upgrade_with_install_path(
    args: UpgradeArgs,
    ctx: UpgradeContext,
    install_path: PathBuf,
) -> Result<UpgradePlan> {
    let repo = args.repo.as_deref().unwrap_or(DEFAULT_INSTALL_REPO);
    validate_repo(repo)?;

    if args.from_source {
        let install_root = infer_install_root(&install_path)?;
        return Ok(UpgradePlan::Source(SourceUpgradePlan {
            git_url: git_repo_url(repo),
            install_root,
            version: normalize_version(args.version.as_deref())?,
            no_modify_path: args.no_modify_path,
        }));
    }

    let target = detect_release_target()?;
    let asset_name = format!("{APP}-{}.{}", target.slug, target.archive_ext);
    let version = normalize_version(args.version.as_deref())?;
    let archive_url = release_archive_url(repo, version.as_deref(), &asset_name);
    let checksum_url = format!("{archive_url}.sha256");
    let checksum_sig_url = format!("{checksum_url}.sig");
    let temp_dir = create_secure_temp_dir("upgrade", ctx.nonce)?;
    Ok(UpgradePlan::Release(ReleaseUpgradePlan {
        target,
        archive_url,
        checksum_url,
        checksum_sig_url,
        asset_name,
        install_path,
        temp_dir,
        no_modify_path: args.no_modify_path,
    }))
}

fn normalize_version(version: Option<&str>) -> Result<Option<String>> {
    match version {
        Some(value) => {
            let normalized = value.trim().trim_start_matches('v').to_string();
            if normalized.is_empty() {
                anyhow::bail!("invalid version '{value}'");
            }
            Ok(Some(normalized))
        }
        None => Ok(None),
    }
}

fn release_archive_url(repo: &str, version: Option<&str>, asset_name: &str) -> String {
    match version {
        Some(version) => {
            format!("https://github.com/{repo}/releases/download/v{version}/{asset_name}")
        }
        None => format!("https://github.com/{repo}/releases/latest/download/{asset_name}"),
    }
}

fn git_repo_url(repo: &str) -> String {
    format!("https://github.com/{repo}")
}

fn detect_release_target() -> Result<ReleaseTarget> {
    detect_release_target_from(env::consts::OS, env::consts::ARCH)
}

fn detect_release_target_from(os: &str, arch: &str) -> Result<ReleaseTarget> {
    match (os, arch) {
        ("linux", "x86_64") => Ok(ReleaseTarget {
            slug: "linux-x64",
            archive_ext: "tar.gz",
            binary_name: APP,
        }),
        ("macos", "x86_64") => Ok(ReleaseTarget {
            slug: "darwin-x64",
            archive_ext: "tar.gz",
            binary_name: APP,
        }),
        ("macos", "aarch64") => Ok(ReleaseTarget {
            slug: "darwin-arm64",
            archive_ext: "tar.gz",
            binary_name: APP,
        }),
        ("windows", "x86_64") => Ok(ReleaseTarget {
            slug: "windows-x64",
            archive_ext: "zip",
            binary_name: "lazycompass.exe",
        }),
        _ => anyhow::bail!("unsupported platform {os}/{arch} for release upgrades"),
    }
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

fn infer_install_root(install_path: &Path) -> Result<PathBuf> {
    let bin_dir = install_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "unable to resolve install directory for current binary {}",
            install_path.display()
        )
    })?;
    if bin_dir.file_name().and_then(|name| name.to_str()) != Some("bin") {
        anyhow::bail!(
            "--from-source requires the current binary to live under a bin/ directory; found {}",
            install_path.display()
        );
    }
    bin_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
        anyhow::anyhow!(
            "unable to resolve install root for current binary {}",
            install_path.display()
        )
    })
}

fn run_release_upgrade(plan: ReleaseUpgradePlan) -> Result<()> {
    let archive_path = plan.temp_dir.join(&plan.asset_name);
    let checksum_path = plan.temp_dir.join(format!("{}.sha256", plan.asset_name));
    let signature_path = plan
        .temp_dir
        .join(format!("{}.sha256.sig", plan.asset_name));

    let result = (|| {
        download_file(&plan.archive_url, &archive_path)
            .with_context(|| format!("failed to download {}", plan.archive_url))?;
        download_file(&plan.checksum_url, &checksum_path)
            .with_context(|| format!("failed to download {}", plan.checksum_url))?;
        verify_checksum(&checksum_path, &archive_path)?;

        if download_optional_file(&plan.checksum_sig_url, &signature_path)? {
            verify_signature(&checksum_path, &signature_path)?;
        }

        extract_archive(&plan.target, &archive_path, &plan.temp_dir)?;
        let extracted_binary = plan.temp_dir.join(plan.target.binary_name);
        if !extracted_binary.is_file() {
            anyhow::bail!(
                "expected binary '{}' in downloaded archive",
                plan.target.binary_name
            );
        }

        replace_binary(&extracted_binary, &plan.install_path)?;
        Ok(())
    })();

    let _ = fs::remove_dir_all(&plan.temp_dir);
    result?;
    eprintln!("Upgrade complete.");
    Ok(())
}

fn run_source_upgrade(plan: SourceUpgradePlan) -> Result<()> {
    require_command("cargo")?;

    let mut command = Command::new("cargo");
    command
        .arg("install")
        .arg("--git")
        .arg(&plan.git_url)
        .arg("-p")
        .arg(APP)
        .arg("--locked")
        .arg("--root")
        .arg(&plan.install_root);
    if let Some(version) = plan.version.as_ref() {
        command.arg("--tag").arg(format!("v{version}"));
    }

    let status = command.status().context("failed to run cargo install")?;
    if !status.success() {
        anyhow::bail!("cargo install exited with non-zero status");
    }

    eprintln!("Source upgrade complete.");
    Ok(())
}

fn require_command(command: &str) -> Result<()> {
    if find_command(command).is_none() {
        anyhow::bail!("{command} is required but not installed");
    }
    Ok(())
}

fn find_command(command: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    let candidates = command_candidates(command);
    env::split_paths(&path)
        .flat_map(|dir| candidates.iter().map(move |candidate| dir.join(candidate)))
        .find(|candidate| candidate.is_file())
}

fn command_candidates(command: &str) -> Vec<OsString> {
    let command_path = Path::new(command);
    if command_path.extension().is_some() {
        return vec![OsString::from(command)];
    }

    #[cfg(windows)]
    {
        let path_ext = env::var_os("PATHEXT")
            .map(|value| {
                env::split_paths(&value)
                    .map(|value| value.into_os_string())
                    .collect::<Vec<_>>()
            })
            .filter(|value| !value.is_empty());
        let mut candidates = vec![OsString::from(command)];
        for ext in path_ext.unwrap_or_else(|| {
            vec![
                OsString::from(".COM"),
                OsString::from(".EXE"),
                OsString::from(".BAT"),
                OsString::from(".CMD"),
            ]
        }) {
            let ext = ext.to_string_lossy();
            candidates.push(OsString::from(format!("{command}{ext}")));
        }
        candidates
    }

    #[cfg(not(windows))]
    {
        vec![OsString::from(command)]
    }
}

fn download_file(url: &str, destination: &Path) -> Result<()> {
    if find_command("curl").is_some() {
        let status = Command::new("curl")
            .arg("-fsSL")
            .arg("-o")
            .arg(destination)
            .arg(url)
            .status()
            .context("failed to execute curl")?;
        if !status.success() {
            anyhow::bail!("curl exited with non-zero status");
        }
        return Ok(());
    }

    #[cfg(windows)]
    {
        require_command("powershell")?;
        let command = format!(
            "Invoke-WebRequest -UseBasicParsing -Uri '{}' -OutFile '{}'",
            escape_powershell_single_quoted(url),
            escape_powershell_single_quoted(&destination.display().to_string())
        );
        let status = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(command)
            .status()
            .context("failed to execute powershell download")?;
        if !status.success() {
            anyhow::bail!("powershell download exited with non-zero status");
        }
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        anyhow::bail!("curl is required but not installed")
    }
}

fn download_optional_file(url: &str, destination: &Path) -> Result<bool> {
    if find_command("curl").is_some() {
        let status = Command::new("curl")
            .arg("-fsL")
            .arg("-o")
            .arg(destination)
            .arg(url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to execute curl")?;
        return Ok(status.success());
    }

    #[cfg(windows)]
    {
        require_command("powershell")?;
        let command = format!(
            "try {{ Invoke-WebRequest -UseBasicParsing -Uri '{}' -OutFile '{}' | Out-Null; exit 0 }} catch {{ exit 1 }}",
            escape_powershell_single_quoted(url),
            escape_powershell_single_quoted(&destination.display().to_string())
        );
        let status = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-NonInteractive")
            .arg("-Command")
            .arg(command)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to execute powershell download")?;
        return Ok(status.success());
    }

    #[cfg(not(windows))]
    {
        Ok(false)
    }
}

fn verify_checksum(checksum_path: &Path, archive_path: &Path) -> Result<()> {
    let expected = read_expected_checksum(checksum_path)?;
    let actual = compute_sha256(archive_path)?;
    if expected != actual {
        anyhow::bail!(
            "checksum verification failed for {}",
            archive_path.display()
        );
    }
    Ok(())
}

fn read_expected_checksum(path: &Path) -> Result<String> {
    let mut contents = String::new();
    File::open(path)
        .with_context(|| format!("unable to open checksum file {}", path.display()))?
        .read_to_string(&mut contents)
        .with_context(|| format!("unable to read checksum file {}", path.display()))?;
    contents
        .split_whitespace()
        .next()
        .map(str::to_string)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("checksum file {} is empty or invalid", path.display()))
}

fn compute_sha256(path: &Path) -> Result<String> {
    if find_command("sha256sum").is_some() {
        return command_output_digest(Command::new("sha256sum").arg(path));
    }
    if find_command("shasum").is_some() {
        return command_output_digest(Command::new("shasum").arg("-a").arg("256").arg(path));
    }

    #[cfg(windows)]
    {
        if find_command("powershell").is_some() {
            let output = Command::new("powershell")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg(format!(
                    "(Get-FileHash -Algorithm SHA256 -LiteralPath '{}').Hash.ToLowerInvariant()",
                    escape_powershell_single_quoted(&path.display().to_string())
                ))
                .output()
                .context("failed to compute sha256 digest with powershell")?;
            if !output.status.success() {
                anyhow::bail!("powershell sha256 command exited with non-zero status");
            }
            let stdout = String::from_utf8(output.stdout)
                .context("powershell sha256 output was not valid UTF-8")?;
            let digest = stdout.trim();
            if digest.is_empty() {
                anyhow::bail!("powershell sha256 command returned empty output");
            }
            return Ok(digest.to_string());
        }
    }

    anyhow::bail!("sha256sum, shasum, or powershell is required to verify release assets")
}

fn command_output_digest(command: &mut Command) -> Result<String> {
    let output = command
        .output()
        .context("failed to compute sha256 digest")?;
    if !output.status.success() {
        anyhow::bail!("sha256 command exited with non-zero status");
    }
    let stdout = String::from_utf8(output.stdout).context("sha256 output was not valid UTF-8")?;
    stdout
        .split_whitespace()
        .next()
        .map(str::to_string)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("sha256 command returned empty output"))
}

fn verify_signature(checksum_path: &Path, signature_path: &Path) -> Result<()> {
    if find_command("gpg").is_none() {
        eprintln!("Checksum signature present, but gpg is not installed; skipping verification.");
        return Ok(());
    }

    let status = Command::new("gpg")
        .arg("--verify")
        .arg(signature_path)
        .arg(checksum_path)
        .status()
        .context("failed to run gpg --verify")?;
    if !status.success() {
        anyhow::bail!("checksum signature verification failed");
    }
    Ok(())
}

fn extract_archive(target: &ReleaseTarget, archive_path: &Path, destination: &Path) -> Result<()> {
    match target.archive_ext {
        "tar.gz" => {
            require_command("tar")?;
            let status = Command::new("tar")
                .arg("-xzf")
                .arg(archive_path)
                .arg("-C")
                .arg(destination)
                .status()
                .context("failed to run tar")?;
            if !status.success() {
                anyhow::bail!("tar exited with non-zero status");
            }
        }
        "zip" => {
            require_command("powershell")?;
            let status = Command::new("powershell")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg(format!(
                    "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
                    escape_powershell_single_quoted(&archive_path.display().to_string()),
                    escape_powershell_single_quoted(&destination.display().to_string())
                ))
                .status()
                .context("failed to run powershell Expand-Archive")?;
            if !status.success() {
                anyhow::bail!("powershell Expand-Archive exited with non-zero status");
            }
        }
        other => anyhow::bail!("unsupported archive format {other}"),
    }
    Ok(())
}

fn replace_binary(source: &Path, destination: &Path) -> Result<()> {
    let parent = destination.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "unable to resolve install directory for {}",
            destination.display()
        )
    })?;
    let temp_path = sibling_temp_path(destination);
    copy_file(source, &temp_path)?;
    #[cfg(not(unix))]
    if destination.exists() {
        fs::remove_file(destination).with_context(|| {
            format!(
                "unable to replace existing binary {}",
                destination.display()
            )
        })?;
    }
    fs::rename(&temp_path, destination).with_context(|| {
        format!(
            "unable to replace {} with {}",
            destination.display(),
            source.display()
        )
    })?;
    sync_directory(parent)?;
    Ok(())
}

fn sibling_temp_path(destination: &Path) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(APP);
    let pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    destination.with_file_name(format!(".{file_name}.{pid}.{nonce}.tmp"))
}

fn copy_file(source: &Path, destination: &Path) -> Result<()> {
    let mut input = File::open(source)
        .with_context(|| format!("unable to open extracted binary {}", source.display()))?;

    #[cfg(unix)]
    let mut output = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o755)
            .open(destination)
            .with_context(|| format!("unable to open temp binary {}", destination.display()))?
    };

    #[cfg(not(unix))]
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .with_context(|| format!("unable to open temp binary {}", destination.display()))?;

    io::copy(&mut input, &mut output)
        .with_context(|| format!("unable to copy binary into {}", destination.display()))?;
    output
        .sync_all()
        .with_context(|| format!("unable to sync temp binary {}", destination.display()))?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        File::open(path)
            .with_context(|| format!("unable to open directory {}", path.display()))?
            .sync_all()
            .with_context(|| format!("unable to sync directory {}", path.display()))?;
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

fn escape_powershell_single_quoted(value: &str) -> String {
    value.replace('\'', "''")
}

#[cfg(test)]
mod tests {
    use super::{
        APP, DEFAULT_INSTALL_REPO, ReleaseUpgradePlan, SourceUpgradePlan, UpgradeContext,
        UpgradePlan, detect_release_target_from, infer_install_root, normalize_version,
        plan_upgrade, plan_upgrade_with_install_path, read_expected_checksum, release_archive_url,
    };
    use crate::cli::UpgradeArgs;
    use std::path::Path;

    fn base_args() -> UpgradeArgs {
        UpgradeArgs {
            version: Some("1.2.3".to_string()),
            repo: Some("owner/repo".to_string()),
            from_source: false,
            no_modify_path: true,
        }
    }

    #[test]
    fn normalize_version_strips_v_prefix() {
        assert_eq!(
            normalize_version(Some("v1.2.3")).expect("normalized"),
            Some("1.2.3".to_string())
        );
    }

    #[test]
    fn detect_release_target_supports_linux_x64() {
        assert_eq!(
            detect_release_target_from("linux", "x86_64")
                .expect("target")
                .slug,
            "linux-x64"
        );
    }

    #[test]
    fn detect_release_target_supports_macos_arm64() {
        assert_eq!(
            detect_release_target_from("macos", "aarch64")
                .expect("target")
                .slug,
            "darwin-arm64"
        );
    }

    #[test]
    fn detect_release_target_supports_windows_x64() {
        let target = detect_release_target_from("windows", "x86_64").expect("target");
        assert_eq!(target.slug, "windows-x64");
        assert_eq!(target.archive_ext, "zip");
        assert_eq!(target.binary_name, "lazycompass.exe");
    }

    #[test]
    fn release_archive_url_supports_windows_zip_assets() {
        assert_eq!(
            release_archive_url("owner/repo", Some("1.2.3"), "lazycompass-windows-x64.zip"),
            "https://github.com/owner/repo/releases/download/v1.2.3/lazycompass-windows-x64.zip"
        );
    }

    #[test]
    fn release_archive_url_uses_versioned_tag() {
        assert_eq!(
            release_archive_url("owner/repo", Some("1.2.3"), "lazycompass-linux-x64.tar.gz"),
            "https://github.com/owner/repo/releases/download/v1.2.3/lazycompass-linux-x64.tar.gz"
        );
    }

    #[test]
    fn infer_install_root_uses_parent_of_bin_dir() {
        let root = infer_install_root(Path::new("/tmp/lazycompass-test/bin/lazycompass"))
            .expect("install root");
        assert_eq!(root, Path::new("/tmp/lazycompass-test"));
    }

    #[test]
    fn read_expected_checksum_uses_first_token() {
        let dir =
            std::env::temp_dir().join(format!("lazycompass_checksum_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let path = dir.join("asset.sha256");
        std::fs::write(&path, "abc123  lazycompass-linux-x64.tar.gz\n").expect("write checksum");
        let digest = read_expected_checksum(&path).expect("digest");
        assert_eq!(digest, "abc123");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn plan_release_upgrade_uses_default_repo() {
        let args = UpgradeArgs {
            version: None,
            repo: None,
            from_source: false,
            no_modify_path: false,
        };
        let plan = plan_upgrade(args, UpgradeContext { nonce: 7 }).expect("plan upgrade");
        match plan {
            UpgradePlan::Release(ReleaseUpgradePlan {
                archive_url,
                asset_name,
                no_modify_path,
                ..
            }) => {
                assert_eq!(
                    archive_url,
                    release_archive_url(
                        DEFAULT_INSTALL_REPO,
                        None,
                        &format!(
                            "{APP}-{}.{}",
                            detect_release_target_from(
                                std::env::consts::OS,
                                std::env::consts::ARCH
                            )
                            .expect("target")
                            .slug,
                            detect_release_target_from(
                                std::env::consts::OS,
                                std::env::consts::ARCH
                            )
                            .expect("target")
                            .archive_ext,
                        )
                    )
                );
                assert!(!asset_name.is_empty());
                assert!(!no_modify_path);
            }
            UpgradePlan::Source(_) => panic!("expected release plan"),
        }
    }

    #[test]
    fn plan_release_upgrade_preserves_version_and_repo() {
        let plan = plan_upgrade(base_args(), UpgradeContext { nonce: 42 }).expect("plan upgrade");
        match plan {
            UpgradePlan::Release(ReleaseUpgradePlan {
                archive_url,
                checksum_url,
                checksum_sig_url,
                no_modify_path,
                ..
            }) => {
                assert!(archive_url.contains("owner/repo"));
                assert!(archive_url.contains("/releases/download/v1.2.3/"));
                assert_eq!(checksum_url, format!("{archive_url}.sha256"));
                assert_eq!(checksum_sig_url, format!("{checksum_url}.sig"));
                assert!(archive_url.ends_with(".tar.gz") || archive_url.ends_with(".zip"));
                assert!(no_modify_path);
            }
            UpgradePlan::Source(_) => panic!("expected release plan"),
        }
    }

    #[test]
    fn plan_source_upgrade_uses_git_repo_url() {
        let args = UpgradeArgs {
            from_source: true,
            ..base_args()
        };
        let plan = plan_upgrade_with_install_path(
            args,
            UpgradeContext { nonce: 42 },
            Path::new("/tmp/lazycompass-test/bin/lazycompass").to_path_buf(),
        )
        .expect("plan upgrade");
        match plan {
            UpgradePlan::Source(SourceUpgradePlan {
                git_url,
                version,
                no_modify_path,
                ..
            }) => {
                assert_eq!(git_url, "https://github.com/owner/repo");
                assert_eq!(version.as_deref(), Some("1.2.3"));
                assert!(no_modify_path);
            }
            UpgradePlan::Release(_) => panic!("expected source plan"),
        }
    }
}
