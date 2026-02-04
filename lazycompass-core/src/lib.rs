use serde::{Deserialize, Serialize};
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeConfig {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoggingConfig {
    pub level: Option<String>,
    pub file: Option<String>,
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
}
