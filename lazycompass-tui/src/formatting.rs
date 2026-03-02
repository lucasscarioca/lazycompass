use super::*;

const MAX_PREVIEW_SERIALIZE_BYTES: usize = 4 * 1024;
const MAX_DETAIL_SERIALIZE_BYTES: usize = 256 * 1024;
const MAX_SUMMARY_FIELDS: usize = 4;

pub(crate) fn document_id(document: &Document) -> Result<Bson> {
    document
        .get("_id")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("document is missing _id"))
}

pub(crate) fn format_bson(value: &Bson) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(value)) => value,
        Ok(serde_json::Value::Null) => "null".to_string(),
        Ok(value) => value.to_string(),
        Err(_) => format!("{value:?}"),
    }
}

pub(crate) fn connection_label(connection: &ConnectionSpec) -> String {
    match connection.default_database.as_deref() {
        Some(default_db) => format!("{} ({default_db})", connection.name),
        None => connection.name.clone(),
    }
}

pub(crate) fn document_preview(document: &Document) -> String {
    if let Some(size) = encoded_bson_size(document)
        && size > MAX_PREVIEW_SERIALIZE_BYTES
    {
        return large_document_summary(document, size);
    }

    let mut json = serde_json::to_string(document).unwrap_or_else(|_| format!("{document:?}"));
    json = json.replace('\n', " ");
    if json.len() > 120 {
        json.truncate(117);
        json.push_str("...");
    }
    json
}

pub(crate) fn format_document(document: &Document) -> Vec<String> {
    if let Some(size) = encoded_bson_size(document)
        && size > MAX_DETAIL_SERIALIZE_BYTES
    {
        return vec![
            format!(
                "document is too large to render safely ({} bytes BSON)",
                size
            ),
            "Use export/copy to inspect the full contents.".to_string(),
            format!("summary: {}", summary_fields(document)),
        ];
    }

    match serde_json::to_string_pretty(document) {
        Ok(output) => output.lines().map(|line| line.to_string()).collect(),
        Err(_) => vec![format!("{document:?}")],
    }
}

fn encoded_bson_size(document: &Document) -> Option<usize> {
    let mut buffer = Vec::new();
    document.to_writer(&mut buffer).ok().map(|_| buffer.len())
}

fn large_document_summary(document: &Document, size: usize) -> String {
    let summary = summary_fields(document);
    let mut output = format!("{{large document: {size} bytes BSON, {summary}}}");
    if output.len() > 120 {
        output.truncate(117);
        output.push_str("...");
    }
    output
}

fn summary_fields(document: &Document) -> String {
    let mut parts = Vec::new();
    if let Some(id) = document.get("_id") {
        parts.push(format!("_id={}", format_bson(id)));
    }
    for (index, (key, value)) in document.iter().enumerate() {
        if key == "_id" {
            continue;
        }
        if index >= MAX_SUMMARY_FIELDS {
            break;
        }
        let value = match value {
            Bson::Document(_) => "{...}".to_string(),
            Bson::Array(values) => format!("[{} items]", values.len()),
            _ => format_bson(value),
        };
        parts.push(format!("{key}={value}"));
    }
    if parts.is_empty() {
        format!("{} top-level keys", document.len())
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::{document_preview, format_document};
    use lazycompass_mongo::{Bson, Document};

    #[test]
    fn document_preview_summarizes_large_documents() {
        let mut nested = Document::new();
        nested.insert("blob", "x".repeat(10_000));
        let mut document = Document::new();
        document.insert("_id", "doc-1");
        document.insert("nested", nested);

        let preview = document_preview(&document);
        assert!(preview.contains("large document"));
        assert!(preview.contains("_id=doc-1"));
    }

    #[test]
    fn format_document_short_circuits_large_documents() {
        let mut document = Document::new();
        document.insert("_id", "doc-1");
        document.insert("blob", Bson::String("x".repeat(300_000)));

        let lines = format_document(&document);
        assert!(lines[0].contains("too large to render safely"));
        assert!(lines[1].contains("export/copy"));
    }
}
