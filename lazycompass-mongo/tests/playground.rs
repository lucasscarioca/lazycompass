use anyhow::Result;
use lazycompass_core::{Config, ConnectionSpec, LoggingConfig, ThemeConfig, TimeoutConfig};
use lazycompass_mongo::{AggregationSpec, MongoExecutor, QuerySpec};

const PLAYGROUND_ENV: &str = "LAZYCOMPASS_PLAYGROUND";
const PLAYGROUND_URI_ENV: &str = "LAZYCOMPASS_MONGO_URI";

fn playground_enabled() -> bool {
    std::env::var(PLAYGROUND_ENV)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn playground_uri() -> String {
    std::env::var(PLAYGROUND_URI_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "mongodb://localhost:27017".to_string())
}

#[tokio::test]
async fn playground_query_and_aggregation() -> Result<()> {
    if !playground_enabled() {
        eprintln!(
            "skipping playground_query_and_aggregation: set {PLAYGROUND_ENV}=1 (optional {PLAYGROUND_URI_ENV}=mongodb://localhost:27017)"
        );
        return Ok(());
    }

    let config = Config {
        connections: vec![ConnectionSpec {
            name: "playground".to_string(),
            uri: playground_uri(),
            default_database: Some("lazycompass".to_string()),
        }],
        theme: ThemeConfig::default(),
        logging: LoggingConfig::default(),
        read_only: Some(true),
        allow_pipeline_writes: None,
        allow_insecure: None,
        timeouts: TimeoutConfig {
            connect_ms: Some(5_000),
            query_ms: Some(5_000),
        },
    };

    let executor = MongoExecutor::new();
    let databases = executor.list_databases(&config, Some("playground")).await?;
    assert!(databases.iter().any(|name| name == "lazycompass"));

    let query = QuerySpec {
        connection: Some("playground".to_string()),
        database: "lazycompass".to_string(),
        collection: "users".to_string(),
        filter: Some("{ \"active\": true }".to_string()),
        projection: None,
        sort: None,
        limit: Some(10),
    };
    let documents = executor.execute_query(&config, &query).await?;
    assert!(!documents.is_empty());

    let aggregation = AggregationSpec {
        connection: Some("playground".to_string()),
        database: "lazycompass".to_string(),
        collection: "orders".to_string(),
        pipeline: "[ { \"$group\": { \"_id\": \"$userId\", \"count\": { \"$sum\": 1 } } } ]"
            .to_string(),
    };
    let results = executor.execute_aggregation(&config, &aggregation).await?;
    assert!(!results.is_empty());

    Ok(())
}
