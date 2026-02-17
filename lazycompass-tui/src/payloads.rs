use super::*;
pub(crate) fn render_query_payload_template(template: &SavedQuery) -> Result<String> {
    let mut object = serde_json::Map::new();
    if let Some(filter) = template.filter.as_deref() {
        object.insert(
            "filter".to_string(),
            serde_json::from_str(filter).context("saved query filter must be valid JSON")?,
        );
    }
    if let Some(projection) = template.projection.as_deref() {
        object.insert(
            "projection".to_string(),
            serde_json::from_str(projection)
                .context("saved query projection must be valid JSON")?,
        );
    }
    if let Some(sort) = template.sort.as_deref() {
        object.insert(
            "sort".to_string(),
            serde_json::from_str(sort).context("saved query sort must be valid JSON")?,
        );
    }
    if let Some(limit) = template.limit {
        object.insert("limit".to_string(), serde_json::Value::from(limit));
    }
    serde_json::to_string_pretty(&serde_json::Value::Object(object))
        .context("unable to serialize query template")
}

pub(crate) fn parse_query_payload_input(
    contents: &str,
    template: &SavedQuery,
) -> Result<SavedQuery> {
    let value: serde_json::Value =
        serde_json::from_str(contents).context("invalid JSON for saved query")?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("saved query payload must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "filter" | "projection" | "sort" | "limit") {
            anyhow::bail!("unknown field '{key}' in saved query payload");
        }
    }
    let filter = json_field_as_string(object, "filter")?;
    let projection = json_field_as_string(object, "projection")?;
    let sort = json_field_as_string(object, "sort")?;
    let limit = match object.get("limit") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .ok_or_else(|| anyhow::anyhow!("field 'limit' must be a non-negative integer"))?,
        ),
    };
    Ok(SavedQuery {
        id: template.id.clone(),
        scope: template.scope.clone(),
        filter,
        projection,
        sort,
        limit,
    })
}

pub(crate) fn render_aggregation_payload_template(template: &SavedAggregation) -> Result<String> {
    let pipeline: serde_json::Value = serde_json::from_str(&template.pipeline)
        .context("saved aggregation pipeline must be valid JSON")?;
    if !pipeline.is_array() {
        anyhow::bail!("saved aggregation pipeline must be a JSON array");
    }
    serde_json::to_string_pretty(&pipeline).context("unable to serialize aggregation template")
}

pub(crate) fn parse_aggregation_payload_input(
    contents: &str,
    template: &SavedAggregation,
) -> Result<SavedAggregation> {
    let pipeline: serde_json::Value =
        serde_json::from_str(contents).context("invalid JSON for saved aggregation")?;
    if !pipeline.is_array() {
        anyhow::bail!("saved aggregation payload must be a JSON array");
    }
    Ok(SavedAggregation {
        id: template.id.clone(),
        scope: template.scope.clone(),
        pipeline: serde_json::to_string(&pipeline).context("unable to serialize pipeline JSON")?,
    })
}

pub(crate) fn json_field_as_string(
    object: &serde_json::Map<String, serde_json::Value>,
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

pub(crate) fn saved_scope_label(scope: &SavedScope) -> String {
    match scope {
        SavedScope::Shared => "shared".to_string(),
        SavedScope::Scoped {
            database,
            collection,
        } => format!("{database}.{collection}"),
    }
}

pub(crate) fn default_saved_id(kind: &str, scope: &SavedScope) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let name = format!("{kind}_{millis}");
    match scope {
        SavedScope::Shared => name,
        SavedScope::Scoped {
            database,
            collection,
        } => format!("{database}.{collection}.{name}"),
    }
}
