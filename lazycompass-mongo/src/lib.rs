pub use mongodb::bson::{Bson, Document};

use anyhow::{Context, Result};
use futures::TryStreamExt;
use lazycompass_core::{Config, ConnectionSpec, WriteGuard, redact_connection_uri};
use mongodb::{
    Client, bson,
    options::{AggregateOptions, ClientOptions, FindOptions},
};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct QuerySpec {
    pub connection: Option<String>,
    pub database: String,
    pub collection: String,
    pub filter: Option<String>,
    pub projection: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct AggregationSpec {
    pub connection: Option<String>,
    pub database: String,
    pub collection: String,
    pub pipeline: String,
}

#[derive(Debug, Clone)]
pub struct DocumentListSpec {
    pub connection: Option<String>,
    pub database: String,
    pub collection: String,
    pub skip: u64,
    pub limit: u64,
}

#[derive(Debug, Clone)]
pub struct DocumentInsertSpec {
    pub connection: Option<String>,
    pub database: String,
    pub collection: String,
    pub document: Document,
}

#[derive(Debug, Clone)]
pub struct DocumentReplaceSpec {
    pub connection: Option<String>,
    pub database: String,
    pub collection: String,
    pub id: Bson,
    pub document: Document,
}

#[derive(Debug, Clone)]
pub struct DocumentDeleteSpec {
    pub connection: Option<String>,
    pub database: String,
    pub collection: String,
    pub id: Bson,
}

#[derive(Debug, Default)]
pub struct MongoExecutor;

impl MongoExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn resolve_connection<'a>(
        &self,
        config: &'a Config,
        name: Option<&str>,
    ) -> Result<&'a ConnectionSpec> {
        if config.connections.is_empty() {
            anyhow::bail!("no connections configured");
        }

        let name = name.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });

        if let Some(name) = name {
            return config
                .connections
                .iter()
                .find(|connection| connection.name == name)
                .with_context(|| format!("connection '{name}' not found"));
        }

        if config.connections.len() == 1 {
            return Ok(&config.connections[0]);
        }

        anyhow::bail!(
            "multiple connections configured; specify --connection or set connection in the saved spec"
        )
    }

    pub async fn execute_query(&self, config: &Config, spec: &QuerySpec) -> Result<Vec<Document>> {
        let connection = self.resolve_connection(config, spec.connection.as_deref())?;
        let client = connect(config, connection).await?;
        let database = client.database(&spec.database);
        let collection = database.collection::<Document>(&spec.collection);

        let filter = match normalize_json_option(spec.filter.clone()) {
            Some(value) => parse_json_document("filter", &value)?,
            None => Document::new(),
        };

        let projection = normalize_json_option(spec.projection.clone())
            .map(|value| parse_json_document("projection", &value))
            .transpose()?;
        let sort = normalize_json_option(spec.sort.clone())
            .map(|value| parse_json_document("sort", &value))
            .transpose()?;

        let mut options = FindOptions::default();
        options.projection = projection;
        options.sort = sort;
        if let Some(limit) = spec.limit {
            options.limit = Some(limit as i64);
        }
        options.max_time = Some(config.query_timeout());

        let cursor = collection
            .find(filter)
            .with_options(options)
            .await
            .with_context(|| {
                format!(
                    "failed to run find on {}.{}",
                    spec.database, spec.collection
                )
            })?;
        let documents = cursor
            .try_collect()
            .await
            .context("failed to load query results")?;
        Ok(documents)
    }

    pub async fn execute_aggregation(
        &self,
        config: &Config,
        spec: &AggregationSpec,
    ) -> Result<Vec<Document>> {
        let connection = self.resolve_connection(config, spec.connection.as_deref())?;
        let client = connect(config, connection).await?;
        let database = client.database(&spec.database);
        let collection = database.collection::<Document>(&spec.collection);

        let pipeline = parse_json_pipeline(&spec.pipeline)?;
        if let Some(stage) = find_pipeline_write_stage(&pipeline) {
            WriteGuard::from_config(config).ensure_pipeline_allowed(stage)?;
        }
        let options = AggregateOptions::builder()
            .max_time(config.query_timeout())
            .build();
        let cursor = collection
            .aggregate(pipeline)
            .with_options(options)
            .await
            .with_context(|| {
                format!(
                    "failed to run aggregation on {}.{}",
                    spec.database, spec.collection
                )
            })?;
        let documents = cursor
            .try_collect()
            .await
            .context("failed to load aggregation results")?;
        Ok(documents)
    }

    pub async fn list_databases(
        &self,
        config: &Config,
        connection: Option<&str>,
    ) -> Result<Vec<String>> {
        let connection = self.resolve_connection(config, connection)?;
        let client = connect(config, connection).await?;
        let databases = client
            .list_database_names()
            .await
            .context("failed to list databases")?;
        Ok(databases)
    }

    pub async fn list_collections(
        &self,
        config: &Config,
        connection: Option<&str>,
        database: &str,
    ) -> Result<Vec<String>> {
        let connection = self.resolve_connection(config, connection)?;
        let client = connect(config, connection).await?;
        let database = client.database(database);
        let collections = database
            .list_collection_names()
            .await
            .context("failed to list collections")?;
        Ok(collections)
    }

    pub async fn list_documents(
        &self,
        config: &Config,
        spec: &DocumentListSpec,
    ) -> Result<Vec<Document>> {
        let connection = self.resolve_connection(config, spec.connection.as_deref())?;
        let client = connect(config, connection).await?;
        let database = client.database(&spec.database);
        let collection = database.collection::<Document>(&spec.collection);

        let mut options = FindOptions::default();
        options.skip = Some(spec.skip);
        options.limit = Some(spec.limit as i64);
        options.max_time = Some(config.query_timeout());

        let cursor = collection
            .find(Document::new())
            .with_options(options)
            .await
            .with_context(|| {
                format!(
                    "failed to load documents from {}.{}",
                    spec.database, spec.collection
                )
            })?;
        let documents = cursor
            .try_collect()
            .await
            .context("failed to load documents")?;
        Ok(documents)
    }

    pub async fn insert_document(
        &self,
        config: &Config,
        spec: &DocumentInsertSpec,
    ) -> Result<Bson> {
        WriteGuard::from_config(config).ensure_write_allowed("insert documents")?;
        let connection = self.resolve_connection(config, spec.connection.as_deref())?;
        let client = connect(config, connection).await?;
        let database = client.database(&spec.database);
        let collection = database.collection::<Document>(&spec.collection);

        let result = collection
            .insert_one(spec.document.clone())
            .await
            .with_context(|| {
                format!(
                    "failed to insert document into {}.{}",
                    spec.database, spec.collection
                )
            })?;
        Ok(result.inserted_id)
    }

    pub async fn replace_document(
        &self,
        config: &Config,
        spec: &DocumentReplaceSpec,
    ) -> Result<()> {
        WriteGuard::from_config(config).ensure_write_allowed("replace documents")?;
        let connection = self.resolve_connection(config, spec.connection.as_deref())?;
        let client = connect(config, connection).await?;
        let database = client.database(&spec.database);
        let collection = database.collection::<Document>(&spec.collection);

        let filter = bson::doc! { "_id": spec.id.clone() };
        let result = collection
            .replace_one(filter, spec.document.clone())
            .await
            .with_context(|| {
                format!(
                    "failed to replace document in {}.{}",
                    spec.database, spec.collection
                )
            })?;
        if result.matched_count == 0 {
            anyhow::bail!(
                "document not found in {}.{}",
                spec.database,
                spec.collection
            );
        }
        Ok(())
    }

    pub async fn delete_document(&self, config: &Config, spec: &DocumentDeleteSpec) -> Result<()> {
        WriteGuard::from_config(config).ensure_write_allowed("delete documents")?;
        let connection = self.resolve_connection(config, spec.connection.as_deref())?;
        let client = connect(config, connection).await?;
        let database = client.database(&spec.database);
        let collection = database.collection::<Document>(&spec.collection);

        let filter = bson::doc! { "_id": spec.id.clone() };
        let result = collection.delete_one(filter).await.with_context(|| {
            format!(
                "failed to delete document from {}.{}",
                spec.database, spec.collection
            )
        })?;
        if result.deleted_count == 0 {
            anyhow::bail!(
                "document not found in {}.{}",
                spec.database,
                spec.collection
            );
        }
        Ok(())
    }
}

async fn connect(config: &Config, connection: &ConnectionSpec) -> Result<Client> {
    let redacted_uri = redact_connection_uri(&connection.uri);
    let mut options = ClientOptions::parse(&connection.uri)
        .await
        .with_context(|| format!("unable to parse connection options for {redacted_uri}"))?;
    options.connect_timeout = Some(config.connect_timeout());
    options.server_selection_timeout = Some(config.connect_timeout());
    Client::with_options(options).with_context(|| format!("unable to connect to {redacted_uri}"))
}

fn normalize_json_option(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub fn parse_json_document(label: &str, value: &str) -> Result<Document> {
    let json: Value =
        serde_json::from_str(value).with_context(|| format!("invalid JSON in {label}"))?;
    let bson = Bson::try_from(json).with_context(|| format!("invalid JSON in {label}"))?;
    match bson {
        Bson::Document(document) => Ok(document),
        _ => anyhow::bail!("{label} must be a JSON object"),
    }
}

fn parse_json_pipeline(value: &str) -> Result<Vec<Document>> {
    let json: Value = serde_json::from_str(value).context("invalid JSON in pipeline")?;
    let bson = Bson::try_from(json).context("invalid JSON in pipeline")?;
    match bson {
        Bson::Array(items) => items
            .into_iter()
            .map(|item| match item {
                Bson::Document(document) => Ok(document),
                _ => anyhow::bail!("pipeline items must be JSON objects"),
            })
            .collect(),
        _ => anyhow::bail!("pipeline must be a JSON array"),
    }
}

fn find_pipeline_write_stage(pipeline: &[Document]) -> Option<&'static str> {
    for stage in pipeline {
        if stage.contains_key("$out") {
            return Some("$out");
        }
        if stage.contains_key("$merge") {
            return Some("$merge");
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::oid::ObjectId;

    #[test]
    fn parse_json_document_supports_extjson_oid() {
        let oid = ObjectId::new();
        let value = format!(r#"{{ "_id": {{ "$oid": "{}" }}, "name": "sample" }}"#, oid);
        let doc = parse_json_document("filter", &value).expect("parse extjson");
        match doc.get("_id") {
            Some(Bson::ObjectId(parsed)) => assert_eq!(parsed, &oid),
            other => panic!("unexpected _id value: {other:?}"),
        }
    }

    #[test]
    fn parse_json_pipeline_supports_extjson_oid() {
        let oid = ObjectId::new();
        let value = format!(r#"[{{ "$match": {{ "_id": {{ "$oid": "{}" }} }} }}]"#, oid);
        let pipeline = parse_json_pipeline(&value).expect("parse pipeline");
        let match_stage = pipeline.first().expect("first stage");
        let filter = match_stage.get("$match").expect("match stage");
        match filter {
            Bson::Document(doc) => match doc.get("_id") {
                Some(Bson::ObjectId(parsed)) => assert_eq!(parsed, &oid),
                other => panic!("unexpected _id value: {other:?}"),
            },
            other => panic!("unexpected match stage: {other:?}"),
        }
    }

    #[test]
    fn find_pipeline_write_stage_detects_out_and_merge() {
        let pipeline = vec![
            bson::doc! { "$match": { "active": true } },
            bson::doc! { "$out": "archive" },
        ];
        assert_eq!(find_pipeline_write_stage(&pipeline), Some("$out"));

        let pipeline = vec![bson::doc! { "$merge": { "into": "archive" } }];
        assert_eq!(find_pipeline_write_stage(&pipeline), Some("$merge"));

        let pipeline = vec![bson::doc! { "$group": { "_id": "$userId" } }];
        assert_eq!(find_pipeline_write_stage(&pipeline), None);
    }
}
