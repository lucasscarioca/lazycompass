use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSpec {
    pub name: String,
    pub uri: String,
    pub default_database: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub connections: Vec<ConnectionSpec>,
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
