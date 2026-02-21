use anyhow::{Context, Result};
use lazycompass_mongo::{DocumentReplaceSpec, MongoExecutor, parse_json_document};
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

    let connection = args.connection;
    let database = resolve_database_arg(
        &config,
        connection.as_deref(),
        args.db,
        "--db is required for update",
    )?;
    let collection = args
        .collection
        .ok_or_else(|| anyhow::anyhow!("--collection is required for update"))?;

    let id = parse_json_value("id", &args.id)?;
    let contents = read_document_input("update", args.document, args.file)?;
    let mut document = parse_json_document("document", &contents)?;
    let mut id_changed = false;
    match document.get("_id") {
        Some(existing) if existing == &id => {}
        _ => {
            document.insert("_id", id.clone());
            id_changed = true;
        }
    }

    let spec = DocumentReplaceSpec {
        connection,
        database,
        collection,
        id: id.clone(),
        document,
    };

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
        println!("updated document {} (kept --id)", format_bson(&id));
    } else {
        println!("updated document {}", format_bson(&id));
    }
    Ok(())
}
