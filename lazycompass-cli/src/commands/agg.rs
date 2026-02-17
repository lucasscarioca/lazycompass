use anyhow::{Context, Result};
use lazycompass_core::{AggregationRequest, AggregationTarget, OutputFormat};
use lazycompass_mongo::{AggregationSpec, MongoExecutor};
use lazycompass_storage::{ConfigPaths, StorageSnapshot, load_storage};

use crate::cli::AggArgs;
use crate::errors::report_warnings;
use crate::logging::{apply_cli_overrides, init_logging};
use crate::output::print_documents;

pub(crate) fn run_agg(
    args: AggArgs,
    write_enabled: bool,
    allow_pipeline_writes: bool,
    allow_insecure: bool,
) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let request = build_agg_request(args)?;
    let storage = load_storage(&paths)?;
    let mut config = storage.config.clone();
    apply_cli_overrides(
        &mut config,
        write_enabled,
        allow_pipeline_writes,
        allow_insecure,
    );
    init_logging(&paths, &config)?;
    tracing::info!(component = "cli", command = "agg", "lazycompass started");
    report_warnings(&storage);
    let spec = resolve_aggregation_spec(&request, &storage)?;
    let executor = MongoExecutor::new();
    let connection = executor.resolve_connection(&config, spec.connection.as_deref())?;
    tracing::info!(
        component = "cli",
        command = "agg",
        connection = connection.name.as_str(),
        database = spec.database.as_str(),
        collection = spec.collection.as_str(),
        "executing aggregation"
    );
    let runtime = tokio::runtime::Runtime::new().context("unable to start async runtime")?;
    let documents = runtime.block_on(executor.execute_aggregation(&config, &spec))?;
    print_documents(request.output, &documents)
}

fn build_agg_request(args: AggArgs) -> Result<AggregationRequest> {
    let output = if args.table {
        OutputFormat::Table
    } else {
        OutputFormat::JsonPretty
    };

    if let Some(id) = &args.name {
        let mut conflicts = Vec::new();
        if args.pipeline.is_some() {
            conflicts.push("--pipeline");
        }
        if !conflicts.is_empty() {
            anyhow::bail!(
                "saved aggregation '{}' cannot be combined with {}",
                id,
                conflicts.join(", ")
            );
        }
    }

    let target = if let Some(id) = args.name {
        AggregationTarget::Saved {
            id,
            database: args.db,
            collection: args.collection,
        }
    } else {
        let database = args
            .db
            .ok_or_else(|| anyhow::anyhow!("--db is required for inline aggregations"))?;
        let collection = args
            .collection
            .ok_or_else(|| anyhow::anyhow!("--collection is required for inline aggregations"))?;
        let pipeline = args
            .pipeline
            .ok_or_else(|| anyhow::anyhow!("--pipeline is required for inline aggregations"))?;

        AggregationTarget::Inline {
            database,
            collection,
            pipeline,
        }
    };

    Ok(AggregationRequest {
        connection: args.connection,
        output,
        target,
    })
}

fn resolve_aggregation_spec(
    request: &AggregationRequest,
    storage: &StorageSnapshot,
) -> Result<AggregationSpec> {
    match &request.target {
        AggregationTarget::Saved {
            id,
            database,
            collection,
        } => {
            let saved = storage
                .aggregations
                .iter()
                .find(|aggregation| aggregation.id == *id)
                .with_context(|| format!("saved aggregation '{id}' not found"))?;
            let (resolved_db, resolved_collection) =
                if let Some((database, collection)) = saved.scope.database_collection() {
                    (database.to_string(), collection.to_string())
                } else {
                    let database = database.clone().ok_or_else(|| {
                        anyhow::anyhow!("saved aggregation '{id}' is shared; pass --db")
                    })?;
                    let collection = collection.clone().ok_or_else(|| {
                        anyhow::anyhow!("saved aggregation '{id}' is shared; pass --collection")
                    })?;
                    (database, collection)
                };
            Ok(AggregationSpec {
                connection: request.connection.clone(),
                database: resolved_db,
                collection: resolved_collection,
                pipeline: saved.pipeline.clone(),
            })
        }
        AggregationTarget::Inline {
            database,
            collection,
            pipeline,
        } => Ok(AggregationSpec {
            connection: request.connection.clone(),
            database: database.clone(),
            collection: collection.clone(),
            pipeline: pipeline.clone(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::build_agg_request;
    use crate::cli::AggArgs;

    #[test]
    fn build_agg_request_rejects_pipeline_with_saved_name() {
        let args = AggArgs {
            name: Some("saved".to_string()),
            connection: None,
            db: None,
            collection: Some("orders".to_string()),
            pipeline: Some("[]".to_string()),
            table: false,
        };

        assert!(build_agg_request(args).is_err());
    }
}
