use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSpec {
    pub name: String,
    pub uri: String,
    pub default_database: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub connections: Vec<ConnectionSpec>,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub read_only: Option<bool>,
    #[serde(default)]
    pub allow_pipeline_writes: Option<bool>,
    #[serde(default)]
    pub allow_insecure: Option<bool>,
    #[serde(default)]
    pub timeouts: TimeoutConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeConfig {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoggingConfig {
    pub level: Option<String>,
    pub file: Option<String>,
    pub max_size_mb: Option<u64>,
    pub max_backups: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimeoutConfig {
    pub connect_ms: Option<u64>,
    pub query_ms: Option<u64>,
}

pub const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 10_000;
pub const DEFAULT_QUERY_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_LOG_MAX_SIZE_MB: u64 = 10;
pub const DEFAULT_LOG_MAX_BACKUPS: u64 = 3;

impl LoggingConfig {
    pub fn max_size_bytes(&self) -> u64 {
        self.max_size_mb
            .unwrap_or(DEFAULT_LOG_MAX_SIZE_MB)
            .saturating_mul(1024)
            .saturating_mul(1024)
    }

    pub fn max_backups(&self) -> u64 {
        self.max_backups.unwrap_or(DEFAULT_LOG_MAX_BACKUPS)
    }
}

pub fn redact_connection_uri(uri: &str) -> String {
    let Some(scheme_end) = uri.find("://") else {
        return uri.to_string();
    };
    let authority_start = scheme_end + 3;
    let mut authority_end = uri.len();
    for (index, ch) in uri[authority_start..].char_indices() {
        if ch == '/' || ch == '?' {
            authority_end = authority_start + index;
            break;
        }
    }

    let authority = &uri[authority_start..authority_end];
    let Some(at_index) = authority.rfind('@') else {
        return uri.to_string();
    };

    let mut redacted = String::with_capacity(uri.len());
    redacted.push_str(&uri[..authority_start]);
    redacted.push_str("***");
    redacted.push_str(&authority[at_index..]);
    redacted.push_str(&uri[authority_end..]);
    redacted
}

pub fn redact_uris_in_text(input: &str) -> String {
    if !input.contains("mongodb://") && !input.contains("mongodb+srv://") {
        return input.to_string();
    }

    let mut output = String::with_capacity(input.len());
    let mut remainder = input;

    while let Some(index) = find_next_mongo_uri(remainder) {
        output.push_str(&remainder[..index]);
        let rest = &remainder[index..];
        let end = rest
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | ')' | ']' | '}' | ','))
            .unwrap_or(rest.len());
        let uri = &rest[..end];
        output.push_str(&redact_connection_uri(uri));
        remainder = &rest[end..];
    }

    output.push_str(remainder);
    output
}

pub fn redact_sensitive_text(input: &str) -> String {
    let output = redact_uris_in_text(input);
    redact_query_fields_in_text(&output)
}

fn find_next_mongo_uri(value: &str) -> Option<usize> {
    let standard = value.find("mongodb://");
    let srv = value.find("mongodb+srv://");
    match (standard, srv) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn redact_query_fields_in_text(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    if !lower.contains("filter") && !lower.contains("pipeline") {
        return input.to_string();
    }

    let mut output = String::with_capacity(input.len());
    for (index, line) in input.split('\n').enumerate() {
        if index > 0 {
            output.push('\n');
        }
        let line = redact_query_field_in_line(line, "filter");
        let line = redact_query_field_in_line(&line, "pipeline");
        output.push_str(&line);
    }
    output
}

fn redact_query_field_in_line(line: &str, field: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let mut search_start = 0;
    while let Some(pos) = lower[search_start..].find(field) {
        let pos = search_start + pos;
        if pos > 0 {
            let prev = line.as_bytes()[pos - 1];
            if (prev as char).is_ascii_alphanumeric() || prev == b'_' {
                search_start = pos + field.len();
                continue;
            }
        }

        let mut index = pos + field.len();
        let bytes = line.as_bytes();
        while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }
        let separator = bytes[index];
        if separator == b'=' || separator == b':' {
            let prefix_end = index + 1;
            let mut redacted = String::with_capacity(line.len());
            redacted.push_str(&line[..prefix_end]);
            redacted.push_str(" <redacted>");
            return redacted;
        }

        search_start = pos + field.len();
    }

    line.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    JsonPretty,
    Table,
}

impl OutputFormat {
    pub fn label(self) -> &'static str {
        match self {
            OutputFormat::JsonPretty => "json",
            OutputFormat::Table => "table",
        }
    }
}

impl Config {
    pub fn read_only(&self) -> bool {
        self.read_only.unwrap_or(true)
    }

    pub fn allow_pipeline_writes(&self) -> bool {
        self.allow_pipeline_writes.unwrap_or(false)
    }

    pub fn allow_insecure(&self) -> bool {
        self.allow_insecure.unwrap_or(false)
    }

    pub fn connect_timeout(&self) -> Duration {
        Duration::from_millis(
            self.timeouts
                .connect_ms
                .unwrap_or(DEFAULT_CONNECT_TIMEOUT_MS),
        )
    }

    pub fn query_timeout(&self) -> Duration {
        Duration::from_millis(self.timeouts.query_ms.unwrap_or(DEFAULT_QUERY_TIMEOUT_MS))
    }
}

pub fn connection_security_warnings(config: &Config) -> Vec<String> {
    if config.allow_insecure() {
        return Vec::new();
    }

    let mut warnings = Vec::new();
    for connection in &config.connections {
        let security = connection_security(&connection.uri);
        let missing = match (security.tls, security.auth) {
            (false, false) => Some("TLS and authentication"),
            (false, true) => Some("TLS"),
            (true, false) => Some("authentication"),
            (true, true) => None,
        };
        let Some(missing) = missing else {
            continue;
        };
        let redacted_uri = redact_connection_uri(&connection.uri);
        warnings.push(format!(
            "connection '{}' is missing {missing}; set allow_insecure=true to silence (uri: {redacted_uri})",
            connection.name
        ));
    }
    warnings
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConnectionSecurity {
    tls: bool,
    auth: bool,
}

fn connection_security(uri: &str) -> ConnectionSecurity {
    let mut tls = uri.starts_with("mongodb+srv://");
    let mut auth = uri_has_userinfo(uri);
    let mut tls_override = None;

    for (key, value) in mongo_uri_query_params(uri) {
        let key = key.to_ascii_lowercase();
        if (key == "tls" || key == "ssl")
            && let Some(parsed) = parse_bool(value)
        {
            tls_override = Some(parsed);
        }
        if key == "authmechanism" && !value.trim().is_empty() {
            auth = true;
        }
    }

    if let Some(tls_override) = tls_override {
        tls = tls_override;
    }

    ConnectionSecurity { tls, auth }
}

fn uri_has_userinfo(uri: &str) -> bool {
    let Some(scheme_end) = uri.find("://") else {
        return false;
    };
    let authority_start = scheme_end + 3;
    let mut authority_end = uri.len();
    for (index, ch) in uri[authority_start..].char_indices() {
        if ch == '/' || ch == '?' {
            authority_end = authority_start + index;
            break;
        }
    }

    let authority = &uri[authority_start..authority_end];
    let Some(at_index) = authority.rfind('@') else {
        return false;
    };
    let userinfo = &authority[..at_index];
    !userinfo.trim().is_empty()
}

fn mongo_uri_query_params(uri: &str) -> Vec<(&str, &str)> {
    let Some(start) = uri.find('?') else {
        return Vec::new();
    };
    let query = &uri[start + 1..];
    let mut params = Vec::new();
    for segment in query.split('&') {
        for item in segment.split(';') {
            if item.is_empty() {
                continue;
            }
            let mut parts = item.splitn(2, '=');
            let key = parts.next().unwrap_or_default();
            let value = parts.next().unwrap_or_default();
            params.push((key, value));
        }
    }
    params
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteGuard {
    read_only: bool,
    allow_pipeline_writes: bool,
}

impl WriteGuard {
    pub fn new(read_only: bool, allow_pipeline_writes: bool) -> Self {
        Self {
            read_only,
            allow_pipeline_writes,
        }
    }

    pub fn from_config(config: &Config) -> Self {
        Self::new(config.read_only(), config.allow_pipeline_writes())
    }

    pub fn ensure_write_allowed(&self, action: &str) -> Result<(), WriteGuardError> {
        if self.read_only {
            return Err(WriteGuardError::ReadOnly {
                action: action.to_string(),
            });
        }
        Ok(())
    }

    pub fn ensure_pipeline_allowed(&self, stage: &str) -> Result<(), WriteGuardError> {
        self.ensure_write_allowed("aggregation pipeline write stages")?;
        if !self.allow_pipeline_writes {
            return Err(WriteGuardError::PipelineWrite {
                stage: stage.to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WriteGuardError {
    #[error("read-only mode: {action} is disabled")]
    ReadOnly { action: String },
    #[error("pipeline stage '{stage}' is blocked; enable allow_pipeline_writes to proceed")]
    PipelineWrite { stage: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQuery {
    pub id: String,
    pub scope: SavedScope,
    pub filter: Option<String>,
    pub projection: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedAggregation {
    pub id: String,
    pub scope: SavedScope,
    pub pipeline: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SavedScope {
    Shared,
    Scoped {
        database: String,
        collection: String,
    },
}

impl SavedScope {
    pub fn validate(&self) -> Result<(), SpecValidationError> {
        if let SavedScope::Scoped {
            database,
            collection,
        } = self
        {
            validate_required("database", database)?;
            validate_required("collection", collection)?;
        }
        Ok(())
    }

    pub fn database_collection(&self) -> Option<(&str, &str)> {
        match self {
            SavedScope::Shared => None,
            SavedScope::Scoped {
                database,
                collection,
            } => Some((database.as_str(), collection.as_str())),
        }
    }
}

#[derive(Debug, Clone)]
pub enum QueryTarget {
    Saved {
        id: String,
        database: Option<String>,
        collection: Option<String>,
    },
    Inline {
        database: String,
        collection: String,
        filter: Option<String>,
        projection: Option<String>,
        sort: Option<String>,
        limit: Option<u64>,
    },
}

#[derive(Debug, Clone)]
pub struct QueryRequest {
    pub connection: Option<String>,
    pub output: OutputFormat,
    pub target: QueryTarget,
}

#[derive(Debug, Clone)]
pub enum AggregationTarget {
    Saved {
        id: String,
        database: Option<String>,
        collection: Option<String>,
    },
    Inline {
        database: String,
        collection: String,
        pipeline: String,
    },
}

#[derive(Debug, Clone)]
pub struct AggregationRequest {
    pub connection: Option<String>,
    pub output: OutputFormat,
    pub target: AggregationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SpecValidationError {
    #[error("field `{field}` cannot be empty")]
    EmptyField { field: &'static str },
}

impl SavedQuery {
    pub fn validate(&self) -> Result<(), SpecValidationError> {
        validate_required("id", &self.id)?;
        self.scope.validate()?;
        Ok(())
    }
}

impl SavedAggregation {
    pub fn validate(&self) -> Result<(), SpecValidationError> {
        validate_required("id", &self.id)?;
        self.scope.validate()?;
        validate_required("pipeline", &self.pipeline)?;
        Ok(())
    }
}

fn validate_required(field: &'static str, value: &str) -> Result<(), SpecValidationError> {
    if value.trim().is_empty() {
        return Err(SpecValidationError::EmptyField { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_query_validation_rejects_empty_fields() {
        let query = SavedQuery {
            id: " ".to_string(),
            scope: SavedScope::Scoped {
                database: "lazycompass".to_string(),
                collection: "users".to_string(),
            },
            filter: None,
            projection: None,
            sort: None,
            limit: None,
        };

        assert!(matches!(
            query.validate(),
            Err(SpecValidationError::EmptyField { field: "id" })
        ));
    }

    #[test]
    fn saved_aggregation_validation_rejects_empty_pipeline() {
        let aggregation = SavedAggregation {
            id: "orders_by_user".to_string(),
            scope: SavedScope::Scoped {
                database: "lazycompass".to_string(),
                collection: "orders".to_string(),
            },
            pipeline: "  ".to_string(),
        };

        assert!(matches!(
            aggregation.validate(),
            Err(SpecValidationError::EmptyField { field: "pipeline" })
        ));
    }

    #[test]
    fn saved_scope_validation_rejects_empty_database_or_collection() {
        let scope = SavedScope::Scoped {
            database: " ".to_string(),
            collection: "users".to_string(),
        };
        assert!(matches!(
            scope.validate(),
            Err(SpecValidationError::EmptyField { field: "database" })
        ));
    }

    #[test]
    fn redact_connection_uri_masks_userinfo() {
        let uri = "mongodb://user:password@localhost:27017/app?retryWrites=true";
        let redacted = redact_connection_uri(uri);
        assert_eq!(
            redacted,
            "mongodb://***@localhost:27017/app?retryWrites=true"
        );
    }

    #[test]
    fn redact_connection_uri_handles_srv_and_no_credentials() {
        let srv = "mongodb+srv://user@cluster0.example.mongodb.net/app";
        assert_eq!(
            redact_connection_uri(srv),
            "mongodb+srv://***@cluster0.example.mongodb.net/app"
        );

        let no_creds = "mongodb://localhost:27017";
        assert_eq!(redact_connection_uri(no_creds), no_creds);
    }

    #[test]
    fn redact_uris_in_text_masks_credentials() {
        let message = "error connecting to mongodb://user:secret@localhost:27017/app";
        let redacted = redact_uris_in_text(message);
        assert_eq!(
            redacted,
            "error connecting to mongodb://***@localhost:27017/app"
        );
    }

    #[test]
    fn redact_uris_in_text_handles_multiple_uris() {
        let message = "mongodb://user@a mongodb+srv://user:pass@b.example.com";
        let redacted = redact_uris_in_text(message);
        assert_eq!(redacted, "mongodb://***@a mongodb+srv://***@b.example.com");
    }

    #[test]
    fn redact_uris_in_text_returns_original_without_uri() {
        let message = "no secrets here";
        let redacted = redact_uris_in_text(message);
        assert_eq!(redacted, message);
    }

    #[test]
    fn redact_sensitive_text_masks_filter_and_pipeline() {
        let message = "invalid query: filter = \"{ \\\"active\\\": true }\"";
        let redacted = redact_sensitive_text(message);
        assert_eq!(redacted, "invalid query: filter = <redacted>");

        let message = "pipeline: [ { \"$match\": { \"active\": true } } ]";
        let redacted = redact_sensitive_text(message);
        assert_eq!(redacted, "pipeline: <redacted>");
    }

    #[test]
    fn connection_security_warnings_detect_missing_tls_and_auth() {
        let config = Config {
            connections: vec![ConnectionSpec {
                name: "local".to_string(),
                uri: "mongodb://localhost:27017".to_string(),
                default_database: None,
            }],
            theme: ThemeConfig::default(),
            logging: LoggingConfig::default(),
            read_only: None,
            allow_pipeline_writes: None,
            allow_insecure: None,
            timeouts: TimeoutConfig::default(),
        };

        let warnings = connection_security_warnings(&config);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("TLS and authentication"));
    }

    #[test]
    fn connection_security_warnings_respects_allow_insecure() {
        let config = Config {
            connections: vec![ConnectionSpec {
                name: "local".to_string(),
                uri: "mongodb://localhost:27017".to_string(),
                default_database: None,
            }],
            theme: ThemeConfig::default(),
            logging: LoggingConfig::default(),
            read_only: None,
            allow_pipeline_writes: None,
            allow_insecure: Some(true),
            timeouts: TimeoutConfig::default(),
        };

        let warnings = connection_security_warnings(&config);
        assert!(warnings.is_empty());
    }

    #[test]
    fn connection_security_treats_tls_and_auth_query_params_as_secure() {
        let config = Config {
            connections: vec![ConnectionSpec {
                name: "atlas".to_string(),
                uri: "mongodb://localhost:27017/?tls=true&authMechanism=SCRAM-SHA-256".to_string(),
                default_database: None,
            }],
            ..Config::default()
        };

        let warnings = connection_security_warnings(&config);
        assert!(warnings.is_empty());
    }

    #[test]
    fn connection_security_ssl_false_overrides_srv_default() {
        let config = Config {
            connections: vec![ConnectionSpec {
                name: "srv".to_string(),
                uri: "mongodb+srv://user@cluster.example.mongodb.net/?ssl=false".to_string(),
                default_database: None,
            }],
            ..Config::default()
        };

        let warnings = connection_security_warnings(&config);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("TLS"));
    }

    #[test]
    fn connection_security_reports_missing_auth_when_tls_only() {
        let config = Config {
            connections: vec![ConnectionSpec {
                name: "tls_only".to_string(),
                uri: "mongodb://localhost:27017/?tls=true".to_string(),
                default_database: None,
            }],
            ..Config::default()
        };

        let warnings = connection_security_warnings(&config);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("authentication"));
    }

    #[test]
    fn default_timeouts_are_applied() {
        let config = Config::default();
        assert_eq!(
            config.connect_timeout(),
            Duration::from_millis(DEFAULT_CONNECT_TIMEOUT_MS)
        );
        assert_eq!(
            config.query_timeout(),
            Duration::from_millis(DEFAULT_QUERY_TIMEOUT_MS)
        );
    }

    #[test]
    fn read_only_defaults_to_true() {
        let config = Config::default();
        assert!(config.read_only());

        let mut config = Config::default();
        config.read_only = Some(false);
        assert!(!config.read_only());
    }

    #[test]
    fn allow_pipeline_writes_defaults_to_false() {
        let config = Config::default();
        assert!(!config.allow_pipeline_writes());

        let mut config = Config::default();
        config.allow_pipeline_writes = Some(true);
        assert!(config.allow_pipeline_writes());
    }

    #[test]
    fn write_guard_blocks_in_read_only() {
        let guard = WriteGuard::new(true, true);
        let err = guard
            .ensure_write_allowed("insert documents")
            .expect_err("read-only");
        assert!(matches!(err, WriteGuardError::ReadOnly { .. }));
    }

    #[test]
    fn write_guard_blocks_pipeline_without_flag() {
        let guard = WriteGuard::new(false, false);
        let err = guard
            .ensure_pipeline_allowed("$out")
            .expect_err("pipeline guard");
        assert!(matches!(err, WriteGuardError::PipelineWrite { .. }));
    }

    #[test]
    fn write_guard_allows_pipeline_with_flag() {
        let guard = WriteGuard::new(false, true);
        assert!(guard.ensure_pipeline_allowed("$merge").is_ok());
    }
}
