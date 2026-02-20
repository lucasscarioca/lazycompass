use anyhow::Result;
use lazycompass_core::{Config, SavedAggregation, SavedQuery, connection_security_warnings};

use crate::{
    ConfigPaths, load_config, load_saved_aggregations, load_saved_queries,
    security::{normalize_permissions, permission_warnings},
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
    normalize_permissions(paths);
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

#[cfg(test)]
mod tests {
    use super::load_storage_with_config;
    use crate::ConfigPaths;
    use lazycompass_core::{Config, ConnectionSpec};
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("lazycompass_snapshot_{prefix}_{nonce}"));
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    #[test]
    fn load_storage_with_config_loads_saved_specs() {
        let root = temp_dir("saved_specs");
        let repo_root = root.join("repo");
        fs::create_dir_all(repo_root.join(".lazycompass/queries")).expect("create queries");
        fs::create_dir_all(repo_root.join(".lazycompass/aggregations"))
            .expect("create aggregations");
        fs::write(
            repo_root.join(".lazycompass/queries/shared_query.json"),
            r#"{ "filter": { "active": true } }"#,
        )
        .expect("write query");
        fs::write(
            repo_root.join(".lazycompass/aggregations/shared_agg.json"),
            r#"[{ "$match": { "active": true } }]"#,
        )
        .expect("write aggregation");

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let storage = load_storage_with_config(&paths, Config::default()).expect("load storage");

        assert_eq!(storage.queries.len(), 1);
        assert_eq!(storage.queries[0].id, "shared_query");
        assert_eq!(storage.aggregations.len(), 1);
        assert_eq!(storage.aggregations[0].id, "shared_agg");
        assert!(
            !storage
                .warnings
                .iter()
                .any(|warning| warning.contains("skipping saved"))
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_storage_with_config_aggregates_security_and_parse_warnings() {
        let root = temp_dir("warnings");
        let repo_root = root.join("repo");
        fs::create_dir_all(repo_root.join(".lazycompass/queries")).expect("create queries");
        fs::create_dir_all(repo_root.join(".lazycompass/aggregations"))
            .expect("create aggregations");
        fs::write(repo_root.join(".lazycompass/queries/broken.json"), "[]")
            .expect("write broken query");
        fs::write(
            repo_root.join(".lazycompass/aggregations/ok_agg.json"),
            r#"[{ "$match": { "active": true } }]"#,
        )
        .expect("write agg");

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let config = Config {
            connections: vec![ConnectionSpec {
                name: "insecure".to_string(),
                uri: "mongodb://localhost:27017".to_string(),
                default_database: None,
            }],
            ..Config::default()
        };
        let storage = load_storage_with_config(&paths, config).expect("load storage");

        assert!(storage.warnings.iter().any(|warning| {
            warning.contains("connection 'insecure' is missing TLS and authentication")
        }));
        assert!(
            storage
                .warnings
                .iter()
                .any(|warning| warning.contains("skipping saved query"))
        );

        let _ = fs::remove_dir_all(root);
    }
}
