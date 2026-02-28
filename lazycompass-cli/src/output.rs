use anyhow::{Context, Result};
use lazycompass_core::OutputFormat;
use lazycompass_mongo::{Bson, Document};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(crate) fn print_documents(
    format: OutputFormat,
    documents: &[Document],
    output_path: Option<&Path>,
) -> Result<()> {
    let output = render_documents(format, documents)?;
    if let Some(path) = output_path {
        fs::write(path, output)
            .with_context(|| format!("unable to write output file {}", path.display()))?;
    } else {
        println!("{output}");
    }
    Ok(())
}

fn render_documents(format: OutputFormat, documents: &[Document]) -> Result<String> {
    match format {
        OutputFormat::JsonPretty => {
            serde_json::to_string_pretty(documents).context("unable to serialize results as JSON")
        }
        OutputFormat::Table => Ok(format_table(documents)),
    }
}

pub(crate) fn format_bson(value: &Bson) -> String {
    format_scalar(value)
}

fn format_table(documents: &[Document]) -> String {
    if documents.is_empty() {
        return "no results".to_string();
    }

    let mut columns = BTreeSet::new();
    for document in documents {
        for (key, value) in document.iter() {
            if is_scalar(value) {
                columns.insert(key.to_string());
            }
        }
    }

    if columns.is_empty() {
        return "no scalar fields to display".to_string();
    }

    let columns: Vec<String> = columns.into_iter().collect();
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
    use super::print_documents;
    use lazycompass_core::OutputFormat;
    use lazycompass_mongo::Document;
    use std::fs;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("lazycompass_output_{name}_{nonce}.txt"))
    }

    #[test]
    fn print_documents_writes_json_to_file() {
        let path = temp_path("json");
        let mut document = Document::new();
        document.insert("name", "nora");
        document.insert("active", true);
        let documents = vec![document];

        print_documents(OutputFormat::JsonPretty, &documents, Some(&path)).expect("write output");

        let contents = fs::read_to_string(&path).expect("read output");
        assert!(contents.contains("\"name\": \"nora\""));
        assert!(contents.contains("\"active\": true"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn print_documents_writes_table_to_file() {
        let path = temp_path("table");
        let mut document = Document::new();
        document.insert("name", "nora");
        document.insert("age", 42);
        let documents = vec![document];

        print_documents(OutputFormat::Table, &documents, Some(&path)).expect("write output");

        let contents = fs::read_to_string(&path).expect("read output");
        assert!(contents.contains("name"));
        assert!(contents.contains("nora"));

        let _ = fs::remove_file(path);
    }
}
