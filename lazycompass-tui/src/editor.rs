use super::*;

pub(crate) fn resolve_editor() -> Result<String> {
    std::env::var("VISUAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or_else(|| anyhow::anyhow!("$VISUAL or $EDITOR is required for editing"))
}

pub(crate) fn parse_editor_command(editor: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = editor.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
            continue;
        }

        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    let next = chars
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("unterminated escape in editor command"))?;
                    current.push(next);
                }
                _ => current.push(ch),
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '\\' => {
                let next = chars
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("unterminated escape in editor command"))?;
                current.push(next);
            }
            ch if ch.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if in_single || in_double {
        anyhow::bail!("unterminated quote in editor command");
    }
    if !current.is_empty() {
        args.push(current);
    }
    if args.is_empty() {
        anyhow::bail!("editor command is empty");
    }
    Ok(args)
}

pub(crate) fn run_editor_command(editor: &str, path: &Path) -> Result<std::process::ExitStatus> {
    let args = parse_editor_command(editor)?;
    let (program, rest) = args
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("editor command is empty"))?;
    Command::new(program)
        .args(rest)
        .arg(path)
        .status()
        .context("failed to launch editor")
}

pub(crate) fn create_secure_editor_temp_file(
    label: &str,
    extension: &str,
    contents: &str,
) -> Result<PathBuf> {
    let pid = std::process::id();
    for attempt in 0..32u32 {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "lazycompass_{label}_{pid}_{nonce}_{attempt}.{extension}"
        ));
        match write_new_temp_file(&path, contents) {
            Ok(()) => return Ok(path),
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::AlreadyExists) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        }
    }

    anyhow::bail!("unable to allocate temporary file for {label}")
}

fn write_new_temp_file(path: &Path, contents: &str) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::fs::PermissionsExt;
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .with_context(|| format!("unable to create temporary file {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("unable to write temporary file {}", path.display()))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("unable to set permissions on {}", path.display()))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .with_context(|| format!("unable to create temporary file {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("unable to write temporary file {}", path.display()))?;
        Ok(())
    }
}

pub(crate) fn is_editor_cancelled(contents: &str, initial: &str) -> bool {
    let trimmed = contents.trim();
    trimmed.is_empty() || trimmed == initial.trim()
}

#[cfg(test)]
mod tests {
    use super::{create_secure_editor_temp_file, parse_editor_command, write_new_temp_file};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_editor_command_handles_quotes() {
        let args = parse_editor_command("nvim -c \"set ft=json\"").expect("parse editor command");
        assert_eq!(args, vec!["nvim", "-c", "set ft=json"]);
        let args = parse_editor_command("code --wait").expect("parse editor command");
        assert_eq!(args, vec!["code", "--wait"]);
        let args = parse_editor_command("edit 'arg with spaces'").expect("parse editor command");
        assert_eq!(args, vec!["edit", "arg with spaces"]);
    }

    #[test]
    fn parse_editor_command_rejects_unclosed_quotes() {
        assert!(parse_editor_command("nvim -c \"oops").is_err());
    }

    #[test]
    fn create_secure_editor_temp_file_writes_contents() {
        let path =
            create_secure_editor_temp_file("query", "json", "{\"filter\":{}}").expect("create");
        let contents = fs::read_to_string(&path).expect("read temp file");
        assert_eq!(contents, "{\"filter\":{}}");
        let _ = fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn write_new_temp_file_rejects_symlink_targets() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "lazycompass_tui_editor_test_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create root");
        let target = root.join("target.json");
        let link = root.join("link.json");
        fs::write(&target, "old").expect("write target");
        symlink(&target, &link).expect("create symlink");

        let err = write_new_temp_file(&link, "{}").expect_err("expected symlink failure");
        assert!(
            err.downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::AlreadyExists)
        );
        assert_eq!(fs::read_to_string(&target).expect("read target"), "old");

        let _ = fs::remove_dir_all(root);
    }
}
