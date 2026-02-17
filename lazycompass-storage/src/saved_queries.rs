use anyhow::{Context, Result};
use lazycompass_core::{SavedQuery, SavedScope, redact_sensitive_text};
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

pub fn load_saved_queries(paths: &ConfigPaths) -> Result<(Vec<SavedQuery>, Vec<String>)> {
    let Some(dir) = paths.repo_queries_dir() else {
        return Ok((Vec::new(), Vec::new()));
    };

    load_queries_from_dir(&dir)
}

pub fn saved_query_path(paths: &ConfigPaths, id: &str) -> Result<PathBuf> {
    validate_saved_id(id)?;
    let dir = paths.repo_queries_dir().ok_or_else(|| {
        anyhow::anyhow!("repository config not found; run inside a repo with .lazycompass")
    })?;
    Ok(dir.join(format!("{id}.json")))
}

pub fn write_saved_query(
    paths: &ConfigPaths,
    query: &SavedQuery,
    overwrite: bool,
) -> Result<PathBuf> {
    query.validate().context("invalid saved query")?;
    let parsed_scope = parse_scope_from_saved_id(&query.id)?;
    if parsed_scope != query.scope {
        anyhow::bail!("saved query id '{}' does not match its scope", query.id);
    }
    let path = saved_query_path(paths, &query.id)?;
    if path.exists() && !overwrite {
        anyhow::bail!("saved query '{}' already exists", query.id);
    }
    if let Some(parent) = path.parent() {
        ensure_secure_dir(parent)?;
    }
    let contents = serde_json::to_string_pretty(&saved_query_payload(query)?)
        .context("unable to serialize saved query")?;
    write_secure_file(&path, &contents)
        .with_context(|| format!("unable to write saved query {}", path.display()))?;
    Ok(path)
}

fn load_queries_from_dir(dir: &std::path::Path) -> Result<(Vec<SavedQuery>, Vec<String>)> {
    let paths = collect_json_paths(dir)?;
    let mut queries = Vec::with_capacity(paths.len());
    let mut warnings = Vec::new();

    for path in paths {
        let result = (|| -> Result<SavedQuery> {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("unable to read saved query file {}", path.display()))?;
            let json: Value = serde_json::from_str(&contents)
                .with_context(|| format!("invalid JSON in saved query {}", path.display()))?;
            let id = saved_id_from_path(&path)?;
            let scope = parse_scope_from_saved_id(&id)
                .with_context(|| format!("invalid saved query id '{id}'"))?;
            let query = parse_saved_query_payload(&json, id, scope)
                .with_context(|| format!("invalid saved query {}", path.display()))?;
            Ok(query)
        })();

        match result {
            Ok(query) => queries.push(query),
            Err(error) => {
                let warning = format!("skipping saved query {}: {error}", path.display());
                warnings.push(redact_sensitive_text(&warning));
            }
        }
    }

    Ok((queries, warnings))
}

fn parse_saved_query_payload(json: &Value, id: String, scope: SavedScope) -> Result<SavedQuery> {
    let object = json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("saved query payload must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "filter" | "projection" | "sort" | "limit") {
            anyhow::bail!("unknown field '{key}' in saved query payload");
        }
    }

    let filter = json_field_as_json_string(object, "filter")?;
    let projection = json_field_as_json_string(object, "projection")?;
    let sort = json_field_as_json_string(object, "sort")?;
    let limit = match object.get("limit") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("field 'limit' must be a non-negative integer"))?,
        ),
    };

    let query = SavedQuery {
        id,
        scope,
        filter,
        projection,
        sort,
        limit,
    };
    query.validate().context("invalid saved query data")?;
    Ok(query)
}

fn saved_query_payload(query: &SavedQuery) -> Result<Value> {
    let mut object = serde_json::Map::new();
    if let Some(filter) = query.filter.as_deref() {
        object.insert(
            "filter".to_string(),
            serde_json::from_str(filter).context("saved query filter must be valid JSON")?,
        );
    }
    if let Some(projection) = query.projection.as_deref() {
        object.insert(
            "projection".to_string(),
            serde_json::from_str(projection)
                .context("saved query projection must be valid JSON")?,
        );
    }
    if let Some(sort) = query.sort.as_deref() {
        object.insert(
            "sort".to_string(),
            serde_json::from_str(sort).context("saved query sort must be valid JSON")?,
        );
    }
    if let Some(limit) = query.limit {
        object.insert("limit".to_string(), Value::from(limit));
    }
    Ok(Value::Object(object))
}

fn json_field_as_json_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>> {
    let Some(value) = object.get(field) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let serialized = serde_json::to_string(value)
        .with_context(|| format!("unable to serialize field '{field}'"))?;
    Ok(Some(serialized))
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use lazycompass_core::{SavedQuery, SavedScope};
    use std::fs;

    use super::{load_saved_queries, write_saved_query};
    use crate::{
        ConfigPaths,
        test_support::{temp_root, write_file},
    };

    #[test]
    fn load_saved_queries_from_repo() -> Result<()> {
        let root = temp_root("saved_queries_repo");
        let repo_root = root.join("repo");

        write_file(
            &repo_root.join(".lazycompass/queries/lazycompass.users.active_users.json"),
            r#"{
  "filter": { "active": true },
  "projection": { "email": 1 }
}
"#,
        );

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let (queries, warnings) = load_saved_queries(&paths)?;

        assert!(warnings.is_empty());
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].id, "lazycompass.users.active_users");
        assert!(matches!(
            queries[0].scope,
            SavedScope::Scoped {
                ref database,
                ref collection
            } if database == "lazycompass" && collection == "users"
        ));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn load_saved_specs_skips_invalid_files() -> Result<()> {
        let root = temp_root("saved_specs_invalid");
        let repo_root = root.join("repo");

        write_file(
            &repo_root.join(".lazycompass/queries/valid.json"),
            r#"{
  "filter": { "active": true }
}
"#,
        );
        write_file(
            &repo_root.join(".lazycompass/queries/db.name.json"),
            r#"{
  "filter": { "active": true }
}
"#,
        );

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let (queries, warnings) = load_saved_queries(&paths)?;

        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].id, "valid");
        assert_eq!(warnings.len(), 1);

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn write_saved_query_persists_json() -> Result<()> {
        let root = temp_root("write_saved_query");
        let repo_root = root.join("repo");

        let paths = ConfigPaths {
            global_root: root.join("global"),
            repo_root: Some(repo_root),
        };
        let query = SavedQuery {
            id: "lazycompass.orders.recent_orders".to_string(),
            scope: SavedScope::Scoped {
                database: "lazycompass".to_string(),
                collection: "orders".to_string(),
            },
            filter: Some("{ \"status\": \"open\" }".to_string()),
            projection: None,
            sort: None,
            limit: Some(50),
        };

        let path = write_saved_query(&paths, &query, false)?;
        assert!(path.is_file());
        let contents = fs::read_to_string(&path)?;
        let json: serde_json::Value = serde_json::from_str(&contents)?;
        assert_eq!(json.get("limit").and_then(|value| value.as_u64()), Some(50));

        let _ = fs::remove_dir_all(&root);
        Ok(())
    }
}
