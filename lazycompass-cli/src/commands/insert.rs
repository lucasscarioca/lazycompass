use anyhow::{Context, Result};
use lazycompass_mongo::{DocumentInsertSpec, MongoExecutor, parse_json_document};
use lazycompass_storage::{ConfigPaths, load_storage};

use super::database::resolve_database_arg;
use crate::cli::InsertArgs;
use crate::editor::read_document_input;
use crate::errors::report_warnings;
use crate::logging::{apply_cli_overrides, init_logging};
use crate::output::format_bson;

pub(crate) fn run_insert(
    args: InsertArgs,
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
    tracing::info!(component = "cli", command = "insert", "lazycompass started");
    report_warnings(&storage);

    let contents = read_document_input("insert", args.document.clone(), args.file.clone())?;
    let spec = build_insert_spec(&config, args, &contents)?;

    let executor = MongoExecutor::new();
    let resolved_connection = executor.resolve_connection(&config, spec.connection.as_deref())?;
    tracing::info!(
        component = "cli",
        command = "insert",
        connection = resolved_connection.name.as_str(),
        database = spec.database.as_str(),
        collection = spec.collection.as_str(),
        "inserting document"
    );
    let runtime = tokio::runtime::Runtime::new().context("unable to start async runtime")?;
    let inserted_id = runtime.block_on(executor.insert_document(&config, &spec))?;
    println!("inserted document {}", format_bson(&inserted_id));
    Ok(())
}

fn build_insert_spec(
    config: &lazycompass_core::Config,
    args: InsertArgs,
    contents: &str,
) -> Result<DocumentInsertSpec> {
    let connection = args.connection;
    let database = resolve_database_arg(
        config,
        connection.as_deref(),
        args.db,
        "--db is required for insert",
    )?;
    let collection = args
        .collection
        .ok_or_else(|| anyhow::anyhow!("--collection is required for insert"))?;
    let document = parse_json_document("document", contents)?;

    Ok(DocumentInsertSpec {
        connection,
        database,
        collection,
        document,
    })
}

#[cfg(test)]
mod tests {
    use lazycompass_core::{Config, ConnectionSpec};

    use super::build_insert_spec;
    use crate::cli::InsertArgs;

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

    fn base_args() -> InsertArgs {
        InsertArgs {
            connection: Some("local".to_string()),
            db: None,
            collection: Some("users".to_string()),
            document: None,
            file: None,
        }
    }

    #[test]
    fn build_insert_spec_uses_connection_default_database() {
        let spec = build_insert_spec(
            &config_with_default_db(),
            base_args(),
            r#"{"email":"a@example.com"}"#,
        )
        .expect("build insert spec");
        assert_eq!(spec.database, "app");
        assert_eq!(spec.collection, "users");
    }

    #[test]
    fn build_insert_spec_requires_collection() {
        let args = InsertArgs {
            collection: None,
            ..base_args()
        };
        let err = build_insert_spec(&config_with_default_db(), args, "{}")
            .expect_err("expected missing collection");
        assert!(
            err.to_string()
                .contains("--collection is required for insert")
        );
    }

    #[test]
    fn build_insert_spec_rejects_invalid_document_json() {
        let err = build_insert_spec(&config_with_default_db(), base_args(), "[]")
            .expect_err("expected invalid document");
        assert!(err.to_string().contains("document must be a JSON object"));
    }
}
