use anyhow::{Context, Result};
use lazycompass_mongo::{Bson, Document, DocumentReplaceSpec, MongoExecutor, parse_json_document};
use lazycompass_storage::{ConfigPaths, load_storage};

use super::database::resolve_database_arg;
use crate::cli::UpdateArgs;
use crate::editor::{parse_json_value, read_document_input};
use crate::errors::report_warnings;
use crate::logging::{apply_cli_overrides, init_logging};
use crate::output::format_bson;

pub(crate) fn run_update(
    args: UpdateArgs,
    write_enabled: bool,
    allow_pipeline_writes: bool,
    allow_insecure: bool,
) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let storage = load_storage(&paths)?;
    let mut config = storage.config.clone();
    apply_cli_overrides(
        &mut config,
        write_enabled,
        allow_pipeline_writes,
        allow_insecure,
    );
    init_logging(&paths, &config)?;
    tracing::info!(component = "cli", command = "update", "lazycompass started");
    report_warnings(&storage);

    let contents = read_document_input("update", args.document.clone(), args.file.clone())?;
    let (spec, id_changed) = build_update_spec(&config, args, &contents)?;

    let executor = MongoExecutor::new();
    let resolved_connection = executor.resolve_connection(&config, spec.connection.as_deref())?;
    tracing::info!(
        component = "cli",
        command = "update",
        connection = resolved_connection.name.as_str(),
        database = spec.database.as_str(),
        collection = spec.collection.as_str(),
        "replacing document"
    );
    let runtime = tokio::runtime::Runtime::new().context("unable to start async runtime")?;
    runtime.block_on(executor.replace_document(&config, &spec))?;
    if id_changed {
        println!("updated document {} (kept --id)", format_bson(&spec.id));
    } else {
        println!("updated document {}", format_bson(&spec.id));
    }
    Ok(())
}

fn build_update_spec(
    config: &lazycompass_core::Config,
    args: UpdateArgs,
    contents: &str,
) -> Result<(DocumentReplaceSpec, bool)> {
    let connection = args.connection;
    let database = resolve_database_arg(
        config,
        connection.as_deref(),
        args.db,
        "--db is required for update",
    )?;
    let collection = args
        .collection
        .ok_or_else(|| anyhow::anyhow!("--collection is required for update"))?;

    let id = parse_json_value("id", &args.id)?;
    let mut document = parse_json_document("document", contents)?;
    let id_changed = ensure_document_id(&mut document, &id);

    Ok((
        DocumentReplaceSpec {
            connection,
            database,
            collection,
            id,
            document,
        },
        id_changed,
    ))
}

fn ensure_document_id(document: &mut Document, id: &Bson) -> bool {
    match document.get("_id") {
        Some(existing) if existing == id => false,
        _ => {
            document.insert("_id", id.clone());
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use lazycompass_core::{Config, ConnectionSpec};
    use lazycompass_mongo::{Bson, Document};

    use super::{build_update_spec, ensure_document_id};
    use crate::cli::UpdateArgs;

    fn config_with_default_db() -> Config {
        Config {
            connections: vec![ConnectionSpec {
                name: "local".to_string(),
                uri: "mongodb://localhost:27017".to_string(),
                default_database: Some("app".to_string()),
            }],
            ..Config::default()
        }
    }

    fn base_args() -> UpdateArgs {
        UpdateArgs {
            connection: Some("local".to_string()),
            db: None,
            collection: Some("users".to_string()),
            id: r#""user-1""#.to_string(),
            document: None,
            file: None,
        }
    }

    #[test]
    fn ensure_document_id_keeps_existing_matching_id() {
        let id = Bson::String("user-1".to_string());
        let mut document = Document::from_iter([
            ("_id".to_string(), id.clone()),
            (
                "email".to_string(),
                Bson::String("a@example.com".to_string()),
            ),
        ]);
        assert!(!ensure_document_id(&mut document, &id));
        assert_eq!(document.get("_id"), Some(&id));
    }

    #[test]
    fn ensure_document_id_replaces_mismatched_id() {
        let expected = Bson::String("user-1".to_string());
        let wrong = Bson::String("wrong-id".to_string());
        let mut document = Document::from_iter([
            ("_id".to_string(), wrong),
            (
                "email".to_string(),
                Bson::String("a@example.com".to_string()),
            ),
        ]);
        assert!(ensure_document_id(&mut document, &expected));
        assert_eq!(document.get("_id"), Some(&expected));
    }

    #[test]
    fn build_update_spec_uses_connection_default_database() {
        let (spec, id_changed) = build_update_spec(
            &config_with_default_db(),
            base_args(),
            r#"{"email":"a@example.com"}"#,
        )
        .expect("build update spec");
        assert_eq!(spec.database, "app");
        assert_eq!(spec.collection, "users");
        assert!(id_changed);
    }

    #[test]
    fn build_update_spec_keeps_matching_id_without_flag() {
        let args = base_args();
        let (spec, id_changed) = build_update_spec(
            &config_with_default_db(),
            args,
            r#"{"_id":"user-1","email":"a@example.com"}"#,
        )
        .expect("build update spec");
        assert!(!id_changed);
        assert_eq!(
            spec.document.get("_id"),
            Some(&Bson::String("user-1".to_string()))
        );
    }
}
