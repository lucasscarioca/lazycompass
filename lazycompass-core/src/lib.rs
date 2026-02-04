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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQuery {
    pub name: String,
    pub connection: Option<String>,
    pub database: String,
    pub collection: String,
    pub filter: Option<String>,
    pub projection: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<u64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedAggregation {
    pub name: String,
    pub connection: Option<String>,
    pub database: String,
    pub collection: String,
    pub pipeline: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub enum QueryTarget {
    Saved {
        name: String,
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
        name: String,
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
        validate_required("name", &self.name)?;
        validate_required("database", &self.database)?;
        validate_required("collection", &self.collection)?;
        Ok(())
    }
}

impl SavedAggregation {
    pub fn validate(&self) -> Result<(), SpecValidationError> {
        validate_required("name", &self.name)?;
        validate_required("database", &self.database)?;
        validate_required("collection", &self.collection)?;
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
            name: " ".to_string(),
            connection: None,
            database: "lazycompass".to_string(),
            collection: "users".to_string(),
            filter: None,
            projection: None,
            sort: None,
            limit: None,
            notes: None,
        };

        assert!(matches!(
            query.validate(),
            Err(SpecValidationError::EmptyField { field: "name" })
        ));
    }

    #[test]
    fn saved_aggregation_validation_rejects_empty_pipeline() {
        let aggregation = SavedAggregation {
            name: "orders_by_user".to_string(),
            connection: None,
            database: "lazycompass".to_string(),
            collection: "orders".to_string(),
            pipeline: "  ".to_string(),
            notes: None,
        };

        assert!(matches!(
            aggregation.validate(),
            Err(SpecValidationError::EmptyField { field: "pipeline" })
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
}
