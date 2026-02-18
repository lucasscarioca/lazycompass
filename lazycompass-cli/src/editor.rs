use anyhow::{Context, Result};
use lazycompass_mongo::Bson;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn open_in_editor(path: &Path) -> Result<()> {
    let editor = env::var("EDITOR")
        .or_else(|_| env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    let status = Command::new(&editor)
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

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp_path = env::temp_dir().join(format!("lazycompass_{label}_{nonce}.json"));
    fs::write(&temp_path, "{}")
        .with_context(|| format!("unable to create temp file {}", temp_path.display()))?;
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
    let json: serde_json::Value =
        serde_json::from_str(value).with_context(|| format!("invalid JSON in {label}"))?;
    Bson::try_from(json).with_context(|| format!("invalid JSON in {label}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_json_value, read_document_input};
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
    fn parse_json_value_rejects_invalid_json() {
        let err = parse_json_value("id", "{invalid").expect_err("expected parse failure");
        assert!(err.to_string().contains("invalid JSON in id"));
    }
}
