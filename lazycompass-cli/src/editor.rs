use anyhow::{Context, Result};
use lazycompass_mongo::{Bson, parse_json_value as parse_bson_value};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn open_in_editor(path: &Path) -> Result<()> {
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let args = parse_editor_command(&editor)?;
    let (program, rest) = args
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("editor command is empty"))?;

    let status = Command::new(program)
        .args(rest)
        .arg(path)
        .status()
        .with_context(|| format!("failed to open editor '{editor}'"))?;

    if !status.success() {
        anyhow::bail!("editor exited with non-zero status");
    }

    Ok(())
}

pub(crate) fn read_document_input(
    label: &str,
    document: Option<String>,
    file: Option<String>,
) -> Result<String> {
    if document.is_some() && file.is_some() {
        anyhow::bail!("--document and --file cannot be used together");
    }
    if let Some(value) = document {
        return Ok(value);
    }
    if let Some(path) = file {
        return fs::read_to_string(&path)
            .with_context(|| format!("unable to read document file {path}"));
    }

    let temp_path = create_secure_temp_file(label, "json", "{}")?;
    open_in_editor(&temp_path)?;
    let contents = fs::read_to_string(&temp_path)
        .with_context(|| format!("unable to read temp file {}", temp_path.display()))?;
    let _ = fs::remove_file(&temp_path);
    if contents.trim().is_empty() {
        anyhow::bail!("document cannot be empty");
    }
    Ok(contents)
}

pub(crate) fn parse_json_value(label: &str, value: &str) -> Result<Bson> {
    parse_bson_value(label, value)
}

pub(crate) fn create_secure_temp_file(
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
        let path = env::temp_dir().join(format!(
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
            .with_context(|| format!("unable to create temp file {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("unable to write temp file {}", path.display()))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("unable to set permissions on {}", path.display()))?;
    }

    #[cfg(not(unix))]
    {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .with_context(|| format!("unable to create temp file {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("unable to write temp file {}", path.display()))?;
    }

    Ok(())
}

fn parse_editor_command(editor: &str) -> Result<Vec<String>> {
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

#[cfg(test)]
mod tests {
    use super::{create_secure_temp_file, parse_json_value, read_document_input};
    use lazycompass_mongo::Bson;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(prefix: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("lazycompass_cli_editor_{prefix}_{nonce}.json"))
    }

    #[test]
    fn read_document_input_rejects_document_and_file_together() {
        let err = read_document_input(
            "insert",
            Some(r#"{"a":1}"#.to_string()),
            Some("doc.json".to_string()),
        )
        .expect_err("expected conflict");
        assert!(
            err.to_string()
                .contains("--document and --file cannot be used together")
        );
    }

    #[test]
    fn read_document_input_reads_document_file() {
        let path = temp_file_path("read_ok");
        fs::write(&path, r#"{"name":"nora"}"#).expect("write test file");

        let result = read_document_input("insert", None, Some(path.to_string_lossy().to_string()))
            .expect("read file");
        assert_eq!(result, r#"{"name":"nora"}"#);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn read_document_input_fails_for_missing_file() {
        let path = temp_file_path("missing");
        let err = read_document_input("insert", None, Some(path.to_string_lossy().to_string()))
            .expect_err("expected read failure");
        assert!(err.to_string().contains("unable to read document file"));
    }

    #[test]
    fn parse_json_value_parses_valid_json() {
        let value = parse_json_value("id", r#""abc""#).expect("parse json");
        assert_eq!(value, Bson::String("abc".to_string()));
    }

    #[test]
    fn parse_json_value_parses_shell_object_id() {
        let value = parse_json_value("id", r#"ObjectId("64e1f2b4c2a3e02c9a0a9c10")"#)
            .expect("parse object id");
        assert!(matches!(value, Bson::ObjectId(_)));
    }

    #[test]
    fn parse_json_value_rejects_invalid_json() {
        let err = parse_json_value("id", "{invalid").expect_err("expected parse failure");
        assert!(err.to_string().contains("invalid JSON in id"));
    }

    #[test]
    fn create_secure_temp_file_writes_contents() {
        let path = create_secure_temp_file("test", "json", "{}").expect("create temp file");
        let contents = fs::read_to_string(&path).expect("read temp file");
        assert_eq!(contents, "{}");
        let _ = fs::remove_file(path);
    }
}
