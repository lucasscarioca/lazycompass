pub use mongodb::bson::{Bson, Document};

use anyhow::{Context, Result};
use futures::TryStreamExt;
use lazycompass_core::{
    Config, ConnectionSpec, WriteGuard, ensure_connection_security, redact_connection_uri,
};
use mongodb::{
    Client, bson,
    bson::{DateTime, oid::ObjectId},
    options::{AggregateOptions, ClientOptions, FindOptions},
};
use serde_json::Value;

const MAX_RESULT_DOCUMENTS: usize = 10_000;

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
        let documents = collect_result_documents(
            cursor,
            spec.limit
                .map(|limit| limit.min(MAX_RESULT_DOCUMENTS as u64) as usize)
                .unwrap_or(MAX_RESULT_DOCUMENTS),
            "query",
        )
        .await?;
        Ok(documents)
    }

    pub async fn execute_aggregation(
        &self,
        config: &Config,
        guard: WriteGuard,
        spec: &AggregationSpec,
    ) -> Result<Vec<Document>> {
        let connection = self.resolve_connection(config, spec.connection.as_deref())?;
        let client = connect(config, connection).await?;
        let database = client.database(&spec.database);
        let collection = database.collection::<Document>(&spec.collection);

        let pipeline = parse_json_pipeline(&spec.pipeline)?;
        ensure_pipeline_allowed(guard, &pipeline)?;
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
        let documents =
            collect_result_documents(cursor, MAX_RESULT_DOCUMENTS, "aggregation").await?;
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

    pub async fn list_indexes(
        &self,
        config: &Config,
        connection: Option<&str>,
        database: &str,
        collection: &str,
    ) -> Result<Vec<Document>> {
        let connection = self.resolve_connection(config, connection)?;
        let client = connect(config, connection).await?;
        let database_name = database.to_string();
        let collection_name = collection.to_string();
        let database = client.database(&database_name);
        let collection = database.collection::<Document>(&collection_name);

        let cursor = collection.list_indexes().await.with_context(|| {
            format!(
                "failed to list indexes for {}.{}",
                database_name, collection_name
            )
        })?;
        let indexes = cursor
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .map(|index| bson::to_document(&index).context("failed to serialize index spec"))
            .collect::<Result<Vec<_>>>()?;
        Ok(indexes)
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
        guard: WriteGuard,
        spec: &DocumentInsertSpec,
    ) -> Result<Bson> {
        ensure_write_allowed(guard, "insert documents")?;
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
        guard: WriteGuard,
        spec: &DocumentReplaceSpec,
    ) -> Result<()> {
        ensure_write_allowed(guard, "replace documents")?;
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
        ensure_document_matched(result.matched_count, &spec.database, &spec.collection)?;
        Ok(())
    }

    pub async fn delete_document(
        &self,
        config: &Config,
        guard: WriteGuard,
        spec: &DocumentDeleteSpec,
    ) -> Result<()> {
        ensure_write_allowed(guard, "delete documents")?;
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
        ensure_document_deleted(result.deleted_count, &spec.database, &spec.collection)?;
        Ok(())
    }
}

async fn connect(config: &Config, connection: &ConnectionSpec) -> Result<Client> {
    let redacted_uri = redact_connection_uri(&connection.uri);
    ensure_connection_security(config, connection).map_err(|error| anyhow::anyhow!("{error}"))?;
    let mut options = ClientOptions::parse(&connection.uri)
        .await
        .with_context(|| format!("unable to parse connection options for {redacted_uri}"))?;
    options.connect_timeout = Some(config.connect_timeout());
    options.server_selection_timeout = Some(config.connect_timeout());
    Client::with_options(options).with_context(|| format!("unable to connect to {redacted_uri}"))
}

async fn collect_result_documents(
    mut cursor: mongodb::Cursor<Document>,
    max_documents: usize,
    operation: &str,
) -> Result<Vec<Document>> {
    let mut documents = Vec::new();
    while let Some(document) = cursor
        .try_next()
        .await
        .with_context(|| format!("failed to load {operation} results"))?
    {
        ensure_result_limit(documents.len(), max_documents, operation)?;
        documents.push(document);
    }
    Ok(documents)
}

fn ensure_result_limit(current_len: usize, max_documents: usize, operation: &str) -> Result<()> {
    if current_len >= max_documents {
        anyhow::bail!(
            "{operation} result set exceeded the safety cap of {max_documents} documents; narrow the scope or add a limit"
        );
    }
    Ok(())
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

fn ensure_write_allowed(guard: WriteGuard, action: &str) -> Result<()> {
    guard.ensure_write_allowed(action).map_err(Into::into)
}

fn ensure_pipeline_allowed(guard: WriteGuard, pipeline: &[Document]) -> Result<()> {
    if let Some(stage) = find_pipeline_write_stage(pipeline) {
        guard.ensure_pipeline_allowed(stage)?;
    }
    Ok(())
}

fn ensure_document_matched(matched_count: u64, database: &str, collection: &str) -> Result<()> {
    if matched_count == 0 {
        anyhow::bail!("document not found in {}.{}", database, collection);
    }
    Ok(())
}

fn ensure_document_deleted(deleted_count: u64, database: &str, collection: &str) -> Result<()> {
    if deleted_count == 0 {
        anyhow::bail!("document not found in {}.{}", database, collection);
    }
    Ok(())
}

pub fn normalize_json_text(value: &str) -> Result<String> {
    preprocess_shell_literals(value)
}

pub fn parse_json_value(label: &str, value: &str) -> Result<Bson> {
    let normalized =
        normalize_json_text(value).with_context(|| format!("invalid JSON in {label}"))?;
    let json: Value =
        serde_json::from_str(&normalized).with_context(|| format!("invalid JSON in {label}"))?;
    Bson::try_from(json).with_context(|| format!("invalid JSON in {label}"))
}

pub fn parse_json_document(label: &str, value: &str) -> Result<Document> {
    let bson = parse_json_value(label, value)?;
    match bson {
        Bson::Document(document) => Ok(document),
        _ => anyhow::bail!("{label} must be a JSON object"),
    }
}

pub fn render_relaxed_extjson(value: &Bson) -> Value {
    value.clone().into_relaxed_extjson()
}

pub fn render_relaxed_extjson_string(value: &Bson) -> String {
    match render_relaxed_extjson(value) {
        Value::String(value) => value,
        Value::Null => "null".to_string(),
        value => value.to_string(),
    }
}

pub fn render_relaxed_extjson_document(document: &Document) -> Result<String> {
    serde_json::to_string_pretty(&Bson::Document(document.clone()).into_relaxed_extjson())
        .context("unable to serialize document")
}

pub fn render_relaxed_extjson_documents(documents: &[Document]) -> Result<String> {
    let values = documents
        .iter()
        .cloned()
        .map(Bson::Document)
        .map(|value| value.into_relaxed_extjson())
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&Value::Array(values))
        .context("unable to serialize results as JSON")
}

fn parse_json_pipeline(value: &str) -> Result<Vec<Document>> {
    let bson = parse_json_value("pipeline", value).context("invalid JSON in pipeline")?;
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

fn preprocess_shell_literals(input: &str) -> Result<String> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    let mut in_double = false;
    let mut escaped = false;

    while let Some((index, ch)) = chars.next() {
        if in_double {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }

        if ch == '"' {
            in_double = true;
            output.push(ch);
            continue;
        }

        if ch == 'O' && input[index..].starts_with("ObjectId") {
            let (replacement, next_index) = parse_shell_literal(input, index, "ObjectId")?;
            output.push_str(&replacement);
            advance_chars_to(&mut chars, next_index);
            continue;
        }

        if ch == 'I' && input[index..].starts_with("ISODate") {
            let (replacement, next_index) = parse_shell_literal(input, index, "ISODate")?;
            output.push_str(&replacement);
            advance_chars_to(&mut chars, next_index);
            continue;
        }

        output.push(ch);
    }

    Ok(output)
}

fn advance_chars_to(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, target: usize) {
    while chars.peek().is_some_and(|(index, _)| *index < target) {
        chars.next();
    }
}

fn parse_shell_literal(input: &str, start: usize, kind: &str) -> Result<(String, usize)> {
    let mut index = start + kind.len();
    index = skip_ascii_whitespace(input, index);
    expect_char(input, index, '(')?;
    index += 1;
    index = skip_ascii_whitespace(input, index);

    let (value, next_index) = parse_quoted_literal(input, index)?;
    let normalized = match kind {
        "ObjectId" => {
            ObjectId::parse_str(&value).with_context(|| format!("invalid {kind} literal"))?;
            serde_json::json!({ "$oid": value }).to_string()
        }
        "ISODate" => {
            DateTime::parse_rfc3339_str(&value)
                .with_context(|| format!("invalid {kind} literal"))?;
            serde_json::json!({ "$date": value }).to_string()
        }
        _ => anyhow::bail!("unsupported shell literal {kind}"),
    };

    index = skip_ascii_whitespace(input, next_index);
    expect_char(input, index, ')')?;
    Ok((normalized, index + 1))
}

fn skip_ascii_whitespace(input: &str, mut index: usize) -> usize {
    while let Some(ch) = input[index..].chars().next() {
        if !ch.is_ascii_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn expect_char(input: &str, index: usize, expected: char) -> Result<()> {
    match input[index..].chars().next() {
        Some(ch) if ch == expected => Ok(()),
        _ => anyhow::bail!("expected '{expected}'"),
    }
}

fn parse_quoted_literal(input: &str, start: usize) -> Result<(String, usize)> {
    let quote = input[start..]
        .chars()
        .next()
        .filter(|ch| *ch == '"' || *ch == '\'')
        .ok_or_else(|| anyhow::anyhow!("expected quoted string"))?;
    let mut index = start + quote.len_utf8();
    let mut value = String::new();
    let mut escaped = false;

    while let Some(ch) = input[index..].chars().next() {
        index += ch.len_utf8();
        if escaped {
            value.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Ok((value, index));
        }
        value.push(ch);
    }

    anyhow::bail!("unterminated quoted string")
}

#[cfg(test)]
mod tests {
    use super::*;
    use lazycompass_core::ConnectionSpec;

    fn config_with_connections(connections: Vec<ConnectionSpec>) -> Config {
        Config {
            connections,
            ..Config::default()
        }
    }

    fn connection(name: &str) -> ConnectionSpec {
        ConnectionSpec {
            name: name.to_string(),
            uri: format!("mongodb://{name}:27017"),
            default_database: None,
        }
    }

    #[test]
    fn resolve_connection_rejects_empty_config() {
        let executor = MongoExecutor::new();
        let err = executor
            .resolve_connection(&Config::default(), None)
            .expect_err("expected no connections error");
        assert!(err.to_string().contains("no connections configured"));
    }

    #[test]
    fn resolve_connection_uses_only_connection_when_name_missing() {
        let executor = MongoExecutor::new();
        let config = config_with_connections(vec![connection("local")]);
        let resolved = executor
            .resolve_connection(&config, None)
            .expect("resolve connection");
        assert_eq!(resolved.name, "local");
    }

    #[test]
    fn resolve_connection_requires_name_when_multiple_connections_exist() {
        let executor = MongoExecutor::new();
        let config = config_with_connections(vec![connection("a"), connection("b")]);
        let err = executor
            .resolve_connection(&config, None)
            .expect_err("expected missing connection name error");
        assert!(err.to_string().contains("multiple connections configured"));
    }

    #[test]
    fn resolve_connection_trims_name_input() {
        let executor = MongoExecutor::new();
        let config = config_with_connections(vec![connection("primary")]);
        let resolved = executor
            .resolve_connection(&config, Some("  primary  "))
            .expect("resolve trimmed connection");
        assert_eq!(resolved.name, "primary");
    }

    #[test]
    fn resolve_connection_errors_when_named_connection_missing() {
        let executor = MongoExecutor::new();
        let config = config_with_connections(vec![connection("primary")]);
        let err = executor
            .resolve_connection(&config, Some("secondary"))
            .expect_err("expected unknown connection");
        assert!(err.to_string().contains("connection 'secondary' not found"));
    }

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
    fn parse_json_document_rejects_invalid_json() {
        let err = parse_json_document("filter", "{invalid").expect_err("expected parse error");
        assert!(err.to_string().contains("invalid JSON in filter"));
    }

    #[test]
    fn parse_json_document_supports_shell_object_id() {
        let oid = ObjectId::new();
        let value = format!(r#"{{ "_id": ObjectId("{oid}") }}"#);
        let doc = parse_json_document("filter", &value).expect("parse shell object id");
        assert_eq!(doc.get("_id"), Some(&Bson::ObjectId(oid)));
    }

    #[test]
    fn parse_json_document_supports_shell_iso_date() {
        let value = r#"{ "createdAt": ISODate("2026-03-10T12:00:00Z") }"#;
        let doc = parse_json_document("filter", value).expect("parse shell iso date");
        match doc.get("createdAt") {
            Some(Bson::DateTime(date)) => {
                assert_eq!(
                    date.try_to_rfc3339_string().expect("rfc3339"),
                    "2026-03-10T12:00:00Z"
                );
            }
            other => panic!("unexpected createdAt value: {other:?}"),
        }
    }

    #[test]
    fn parse_json_value_supports_single_quoted_shell_literals() {
        let oid = ObjectId::new();
        let value = format!(
            r#"{{ "_id": ObjectId('{oid}'), "createdAt": ISODate('2026-03-10T12:00:00Z') }}"#
        );
        let doc = parse_json_document("filter", &value).expect("parse single-quoted literals");
        assert_eq!(doc.get("_id"), Some(&Bson::ObjectId(oid)));
        assert!(matches!(doc.get("createdAt"), Some(Bson::DateTime(_))));
    }

    #[test]
    fn parse_json_pipeline_supports_shell_literals() {
        let oid = ObjectId::new();
        let value = format!(
            r#"[{{ "$match": {{ "_id": ObjectId("{oid}"), "createdAt": {{ "$gte": ISODate("2026-03-10T12:00:00Z") }} }} }}]"#
        );
        let pipeline = parse_json_pipeline(&value).expect("parse shell pipeline");
        let stage = pipeline.first().expect("first stage");
        let filter = stage.get_document("$match").expect("match doc");
        assert_eq!(filter.get("_id"), Some(&Bson::ObjectId(oid)));
        let created_at = filter
            .get_document("createdAt")
            .expect("createdAt doc")
            .get("$gte");
        assert!(matches!(created_at, Some(Bson::DateTime(_))));
    }

    #[test]
    fn normalize_json_text_rewrites_shell_literals_to_extjson() {
        let oid = ObjectId::new();
        let input = format!(
            r#"{{ "_id": ObjectId("{oid}"), "createdAt": ISODate("2026-03-10T12:00:00Z") }}"#
        );
        let normalized = normalize_json_text(&input).expect("normalize");
        assert!(normalized.contains(r#""$oid""#));
        assert!(normalized.contains(r#""$date":"2026-03-10T12:00:00Z""#));
    }

    #[test]
    fn render_relaxed_extjson_document_uses_readable_date_strings() {
        let mut document = Document::new();
        document.insert("_id", ObjectId::new());
        document.insert(
            "createdAt",
            Bson::DateTime(DateTime::parse_rfc3339_str("2026-03-10T12:00:00Z").expect("date")),
        );
        let rendered = render_relaxed_extjson_document(&document).expect("render");
        assert!(rendered.contains(r#""$oid""#));
        assert!(rendered.contains(r#""$date": "2026-03-10T12:00:00Z""#));
    }

    #[test]
    fn parse_json_document_rejects_invalid_shell_object_id() {
        let err = parse_json_document("filter", r#"{ "_id": ObjectId("bad") }"#)
            .expect_err("expected invalid object id");
        assert!(err.to_string().contains("invalid JSON in filter"));
    }

    #[test]
    fn parse_json_document_rejects_invalid_shell_iso_date() {
        let err = parse_json_document("filter", r#"{ "createdAt": ISODate("not-a-date") }"#)
            .expect_err("expected invalid iso date");
        assert!(err.to_string().contains("invalid JSON in filter"));
    }

    #[test]
    fn parse_json_document_requires_object() {
        let err = parse_json_document("filter", "[]").expect_err("expected object error");
        assert!(err.to_string().contains("filter must be a JSON object"));
    }

    #[test]
    fn parse_json_pipeline_rejects_invalid_json() {
        let err = parse_json_pipeline("{invalid").expect_err("expected parse error");
        assert!(err.to_string().contains("invalid JSON in pipeline"));
    }

    #[test]
    fn parse_json_pipeline_requires_array() {
        let err = parse_json_pipeline("{}").expect_err("expected array error");
        assert!(err.to_string().contains("pipeline must be a JSON array"));
    }

    #[test]
    fn parse_json_pipeline_requires_object_items() {
        let err = parse_json_pipeline(r#"[{"$match":{}}, 1]"#).expect_err("expected item error");
        assert!(
            err.to_string()
                .contains("pipeline items must be JSON objects")
        );
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

    #[test]
    fn find_pipeline_write_stage_returns_first_matching_stage() {
        let pipeline = vec![
            bson::doc! { "$out": "archive" },
            bson::doc! { "$merge": { "into": "archive2" } },
        ];
        assert_eq!(find_pipeline_write_stage(&pipeline), Some("$out"));
    }

    #[test]
    fn ensure_write_allowed_blocks_without_dangerous_flag() {
        let err = ensure_write_allowed(WriteGuard::new(false, false), "insert documents")
            .expect_err("expected write-disabled error");
        assert!(err.to_string().contains("--dangerously-enable-write"));
    }

    #[test]
    fn ensure_pipeline_allowed_blocks_out_without_flag() {
        let pipeline = vec![bson::doc! { "$out": "archive" }];
        let err = ensure_pipeline_allowed(WriteGuard::new(true, false), &pipeline)
            .expect_err("expected pipeline write error");
        assert!(err.to_string().contains("--allow-pipeline-writes"));
    }

    #[test]
    fn ensure_pipeline_allowed_blocks_merge_without_dangerous_flag() {
        let pipeline = vec![bson::doc! { "$merge": { "into": "archive" } }];
        let err = ensure_pipeline_allowed(WriteGuard::new(false, true), &pipeline)
            .expect_err("expected write-disabled error");
        assert!(err.to_string().contains("--dangerously-enable-write"));
    }

    #[test]
    fn ensure_pipeline_allowed_accepts_non_write_pipeline() {
        let pipeline = vec![bson::doc! { "$match": { "active": true } }];
        assert!(ensure_pipeline_allowed(WriteGuard::new(false, false), &pipeline).is_ok());
    }

    #[test]
    fn ensure_pipeline_allowed_accepts_write_stage_when_enabled() {
        let pipeline = vec![bson::doc! { "$merge": { "into": "archive" } }];
        assert!(ensure_pipeline_allowed(WriteGuard::new(true, true), &pipeline).is_ok());
    }

    #[test]
    fn ensure_document_matched_requires_existing_document() {
        let err = ensure_document_matched(0, "app", "users").expect_err("expected not found");
        assert!(err.to_string().contains("document not found in app.users"));
    }

    #[test]
    fn ensure_document_deleted_requires_existing_document() {
        let err = ensure_document_deleted(0, "app", "users").expect_err("expected not found");
        assert!(err.to_string().contains("document not found in app.users"));
    }

    #[test]
    fn ensure_result_limit_rejects_oversized_results() {
        let err = ensure_result_limit(MAX_RESULT_DOCUMENTS, MAX_RESULT_DOCUMENTS, "query")
            .expect_err("expected safety cap error");
        assert!(err.to_string().contains("safety cap"));
    }
}
