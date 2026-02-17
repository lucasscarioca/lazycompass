use anyhow::{Context, Result};
use lazycompass_core::OutputFormat;
use lazycompass_mongo::{Bson, Document};
use std::collections::BTreeSet;

pub(crate) fn print_documents(format: OutputFormat, documents: &[Document]) -> Result<()> {
    match format {
        OutputFormat::JsonPretty => {
            let output = serde_json::to_string_pretty(documents)
                .context("unable to serialize results as JSON")?;
            println!("{output}");
        }
        OutputFormat::Table => {
            let output = format_table(documents);
            println!("{output}");
        }
    }
    Ok(())
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
