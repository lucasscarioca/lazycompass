use anyhow::Result;
use lazycompass_core::{Config, ConnectionSpec, LoggingConfig, ThemeConfig, TimeoutConfig};
use lazycompass_mongo::{
    AggregationSpec, Bson, DocumentDeleteSpec, DocumentInsertSpec, DocumentReplaceSpec,
    MongoExecutor, QuerySpec,
};
use mongodb::bson::doc;
use std::time::{SystemTime, UNIX_EPOCH};

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
        read_only: Some(false),
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

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let collection = format!("write_test_{nonce}");
    let marker = format!("playground_{nonce}");

    let insert_spec = DocumentInsertSpec {
        connection: Some("playground".to_string()),
        database: "lazycompass".to_string(),
        collection: collection.clone(),
        document: doc! { "marker": marker.clone(), "step": "insert" },
    };
    let inserted_id = executor.insert_document(&config, &insert_spec).await?;

    let inserted_rows = executor
        .execute_query(
            &config,
            &QuerySpec {
                connection: Some("playground".to_string()),
                database: "lazycompass".to_string(),
                collection: collection.clone(),
                filter: Some(format!(r#"{{ "marker": "{}" }}"#, marker)),
                projection: None,
                sort: None,
                limit: None,
            },
        )
        .await?;
    assert_eq!(inserted_rows.len(), 1);
    assert_eq!(
        inserted_rows[0].get_str("step").expect("step string"),
        "insert"
    );

    let mut replacement = doc! { "marker": marker.clone(), "step": "replace" };
    replacement.insert("_id", inserted_id.clone());
    let replace_spec = DocumentReplaceSpec {
        connection: Some("playground".to_string()),
        database: "lazycompass".to_string(),
        collection: collection.clone(),
        id: inserted_id.clone(),
        document: replacement,
    };
    executor.replace_document(&config, &replace_spec).await?;

    let replaced_rows = executor
        .execute_query(
            &config,
            &QuerySpec {
                connection: Some("playground".to_string()),
                database: "lazycompass".to_string(),
                collection: collection.clone(),
                filter: Some(format!(r#"{{ "_id": {} }}"#, bson_id_json(&inserted_id))),
                projection: None,
                sort: None,
                limit: None,
            },
        )
        .await?;
    assert_eq!(replaced_rows.len(), 1);
    assert_eq!(
        replaced_rows[0].get_str("step").expect("step string"),
        "replace"
    );

    let delete_spec = DocumentDeleteSpec {
        connection: Some("playground".to_string()),
        database: "lazycompass".to_string(),
        collection: collection.clone(),
        id: inserted_id.clone(),
    };
    executor.delete_document(&config, &delete_spec).await?;

    let deleted_rows = executor
        .execute_query(
            &config,
            &QuerySpec {
                connection: Some("playground".to_string()),
                database: "lazycompass".to_string(),
                collection,
                filter: Some(format!(r#"{{ "_id": {} }}"#, bson_id_json(&inserted_id))),
                projection: None,
                sort: None,
                limit: None,
            },
        )
        .await?;
    assert!(deleted_rows.is_empty());

    Ok(())
}

fn bson_id_json(id: &Bson) -> String {
    match id {
        Bson::ObjectId(oid) => format!(r#"{{ "$oid": "{}" }}"#, oid),
        value => serde_json::to_string(value).expect("serialize non-oid id"),
    }
}
