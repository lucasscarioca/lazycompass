use anyhow::{Context, Result};
use lazycompass_core::SavedScope;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn collect_json_paths(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("unable to read directory {}", dir.display()))?
    {
        let entry = entry
            .with_context(|| format!("unable to read directory entry in {}", dir.display()))?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            paths.push(path);
        }
    }

    paths.sort();
    Ok(paths)
}

pub(crate) fn saved_id_from_path(path: &Path) -> Result<String> {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| anyhow::anyhow!("saved spec filename is not valid UTF-8"))?;
    validate_saved_id(stem)?;
    Ok(stem.to_string())
}

pub(crate) fn validate_saved_id(id: &str) -> Result<()> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        anyhow::bail!("saved id cannot be empty");
    }
    if trimmed != id {
        anyhow::bail!("saved id cannot have leading or trailing whitespace");
    }
    if id.contains('/') || id.contains('\\') {
        anyhow::bail!("saved id cannot contain path separators");
    }

    let segments: Vec<&str> = id.split('.').collect();
    if segments.iter().any(|segment| segment.trim().is_empty()) {
        anyhow::bail!("saved id cannot contain empty segments");
    }
    if segments.len() == 2 {
        anyhow::bail!(
            "saved id with two segments is invalid; use <name> or <db>.<collection>.<name>"
        );
    }
    Ok(())
}

pub(crate) fn parse_scope_from_saved_id(id: &str) -> Result<SavedScope> {
    validate_saved_id(id)?;
    let segments: Vec<&str> = id.split('.').collect();
    if segments.len() == 1 {
        return Ok(SavedScope::Shared);
    }
    let database = segments
        .first()
        .ok_or_else(|| anyhow::anyhow!("missing database segment"))?
        .to_string();
    let name = segments
        .last()
        .ok_or_else(|| anyhow::anyhow!("missing name segment"))?;
    if name.trim().is_empty() {
        anyhow::bail!("missing name segment");
    }
    let collection = segments[1..segments.len() - 1].join(".");
    if collection.trim().is_empty() {
        anyhow::bail!("missing collection segment");
    }
    Ok(SavedScope::Scoped {
        database,
        collection,
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use lazycompass_core::SavedScope;

    use super::parse_scope_from_saved_id;

    #[test]
    fn parse_scope_from_saved_id_supports_multidot_collection() -> Result<()> {
        let scope = parse_scope_from_saved_id("app.foo.bar.users.active")?;
        assert!(matches!(
            scope,
            SavedScope::Scoped {
                ref database,
                ref collection
            } if database == "app" && collection == "foo.bar.users"
        ));
        Ok(())
    }

    #[test]
    fn parse_scope_from_saved_id_rejects_two_segments() {
        let err = parse_scope_from_saved_id("app.users").expect_err("should fail");
        assert!(err.to_string().contains("two segments"));
    }
}
