use anyhow::Result;
use lazycompass_core::OutputFormat;
use lazycompass_mongo::Document;
use lazycompass_output::{format_bson_scalar, write_documents};
use std::path::Path;

pub(crate) fn print_documents(
    format: OutputFormat,
    documents: &[Document],
    output_path: Option<&Path>,
) -> Result<()> {
    if let Some(path) = output_path {
        write_documents(format, documents, path)?;
    } else {
        let output = lazycompass_output::render_documents(format, documents)?;
        println!("{output}");
    }
    Ok(())
}

pub(crate) fn format_bson(value: &lazycompass_mongo::Bson) -> String {
    format_bson_scalar(value)
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
