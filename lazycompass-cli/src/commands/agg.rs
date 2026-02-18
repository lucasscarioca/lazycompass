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
    use lazycompass_core::{AggregationTarget, Config, SavedAggregation, SavedScope};
    use lazycompass_storage::StorageSnapshot;

    use super::{build_agg_request, resolve_aggregation_spec};
    use crate::cli::AggArgs;

    fn base_args() -> AggArgs {
        AggArgs {
            name: None,
            connection: None,
            db: Some("lazycompass".to_string()),
            collection: Some("orders".to_string()),
            pipeline: Some("[]".to_string()),
            table: false,
        }
    }

    fn storage_with_aggregations(aggregations: Vec<SavedAggregation>) -> StorageSnapshot {
        StorageSnapshot {
            config: Config::default(),
            queries: Vec::new(),
            aggregations,
            warnings: Vec::new(),
        }
    }

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

    #[test]
    fn build_agg_request_requires_db_for_inline_aggregations() {
        let args = AggArgs {
            db: None,
            collection: Some("orders".to_string()),
            pipeline: Some("[]".to_string()),
            ..base_args()
        };
        let err = build_agg_request(args).expect_err("expected missing db");
        assert!(
            err.to_string()
                .contains("--db is required for inline aggregations")
        );
    }

    #[test]
    fn build_agg_request_requires_collection_for_inline_aggregations() {
        let args = AggArgs {
            db: Some("lazycompass".to_string()),
            collection: None,
            pipeline: Some("[]".to_string()),
            ..base_args()
        };
        let err = build_agg_request(args).expect_err("expected missing collection");
        assert!(
            err.to_string()
                .contains("--collection is required for inline aggregations")
        );
    }

    #[test]
    fn build_agg_request_requires_pipeline_for_inline_aggregations() {
        let args = AggArgs {
            db: Some("lazycompass".to_string()),
            collection: Some("orders".to_string()),
            pipeline: None,
            ..base_args()
        };
        let err = build_agg_request(args).expect_err("expected missing pipeline");
        assert!(
            err.to_string()
                .contains("--pipeline is required for inline aggregations")
        );
    }

    #[test]
    fn resolve_aggregation_spec_uses_saved_scope_when_scoped() {
        let request = build_agg_request(AggArgs {
            name: Some("saved.scoped".to_string()),
            db: Some("override_db".to_string()),
            collection: Some("override_collection".to_string()),
            pipeline: None,
            ..base_args()
        })
        .expect("request");
        assert!(matches!(request.target, AggregationTarget::Saved { .. }));
        let storage = storage_with_aggregations(vec![SavedAggregation {
            id: "saved.scoped".to_string(),
            scope: SavedScope::Scoped {
                database: "scoped_db".to_string(),
                collection: "scoped_collection".to_string(),
            },
            pipeline: r#"[{"$match":{"active":true}}]"#.to_string(),
        }]);

        let spec = resolve_aggregation_spec(&request, &storage).expect("resolve scoped saved");
        assert_eq!(spec.database, "scoped_db");
        assert_eq!(spec.collection, "scoped_collection");
        assert_eq!(spec.pipeline, r#"[{"$match":{"active":true}}]"#);
    }

    #[test]
    fn resolve_aggregation_spec_requires_overrides_for_shared_saved_aggregation() {
        let request = build_agg_request(AggArgs {
            name: Some("saved.shared".to_string()),
            db: None,
            collection: Some("orders".to_string()),
            pipeline: None,
            ..base_args()
        })
        .expect("request");
        let storage = storage_with_aggregations(vec![SavedAggregation {
            id: "saved.shared".to_string(),
            scope: SavedScope::Shared,
            pipeline: r#"[{"$match":{"active":true}}]"#.to_string(),
        }]);

        let err = resolve_aggregation_spec(&request, &storage).expect_err("expected missing --db");
        assert!(
            err.to_string()
                .contains("saved aggregation 'saved.shared' is shared")
        );
    }

    #[test]
    fn resolve_aggregation_spec_rejects_unknown_saved_aggregation() {
        let request = build_agg_request(AggArgs {
            name: Some("missing".to_string()),
            db: Some("lazycompass".to_string()),
            collection: Some("orders".to_string()),
            pipeline: None,
            ..base_args()
        })
        .expect("request");
        let storage = storage_with_aggregations(Vec::new());

        let err =
            resolve_aggregation_spec(&request, &storage).expect_err("expected missing saved agg");
        assert!(
            err.to_string()
                .contains("saved aggregation 'missing' not found")
        );
    }
}
