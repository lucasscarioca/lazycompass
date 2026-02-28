use anyhow::{Context, Result};
use lazycompass_core::{SavedAggregation, redact_sensitive_text};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::{
    ConfigPaths,
    saved_common::{
        collect_json_paths, parse_scope_from_saved_id, saved_id_from_path, validate_saved_id,
    },
    security::{ensure_secure_dir, write_secure_file},
};

pub fn load_saved_aggregations(
    paths: &ConfigPaths,
) -> Result<(Vec<SavedAggregation>, Vec<String>)> {
    let Some(dir) = paths.repo_aggregations_dir() else {
        return Ok((Vec::new(), Vec::new()));
    };

    load_aggregations_from_dir(&dir)
}

pub fn saved_aggregation_path(paths: &ConfigPaths, id: &str) -> Result<PathBuf> {
    validate_saved_id(id)?;
    let dir = paths.repo_aggregations_dir().ok_or_else(|| {
        anyhow::anyhow!("repository config not found; run inside a repo with .lazycompass")
    })?;
    Ok(dir.join(format!("{id}.json")))
}

pub fn write_saved_aggregation(
    paths: &ConfigPaths,
    aggregation: &SavedAggregation,
    overwrite: bool,
) -> Result<PathBuf> {
    aggregation
        .validate()
        .context("invalid saved aggregation")?;
    let parsed_scope = parse_scope_from_saved_id(&aggregation.id)?;
    if parsed_scope != aggregation.scope {
        anyhow::bail!(
            "saved aggregation id '{}' does not match its scope",
            aggregation.id
        );
    }
    let path = saved_aggregation_path(paths, &aggregation.id)?;
    if path.exists() && !overwrite {
        anyhow::bail!("saved aggregation '{}' already exists", aggregation.id);
    }
    if let Some(parent) = path.parent() {
        ensure_secure_dir(parent)?;
    }
    let pipeline_json: Value = serde_json::from_str(&aggregation.pipeline)
        .context("saved aggregation pipeline must be valid JSON")?;
    if !pipeline_json.is_array() {
        anyhow::bail!("saved aggregation pipeline must be a JSON array");
    }
    let contents = serde_json::to_string_pretty(&pipeline_json)
        .context("unable to serialize saved aggregation")?;
    write_secure_file(&path, &contents)
        .with_context(|| format!("unable to write saved aggregation {}", path.display()))?;
    Ok(path)
}

fn load_aggregations_from_dir(
    dir: &std::path::Path,
) -> Result<(Vec<SavedAggregation>, Vec<String>)> {
    let paths = collect_json_paths(dir)?;
    let mut aggregations = Vec::with_capacity(paths.len());
    let mut warnings = Vec::new();

    for path in paths {
        let result = (|| -> Result<SavedAggregation> {
            let contents = fs::read_to_string(&path).with_context(|| {
                format!("unable to read saved aggregation file {}", path.display())
            })?;
            let json: Value = serde_json::from_str(&contents)
                .with_context(|| format!("invalid JSON in saved aggregation {}", path.display()))?;
            let id = saved_id_from_path(&path)?;
            let scope = parse_scope_from_saved_id(&id)
                .with_context(|| format!("invalid saved aggregation id '{id}'"))?;
            let aggregation = parse_saved_aggregation_payload(&json, id, scope)
                .with_context(|| format!("invalid saved aggregation {}", path.display()))?;
            Ok(aggregation)
        })();

        match result {
            Ok(aggregation) => aggregations.push(aggregation),
            Err(error) => {
                let warning = format!("skipping saved aggregation {}: {error}", path.display());
                warnings.push(redact_sensitive_text(&warning));
            }
        }
    }

    Ok((aggregations, warnings))
}

fn parse_saved_aggregation_payload(
    json: &Value,
    id: String,
    scope: lazycompass_core::SavedScope,
) -> Result<SavedAggregation> {
    if !json.is_array() {
        anyhow::bail!("saved aggregation payload must be a JSON array");
    }
    let aggregation = SavedAggregation {
        id,
        scope,
        pipeline: serde_json::to_string(json).context("unable to serialize pipeline JSON")?,
    };
    aggregation
        .validate()
        .context("invalid saved aggregation data")?;
    Ok(aggregation)
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use lazycompass_core::{SavedAggregation, SavedScope};
    use std::fs;

    use super::{load_saved_aggregations, write_saved_aggregation};
    use crate::{
        ConfigPaths,
        test_support::{temp_root, write_file},
    };

    #[test]
    fn load_saved_aggregations_from_repo() -> Result<()> {
        let root = temp_root("saved_aggs_repo");
        let repo_root = root.join("repo");

        write_file(
            &repo_root.join(".lazycompass/aggregations/orders_by_user.json"),
            r#"[
  { "$group": { "_id": "$userId" } }
]
"#,
        );

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let (aggregations, warnings) = load_saved_aggregations(&paths)?;

        assert!(warnings.is_empty());
        assert_eq!(aggregations.len(), 1);
        assert_eq!(aggregations[0].id, "orders_by_user");
        assert!(matches!(aggregations[0].scope, SavedScope::Shared));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn write_saved_aggregation_rejects_overwrite_without_flag() -> Result<()> {
        let root = temp_root("write_saved_aggregation");
        let repo_root = root.join("repo");

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let aggregation = SavedAggregation {
            id: "orders_by_user".to_string(),
            scope: SavedScope::Shared,
            pipeline: "[]".to_string(),
        };

        let _ = write_saved_aggregation(&paths, &aggregation, false)?;
        assert!(write_saved_aggregation(&paths, &aggregation, false).is_err());

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_saved_aggregations_warns_on_invalid_payload() -> Result<()> {
        let root = temp_root("saved_aggs_invalid");
        let repo_root = root.join("repo");

        write_file(
            &repo_root.join(".lazycompass/aggregations/orders_by_user.json"),
            r#"{"$group":{"_id":"$userId"}}"#,
        );

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let (aggregations, warnings) = load_saved_aggregations(&paths)?;

        assert!(aggregations.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("skipping saved aggregation"));
        assert!(warnings[0].contains("orders_by_user.json"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn write_saved_aggregation_rejects_scope_mismatch() -> Result<()> {
        let root = temp_root("write_saved_aggregation_scope");
        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(root.join("repo")),
        };
        let aggregation = SavedAggregation {
            id: "orders_by_user".to_string(),
            scope: SavedScope::Scoped {
                database: "app".to_string(),
                collection: "orders".to_string(),
            },
            pipeline: "[]".to_string(),
        };

        let err =
            write_saved_aggregation(&paths, &aggregation, false).expect_err("expected mismatch");
        assert!(err.to_string().contains("does not match its scope"));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn write_saved_aggregation_rejects_invalid_pipeline_json() -> Result<()> {
        let root = temp_root("write_saved_aggregation_invalid");
        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(root.join("repo")),
        };
        let aggregation = SavedAggregation {
            id: "orders_by_user".to_string(),
            scope: SavedScope::Shared,
            pipeline: "{invalid".to_string(),
        };

        let err = write_saved_aggregation(&paths, &aggregation, false)
            .expect_err("expected invalid pipeline");
        assert!(
            err.to_string()
                .contains("saved aggregation pipeline must be valid JSON")
        );

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
