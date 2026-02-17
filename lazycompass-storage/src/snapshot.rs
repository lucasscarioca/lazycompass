use anyhow::Result;
use lazycompass_core::{Config, SavedAggregation, SavedQuery, connection_security_warnings};

use crate::{
    ConfigPaths, load_config, load_saved_aggregations, load_saved_queries,
    security::permission_warnings,
};

#[derive(Debug, Clone)]
pub struct StorageSnapshot {
    pub config: Config,
    pub queries: Vec<SavedQuery>,
    pub aggregations: Vec<SavedAggregation>,
    pub warnings: Vec<String>,
}

pub fn load_storage(paths: &ConfigPaths) -> Result<StorageSnapshot> {
    let config = load_config(paths)?;
    load_storage_with_config(paths, config)
}

pub fn load_storage_with_config(paths: &ConfigPaths, config: Config) -> Result<StorageSnapshot> {
    let mut warnings = connection_security_warnings(&config);
    warnings.extend(permission_warnings(paths));
    let (queries, query_warnings) = load_saved_queries(paths)?;
    let (aggregations, aggregation_warnings) = load_saved_aggregations(paths)?;
    warnings.extend(query_warnings);
    warnings.extend(aggregation_warnings);
    Ok(StorageSnapshot {
        config,
        queries,
        aggregations,
        warnings,
    })
}
