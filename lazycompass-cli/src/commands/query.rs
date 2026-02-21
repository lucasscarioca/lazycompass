use anyhow::{Context, Result};
use lazycompass_core::{OutputFormat, QueryRequest, QueryTarget};
use lazycompass_mongo::{MongoExecutor, QuerySpec};
use lazycompass_storage::{ConfigPaths, StorageSnapshot, load_storage};

use super::database::resolve_database_arg;
use crate::cli::QueryArgs;
use crate::errors::report_warnings;
use crate::logging::{apply_cli_overrides, init_logging};
use crate::output::print_documents;

pub(crate) fn run_query(
    args: QueryArgs,
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
    tracing::info!(component = "cli", command = "query", "lazycompass started");
    report_warnings(&storage);
    let mut args = args;
    if args.name.is_none() {
        args.db = Some(resolve_database_arg(
            &config,
            args.connection.as_deref(),
            args.db,
            "--db is required for inline queries",
        )?);
    }
    let request = build_query_request(args)?;
    let spec = resolve_query_spec(&request, &storage)?;
    let executor = MongoExecutor::new();
    let connection = executor.resolve_connection(&config, spec.connection.as_deref())?;
    tracing::info!(
        component = "cli",
        command = "query",
        connection = connection.name.as_str(),
        database = spec.database.as_str(),
        collection = spec.collection.as_str(),
        "executing query"
    );
    let runtime = tokio::runtime::Runtime::new().context("unable to start async runtime")?;
    let documents = runtime.block_on(executor.execute_query(&config, &spec))?;
    print_documents(request.output, &documents)
}

fn build_query_request(args: QueryArgs) -> Result<QueryRequest> {
    let output = if args.table {
        OutputFormat::Table
    } else {
        OutputFormat::JsonPretty
    };

    if let Some(id) = &args.name {
        let mut conflicts = Vec::new();
        if args.filter.is_some() {
            conflicts.push("--filter");
        }
        if args.projection.is_some() {
            conflicts.push("--projection");
        }
        if args.sort.is_some() {
            conflicts.push("--sort");
        }
        if args.limit.is_some() {
            conflicts.push("--limit");
        }
        if !conflicts.is_empty() {
            anyhow::bail!(
                "saved query '{}' cannot be combined with {}",
                id,
                conflicts.join(", ")
            );
        }
    }

    let target = if let Some(id) = args.name {
        QueryTarget::Saved {
            id,
            database: args.db,
            collection: args.collection,
        }
    } else {
        let database = args
            .db
            .ok_or_else(|| anyhow::anyhow!("--db is required for inline queries"))?;
        let collection = args
            .collection
            .ok_or_else(|| anyhow::anyhow!("--collection is required for inline queries"))?;

        QueryTarget::Inline {
            database,
            collection,
            filter: args.filter,
            projection: args.projection,
            sort: args.sort,
            limit: args.limit,
        }
    };

    Ok(QueryRequest {
        connection: args.connection,
        output,
        target,
    })
}

fn resolve_query_spec(request: &QueryRequest, storage: &StorageSnapshot) -> Result<QuerySpec> {
    match &request.target {
        QueryTarget::Saved {
            id,
            database,
            collection,
        } => {
            let saved = storage
                .queries
                .iter()
                .find(|query| query.id == *id)
                .with_context(|| format!("saved query '{id}' not found"))?;
            let (resolved_db, resolved_collection) =
                if let Some((database, collection)) = saved.scope.database_collection() {
                    (database.to_string(), collection.to_string())
                } else {
                    let database = resolve_database_arg(
                        &storage.config,
                        request.connection.as_deref(),
                        database.clone(),
                        format!("saved query '{id}' is shared; pass --db"),
                    )?;
                    let collection = collection.clone().ok_or_else(|| {
                        anyhow::anyhow!("saved query '{id}' is shared; pass --collection")
                    })?;
                    (database, collection)
                };
            Ok(QuerySpec {
                connection: request.connection.clone(),
                database: resolved_db,
                collection: resolved_collection,
                filter: saved.filter.clone(),
                projection: saved.projection.clone(),
                sort: saved.sort.clone(),
                limit: saved.limit,
            })
        }
        QueryTarget::Inline {
            database,
            collection,
            filter,
            projection,
            sort,
            limit,
        } => Ok(QuerySpec {
            connection: request.connection.clone(),
            database: database.clone(),
            collection: collection.clone(),
            filter: filter.clone(),
            projection: projection.clone(),
            sort: sort.clone(),
            limit: *limit,
        }),
    }
}

#[cfg(test)]
mod tests {
    use lazycompass_core::{Config, ConnectionSpec, QueryTarget, SavedQuery, SavedScope};
    use lazycompass_storage::StorageSnapshot;

    use super::{build_query_request, resolve_query_spec};
    use crate::cli::QueryArgs;

    fn base_args() -> QueryArgs {
        QueryArgs {
            name: None,
            connection: None,
            db: Some("lazycompass".to_string()),
            collection: Some("users".to_string()),
            filter: None,
            projection: None,
            sort: None,
            limit: None,
            table: false,
        }
    }

    fn storage_with_queries(config: Config, queries: Vec<SavedQuery>) -> StorageSnapshot {
        StorageSnapshot {
            config,
            queries,
            aggregations: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn config_with_connection(name: &str, default_database: Option<&str>) -> Config {
        Config {
            connections: vec![ConnectionSpec {
                name: name.to_string(),
                uri: format!("mongodb://{name}:27017"),
                default_database: default_database.map(ToString::to_string),
            }],
            ..Config::default()
        }
    }

    #[test]
    fn build_query_request_allows_target_context_with_saved_name() {
        let args = QueryArgs {
            name: Some("saved".to_string()),
            connection: None,
            db: Some("lazycompass".to_string()),
            collection: Some("users".to_string()),
            filter: None,
            projection: None,
            sort: None,
            limit: None,
            table: false,
        };

        let request = build_query_request(args).expect("request");
        assert!(matches!(
            request.target,
            QueryTarget::Saved {
                id,
                database,
                collection
            } if id == "saved" && database.as_deref() == Some("lazycompass")
                && collection.as_deref() == Some("users")
        ));
    }

    #[test]
    fn build_query_request_rejects_inline_conflicts_with_saved_name() {
        let args = QueryArgs {
            name: Some("saved".to_string()),
            filter: Some(r#"{"active":true}"#.to_string()),
            projection: Some(r#"{"email":1}"#.to_string()),
            sort: Some(r#"{"createdAt":-1}"#.to_string()),
            limit: Some(10),
            ..base_args()
        };
        let err = build_query_request(args).expect_err("expected conflict");
        assert!(err.to_string().contains("cannot be combined"));
    }

    #[test]
    fn build_query_request_requires_db_for_inline_queries() {
        let args = QueryArgs {
            db: None,
            collection: Some("users".to_string()),
            ..base_args()
        };
        let err = build_query_request(args).expect_err("expected missing db");
        assert!(
            err.to_string()
                .contains("--db is required for inline queries")
        );
    }

    #[test]
    fn build_query_request_requires_collection_for_inline_queries() {
        let args = QueryArgs {
            db: Some("lazycompass".to_string()),
            collection: None,
            ..base_args()
        };
        let err = build_query_request(args).expect_err("expected missing collection");
        assert!(
            err.to_string()
                .contains("--collection is required for inline queries")
        );
    }

    #[test]
    fn resolve_query_spec_uses_saved_scope_when_scoped() {
        let request = build_query_request(QueryArgs {
            name: Some("saved.scoped".to_string()),
            db: Some("override_db".to_string()),
            collection: Some("override_collection".to_string()),
            ..base_args()
        })
        .expect("request");
        let storage = storage_with_queries(
            Config::default(),
            vec![SavedQuery {
                id: "saved.scoped".to_string(),
                scope: SavedScope::Scoped {
                    database: "scoped_db".to_string(),
                    collection: "scoped_collection".to_string(),
                },
                filter: Some(r#"{"active":true}"#.to_string()),
                projection: None,
                sort: None,
                limit: Some(5),
            }],
        );

        let spec = resolve_query_spec(&request, &storage).expect("resolve saved scoped");
        assert_eq!(spec.database, "scoped_db");
        assert_eq!(spec.collection, "scoped_collection");
        assert_eq!(spec.limit, Some(5));
    }

    #[test]
    fn resolve_query_spec_requires_overrides_for_shared_saved_query() {
        let request = build_query_request(QueryArgs {
            name: Some("saved.shared".to_string()),
            db: None,
            collection: Some("users".to_string()),
            ..base_args()
        })
        .expect("request");
        let storage = storage_with_queries(
            Config::default(),
            vec![SavedQuery {
                id: "saved.shared".to_string(),
                scope: SavedScope::Shared,
                filter: Some(r#"{"active":true}"#.to_string()),
                projection: None,
                sort: None,
                limit: None,
            }],
        );

        let err = resolve_query_spec(&request, &storage).expect_err("expected missing --db");
        assert!(
            err.to_string()
                .contains("saved query 'saved.shared' is shared")
        );
    }

    #[test]
    fn resolve_query_spec_uses_default_database_for_shared_saved_query() {
        let request = build_query_request(QueryArgs {
            name: Some("saved.shared".to_string()),
            connection: Some("local".to_string()),
            db: None,
            collection: Some("users".to_string()),
            ..base_args()
        })
        .expect("request");
        let storage = storage_with_queries(
            config_with_connection("local", Some("app")),
            vec![SavedQuery {
                id: "saved.shared".to_string(),
                scope: SavedScope::Shared,
                filter: Some(r#"{"active":true}"#.to_string()),
                projection: None,
                sort: None,
                limit: None,
            }],
        );

        let spec = resolve_query_spec(&request, &storage).expect("resolve saved shared");
        assert_eq!(spec.database, "app");
        assert_eq!(spec.collection, "users");
    }

    #[test]
    fn resolve_query_spec_rejects_unknown_saved_query() {
        let request = build_query_request(QueryArgs {
            name: Some("missing".to_string()),
            ..base_args()
        })
        .expect("request");
        let storage = storage_with_queries(Config::default(), Vec::new());

        let err = resolve_query_spec(&request, &storage).expect_err("expected missing saved query");
        assert!(err.to_string().contains("saved query 'missing' not found"));
    }
}
