use anyhow::{Context, Result};
use lazycompass_core::OutputFormat;
use lazycompass_mongo::{
    Bson, Document, render_relaxed_extjson_documents, render_relaxed_extjson_string,
};
use lazycompass_storage::{ensure_not_symlinked_file, ensure_not_symlinked_path};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportNameSource {
    SavedQuery { name: String },
    SavedAggregation { name: String },
    InlineQuery,
    InlineAggregation,
}

pub fn render_documents(format: OutputFormat, documents: &[Document]) -> Result<String> {
    match format {
        OutputFormat::JsonPretty => render_relaxed_extjson_documents(documents),
        OutputFormat::Table => Ok(format_table(documents)),
        OutputFormat::Csv => Ok(format_csv(documents)),
    }
}

pub fn write_documents(
    format: OutputFormat,
    documents: &[Document],
    output_path: &Path,
) -> Result<()> {
    let output = render_documents(format, documents)?;
    write_rendered_output(output_path, &output)
}

pub fn write_rendered_output(output_path: &Path, output: &str) -> Result<()> {
    let parent = output_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "unable to resolve parent directory for {}",
            output_path.display()
        )
    })?;
    ensure_not_symlinked_path(parent)?;
    ensure_not_symlinked_file(output_path)?;
    write_rendered_output_atomically(output_path, output)
}

fn write_rendered_output_atomically(output_path: &Path, output: &str) -> Result<()> {
    let parent = output_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "unable to resolve parent directory for {}",
            output_path.display()
        )
    })?;
    let temp_path = sibling_temp_path(output_path);

    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&temp_path)
            .with_context(|| format!("unable to open output file {}", temp_path.display()))?
    };

    #[cfg(not(unix))]
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .with_context(|| format!("unable to open output file {}", temp_path.display()))?;

    use std::io::Write;
    file.write_all(output.as_bytes())
        .with_context(|| format!("unable to write output file {}", temp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("unable to sync output file {}", temp_path.display()))?;
    drop(file);

    if output_path.exists() {
        fs::remove_file(output_path)
            .with_context(|| format!("unable to replace output file {}", output_path.display()))?;
    }
    fs::rename(&temp_path, output_path)
        .with_context(|| format!("unable to write output file {}", output_path.display()))?;

    #[cfg(unix)]
    {
        use std::fs::File;
        File::open(parent)
            .with_context(|| format!("unable to open directory {}", parent.display()))?
            .sync_all()
            .with_context(|| format!("unable to sync directory {}", parent.display()))?;
    }

    Ok(())
}

fn sibling_temp_path(path: &Path) -> std::path::PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("lazycompass-output.tmp");
    let pid = std::process::id();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_file_name(format!(".{file_name}.{pid}.{nonce}.tmp"))
}

pub fn suggested_export_filename(
    source: &ExportNameSource,
    format: OutputFormat,
    single_document: bool,
) -> String {
    let base = match source {
        ExportNameSource::SavedQuery { name } | ExportNameSource::SavedAggregation { name } => {
            sanitize_filename(name)
        }
        ExportNameSource::InlineQuery => "query_results".to_string(),
        ExportNameSource::InlineAggregation => "aggregation_results".to_string(),
    };
    let mut base = if base.is_empty() {
        "results".to_string()
    } else {
        base
    };
    if single_document {
        base.push_str("_document");
    }
    format!("{base}.{}", format_extension(format))
}

pub fn format_bson_scalar(value: &Bson) -> String {
    format_scalar(value)
}

fn format_extension(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::JsonPretty => "json",
        OutputFormat::Csv => "csv",
        OutputFormat::Table => "txt",
    }
}

fn sanitize_filename(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    trimmed
        .chars()
        .map(|ch| match ch {
            '/' | '\\' => '_',
            _ => ch,
        })
        .collect()
}

fn format_table(documents: &[Document]) -> String {
    if documents.is_empty() {
        return "no results".to_string();
    }

    let columns = scalar_columns(documents);
    if columns.is_empty() {
        return "no scalar fields to display".to_string();
    }

    let mut rows = Vec::with_capacity(documents.len());
    for document in documents {
        let mut row = Vec::with_capacity(columns.len());
        for column in &columns {
            let cell = match document.get(column) {
                Some(value) if is_scalar(value) => format_scalar(value),
                _ => String::new(),
            };
            row.push(cell);
        }
        rows.push(row);
    }

    let widths = column_widths(&columns, &rows);
    let mut output = String::new();
    output.push_str(&format_row(&columns, &widths));
    output.push('\n');
    output.push_str(&format_separator(&widths));
    for row in rows {
        output.push('\n');
        output.push_str(&format_row(&row, &widths));
    }
    output
}

fn format_csv(documents: &[Document]) -> String {
    let columns = all_columns(documents);
    if columns.is_empty() {
        return String::new();
    }

    let mut lines = Vec::with_capacity(documents.len() + 1);
    lines.push(
        columns
            .iter()
            .map(|column| escape_csv_cell(column))
            .collect::<Vec<_>>()
            .join(","),
    );
    for document in documents {
        let row = columns
            .iter()
            .map(|column| {
                document
                    .get(column)
                    .map(format_csv_value)
                    .unwrap_or_default()
            })
            .map(|cell| escape_csv_cell(&cell))
            .collect::<Vec<_>>()
            .join(",");
        lines.push(row);
    }
    lines.join("\n")
}

fn scalar_columns(documents: &[Document]) -> Vec<String> {
    let mut columns = BTreeSet::new();
    for document in documents {
        for (key, value) in document {
            if is_scalar(value) {
                columns.insert(key.to_string());
            }
        }
    }
    columns.into_iter().collect()
}

fn all_columns(documents: &[Document]) -> Vec<String> {
    let mut columns = BTreeSet::new();
    for document in documents {
        for key in document.keys() {
            columns.insert(key.to_string());
        }
    }
    columns.into_iter().collect()
}

fn column_widths(headers: &[String], rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if cell.len() > widths[index] {
                widths[index] = cell.len();
            }
        }
    }
    widths
}

fn format_row(cells: &[String], widths: &[usize]) -> String {
    let mut row = String::new();
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            row.push_str(" | ");
        }
        let width = widths[index];
        row.push_str(&format!("{cell:width$}", width = width));
    }
    row
}

fn format_separator(widths: &[usize]) -> String {
    let mut line = String::new();
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            line.push_str("-+-");
        }
        line.push_str(&"-".repeat(*width));
    }
    line
}

fn format_csv_value(value: &Bson) -> String {
    match value {
        Bson::Document(document) => Bson::Document(document.clone())
            .into_relaxed_extjson()
            .to_string(),
        Bson::Array(values) => Bson::Array(values.clone())
            .into_relaxed_extjson()
            .to_string(),
        _ => format_scalar(value),
    }
}

fn escape_csv_cell(value: &str) -> String {
    let value = neutralize_csv_formula(value);
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}

fn neutralize_csv_formula(value: &str) -> String {
    if value.starts_with(['=', '+', '-', '@']) {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

fn is_scalar(value: &Bson) -> bool {
    !matches!(value, Bson::Document(_) | Bson::Array(_))
}

fn format_scalar(value: &Bson) -> String {
    render_relaxed_extjson_string(value)
}

#[cfg(test)]
mod tests {
    use super::{
        ExportNameSource, format_bson_scalar, render_documents, suggested_export_filename,
        write_documents, write_rendered_output,
    };
    use lazycompass_core::OutputFormat;
    use lazycompass_mongo::{Bson, Document, parse_json_document};
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("lazycompass_output_{name}_{nonce}.txt"))
    }

    fn document(entries: Vec<(&str, Bson)>) -> Document {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    #[test]
    fn render_json_preserves_document_array_shape() {
        let documents = vec![document(vec![
            ("name", Bson::String("nora".to_string())),
            ("active", Bson::Boolean(true)),
        ])];
        let output = render_documents(OutputFormat::JsonPretty, &documents).expect("render json");

        assert!(output.contains("\"name\": \"nora\""));
        assert!(output.starts_with('['));
    }

    #[test]
    fn render_json_uses_relaxed_extjson_for_dates() {
        let documents = vec![
            parse_json_document(
                "document",
                r#"{ "createdAt": { "$date": "2026-03-10T12:00:00Z" } }"#,
            )
            .expect("parse document"),
        ];
        let output = render_documents(OutputFormat::JsonPretty, &documents).expect("render json");

        assert!(output.contains(r#""$date": "2026-03-10T12:00:00Z""#));
    }

    #[test]
    fn render_table_preserves_scalar_columns() {
        let documents = vec![document(vec![
            ("name", Bson::String("nora".to_string())),
            ("age", Bson::Int32(42)),
        ])];
        let output = render_documents(OutputFormat::Table, &documents).expect("render table");

        assert!(output.contains("name"));
        assert!(output.contains("nora"));
    }

    #[test]
    fn render_table_reports_missing_scalar_fields() {
        let nested = document(vec![("active", Bson::Boolean(true))]);
        let documents = vec![document(vec![("meta", Bson::Document(nested))])];
        let output = render_documents(OutputFormat::Table, &documents).expect("render table");

        assert_eq!(output, "no scalar fields to display");
    }

    #[test]
    fn render_csv_supports_nested_values_and_missing_fields() {
        let documents = vec![
            document(vec![
                ("name", Bson::String("nora".to_string())),
                (
                    "meta",
                    Bson::Document(document(vec![("active", Bson::Boolean(true))])),
                ),
                (
                    "tags",
                    Bson::Array(vec![
                        Bson::String("a".to_string()),
                        Bson::String("b".to_string()),
                    ]),
                ),
            ]),
            document(vec![("age", Bson::Int32(42))]),
        ];
        let output = render_documents(OutputFormat::Csv, &documents).expect("render csv");

        assert!(output.starts_with("age,meta,name,tags\n"));
        assert!(output.contains("\"{\"\"active\"\":true}\""));
        assert!(output.contains("\"[\"\"a\"\",\"\"b\"\"]\""));
        assert!(output.lines().any(|line| line == "42,,,"));
    }

    #[test]
    fn render_csv_escapes_special_characters() {
        let documents = vec![document(vec![(
            "text",
            Bson::String("hello,\"world\"\nnext".to_string()),
        )])];
        let output = render_documents(OutputFormat::Csv, &documents).expect("render csv");

        assert_eq!(output, "text\n\"hello,\"\"world\"\"\nnext\"");
    }

    #[test]
    fn render_csv_neutralizes_formula_cells() {
        let documents = vec![document(vec![(
            "text",
            Bson::String("=HYPERLINK(\"https://example.com\")".to_string()),
        )])];
        let output = render_documents(OutputFormat::Csv, &documents).expect("render csv");

        assert_eq!(output, "text\n\"'=HYPERLINK(\"\"https://example.com\"\")\"");
    }

    #[test]
    fn render_csv_is_empty_for_empty_results() {
        let documents: Vec<Document> = Vec::new();
        let output = render_documents(OutputFormat::Csv, &documents).expect("render csv");

        assert!(output.is_empty());
    }

    #[test]
    fn filename_suggestion_uses_source_and_extension() {
        assert_eq!(
            suggested_export_filename(
                &ExportNameSource::SavedQuery {
                    name: "recent_orders".to_string(),
                },
                OutputFormat::JsonPretty,
                false,
            ),
            "recent_orders.json"
        );
        assert_eq!(
            suggested_export_filename(
                &ExportNameSource::InlineAggregation,
                OutputFormat::Table,
                true
            ),
            "aggregation_results_document.txt"
        );
    }

    #[test]
    fn filename_suggestion_sanitizes_separators() {
        assert_eq!(
            suggested_export_filename(
                &ExportNameSource::SavedAggregation {
                    name: "app/orders".to_string(),
                },
                OutputFormat::Csv,
                false,
            ),
            "app_orders.csv"
        );
    }

    #[test]
    fn format_bson_scalar_matches_existing_string_behavior() {
        assert_eq!(format_bson_scalar(&Bson::String("abc".to_string())), "abc");
        assert_eq!(format_bson_scalar(&Bson::Int32(3)), "3");
    }

    #[test]
    fn format_bson_scalar_uses_relaxed_extjson_for_dates() {
        let value = parse_json_document(
            "document",
            r#"{ "createdAt": { "$date": "2026-03-10T12:00:00Z" } }"#,
        )
        .expect("parse document")
        .get("createdAt")
        .expect("createdAt")
        .clone();
        assert_eq!(
            format_bson_scalar(&value),
            r#"{"$date":"2026-03-10T12:00:00Z"}"#
        );
    }

    #[test]
    fn write_documents_writes_json_to_file() {
        let path = temp_path("json");
        let documents = vec![document(vec![
            ("name", Bson::String("nora".to_string())),
            ("active", Bson::Boolean(true)),
        ])];

        write_documents(OutputFormat::JsonPretty, &documents, &path).expect("write output");

        let contents = fs::read_to_string(&path).expect("read output");
        assert!(contents.contains("\"name\": \"nora\""));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn write_rendered_output_writes_exact_contents() {
        let path = temp_path("rendered");

        write_rendered_output(&path, "a,b\n1,2").expect("write rendered");

        let contents = fs::read_to_string(&path).expect("read output");
        assert_eq!(contents, "a,b\n1,2");

        let _ = fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn write_rendered_output_rejects_symlinked_targets() {
        let dir = std::env::temp_dir().join(format!(
            "lazycompass_output_symlink_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("create dir");
        let target = dir.join("target.txt");
        fs::write(&target, "before").expect("write target");
        let link = dir.join("export.txt");
        symlink(&target, &link).expect("create symlink");

        let err = write_rendered_output(&link, "after").expect_err("expected symlink rejection");
        assert!(err.to_string().contains("symlinked file"));
        assert_eq!(fs::read_to_string(&target).expect("read target"), "before");

        let _ = fs::remove_dir_all(dir);
    }
}
