use anyhow::{Context, Result};
use lazycompass_core::OutputFormat;
use lazycompass_mongo::{Bson, Document};
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
        OutputFormat::JsonPretty => {
            serde_json::to_string_pretty(documents).context("unable to serialize results as JSON")
        }
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
    fs::write(output_path, output)
        .with_context(|| format!("unable to write output file {}", output_path.display()))
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
        Bson::Document(document) => {
            serde_json::to_string(document).unwrap_or_else(|_| format!("{document:?}"))
        }
        Bson::Array(values) => {
            serde_json::to_string(values).unwrap_or_else(|_| format!("{values:?}"))
        }
        _ => format_scalar(value),
    }
}

fn escape_csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn is_scalar(value: &Bson) -> bool {
    !matches!(value, Bson::Document(_) | Bson::Array(_))
}

fn format_scalar(value: &Bson) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(value)) => value,
        Ok(serde_json::Value::Null) => "null".to_string(),
        Ok(value) => value.to_string(),
        Err(_) => format!("{value:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ExportNameSource, format_bson_scalar, render_documents, suggested_export_filename,
        write_documents, write_rendered_output,
    };
    use lazycompass_core::OutputFormat;
    use lazycompass_mongo::{Bson, Document};
    use std::fs;

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
}
