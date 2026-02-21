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

    let connection = args.connection;
    let database = resolve_database_arg(
        &config,
        connection.as_deref(),
        args.db,
        "--db is required for insert",
    )?;
    let collection = args
        .collection
        .ok_or_else(|| anyhow::anyhow!("--collection is required for insert"))?;

    let contents = read_document_input("insert", args.document, args.file)?;
    let document = parse_json_document("document", &contents)?;
    let spec = DocumentInsertSpec {
        connection,
        database,
        collection,
        document,
    };

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
