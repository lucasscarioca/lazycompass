use super::*;
pub(crate) fn render_inline_query_template() -> Result<String> {
    serde_json::to_string_pretty(&serde_json::json!({
        "filter": {},
        "sort": {
            "_id": -1
        },
        "limit": 20
    }))
    .context("unable to serialize inline query template")
}

pub(crate) fn parse_inline_query_payload(contents: &str) -> Result<InlineQueryPayload> {
    let value: serde_json::Value =
        serde_json::from_str(contents).context("invalid JSON for inline query")?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("inline query payload must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "filter" | "projection" | "sort" | "limit") {
            anyhow::bail!("unknown field '{key}' in inline query payload");
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
    Ok(InlineQueryPayload {
        filter,
        projection,
        sort,
        limit,
    })
}

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

pub(crate) fn render_query_save_template(id: &str, draft: &InlineQueryPayload) -> Result<String> {
    let mut object = serde_json::Map::new();
    object.insert("id".to_string(), serde_json::Value::String(id.to_string()));
    if let Some(filter) = draft.filter.as_deref() {
        object.insert(
            "filter".to_string(),
            serde_json::from_str(filter).context("inline query filter must be valid JSON")?,
        );
    }
    if let Some(projection) = draft.projection.as_deref() {
        object.insert(
            "projection".to_string(),
            serde_json::from_str(projection)
                .context("inline query projection must be valid JSON")?,
        );
    }
    if let Some(sort) = draft.sort.as_deref() {
        object.insert(
            "sort".to_string(),
            serde_json::from_str(sort).context("inline query sort must be valid JSON")?,
        );
    }
    if let Some(limit) = draft.limit {
        object.insert("limit".to_string(), serde_json::Value::from(limit));
    }
    serde_json::to_string_pretty(&serde_json::Value::Object(object))
        .context("unable to serialize query save template")
}

pub(crate) fn parse_query_save_input(contents: &str, scope: SavedScope) -> Result<SavedQuery> {
    let value: serde_json::Value =
        serde_json::from_str(contents).context("invalid JSON for saved query")?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("saved query payload must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "id" | "filter" | "projection" | "sort" | "limit"
        ) {
            anyhow::bail!("unknown field '{key}' in saved query payload");
        }
    }

    let id = object
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("field 'id' must be a non-empty string"))?;
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
        id,
        scope,
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

pub(crate) fn render_inline_aggregation_template() -> Result<String> {
    serde_json::to_string_pretty(&serde_json::json!([
        { "$match": {} },
        { "$limit": 20 }
    ]))
    .context("unable to serialize inline aggregation template")
}

pub(crate) fn parse_inline_aggregation_payload(contents: &str) -> Result<InlineAggregationPayload> {
    let pipeline: serde_json::Value =
        serde_json::from_str(contents).context("invalid JSON for inline aggregation")?;
    if !pipeline.is_array() {
        anyhow::bail!("inline aggregation payload must be a JSON array");
    }
    Ok(InlineAggregationPayload {
        pipeline: serde_json::to_string(&pipeline).context("unable to serialize pipeline JSON")?,
    })
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

pub(crate) fn render_aggregation_save_template(
    id: &str,
    draft: &InlineAggregationPayload,
) -> Result<String> {
    let pipeline: serde_json::Value = serde_json::from_str(&draft.pipeline)
        .context("inline aggregation pipeline must be valid JSON")?;
    if !pipeline.is_array() {
        anyhow::bail!("inline aggregation pipeline must be a JSON array");
    }
    serde_json::to_string_pretty(&serde_json::json!({
        "id": id,
        "pipeline": pipeline,
    }))
    .context("unable to serialize aggregation save template")
}

pub(crate) fn parse_aggregation_save_input(
    contents: &str,
    scope: SavedScope,
) -> Result<SavedAggregation> {
    let value: serde_json::Value =
        serde_json::from_str(contents).context("invalid JSON for saved aggregation")?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("saved aggregation payload must be a JSON object"))?;
    for key in object.keys() {
        if !matches!(key.as_str(), "id" | "pipeline") {
            anyhow::bail!("unknown field '{key}' in saved aggregation payload");
        }
    }

    let id = object
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("field 'id' must be a non-empty string"))?;
    let pipeline = object
        .get("pipeline")
        .ok_or_else(|| anyhow::anyhow!("field 'pipeline' is required"))?;
    if !pipeline.is_array() {
        anyhow::bail!("field 'pipeline' must be a JSON array");
    }

    Ok(SavedAggregation {
        id,
        scope,
        pipeline: serde_json::to_string(pipeline).context("unable to serialize pipeline JSON")?,
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

#[cfg(test)]
mod tests {
    use super::{
        parse_aggregation_payload_input, parse_aggregation_save_input,
        parse_inline_aggregation_payload, parse_inline_query_payload, parse_query_payload_input,
        parse_query_save_input, render_inline_aggregation_template, render_inline_query_template,
        render_query_payload_template,
    };
    use lazycompass_core::{SavedAggregation, SavedQuery, SavedScope};

    fn query_template() -> SavedQuery {
        SavedQuery {
            id: "shared_query".to_string(),
            scope: SavedScope::Shared,
            filter: None,
            projection: None,
            sort: None,
            limit: None,
        }
    }

    #[test]
    fn parse_query_payload_rejects_unknown_fields() {
        let err = parse_query_payload_input(r#"{ "unknown": 1 }"#, &query_template())
            .expect_err("expected unknown field error");
        assert!(err.to_string().contains("unknown field 'unknown'"));
    }

    #[test]
    fn parse_query_payload_validates_limit_type() {
        let err = parse_query_payload_input(r#"{ "limit": "ten" }"#, &query_template())
            .expect_err("expected invalid limit");
        assert!(
            err.to_string()
                .contains("field 'limit' must be a non-negative integer")
        );
    }

    #[test]
    fn parse_aggregation_payload_requires_array() {
        let template = SavedAggregation {
            id: "agg".to_string(),
            scope: SavedScope::Shared,
            pipeline: "[]".to_string(),
        };
        let err = parse_aggregation_payload_input(r#"{ "x": 1 }"#, &template)
            .expect_err("expected array payload");
        assert!(
            err.to_string()
                .contains("saved aggregation payload must be a JSON array")
        );
    }

    #[test]
    fn render_query_payload_template_keeps_json_shapes() {
        let template = SavedQuery {
            id: "shape".to_string(),
            scope: SavedScope::Shared,
            filter: Some(r#"{"active":true}"#.to_string()),
            projection: Some(r#"{"email":1}"#.to_string()),
            sort: Some(r#"{"createdAt":-1}"#.to_string()),
            limit: Some(10),
        };
        let rendered = render_query_payload_template(&template).expect("render");
        assert!(rendered.contains("\"filter\""));
        assert!(rendered.contains("\"projection\""));
        assert!(rendered.contains("\"sort\""));
        assert!(rendered.contains("\"limit\""));
    }

    #[test]
    fn inline_query_template_is_valid() {
        let rendered = render_inline_query_template().expect("render");
        let parsed = parse_inline_query_payload(&rendered).expect("parse");
        assert_eq!(parsed.limit, Some(20));
        assert!(parsed.filter.is_some());
        assert!(parsed.sort.is_some());
    }

    #[test]
    fn inline_aggregation_template_is_valid() {
        let rendered = render_inline_aggregation_template().expect("render");
        let parsed = parse_inline_aggregation_payload(&rendered).expect("parse");
        assert!(parsed.pipeline.contains("$match"));
        assert!(parsed.pipeline.contains("$limit"));
    }

    #[test]
    fn parse_inline_query_payload_rejects_unknown_fields() {
        let err = parse_inline_query_payload(r#"{ "unknown": 1 }"#)
            .expect_err("expected unknown field error");
        assert!(err.to_string().contains("unknown field 'unknown'"));
    }

    #[test]
    fn parse_inline_query_payload_validates_limit_type() {
        let err = parse_inline_query_payload(r#"{ "limit": "ten" }"#)
            .expect_err("expected invalid limit");
        assert!(
            err.to_string()
                .contains("field 'limit' must be a non-negative integer")
        );
    }

    #[test]
    fn parse_inline_aggregation_payload_requires_array() {
        let err =
            parse_inline_aggregation_payload(r#"{ "x": 1 }"#).expect_err("expected array payload");
        assert!(
            err.to_string()
                .contains("inline aggregation payload must be a JSON array")
        );
    }

    #[test]
    fn parse_query_save_input_requires_id() {
        let err = parse_query_save_input(r#"{ "filter": {} }"#, SavedScope::Shared)
            .expect_err("expected missing id");
        assert!(
            err.to_string()
                .contains("field 'id' must be a non-empty string")
        );
    }

    #[test]
    fn parse_query_save_input_preserves_json_shapes() {
        let query = parse_query_save_input(
            r#"{
                "id": "shared_query",
                "filter": { "active": true },
                "projection": { "email": 1 },
                "sort": { "createdAt": -1 },
                "limit": 10
            }"#,
            SavedScope::Shared,
        )
        .expect("parse");
        assert_eq!(query.id, "shared_query");
        assert_eq!(query.limit, Some(10));
        assert_eq!(query.filter.as_deref(), Some(r#"{"active":true}"#));
        assert_eq!(query.projection.as_deref(), Some(r#"{"email":1}"#));
        assert_eq!(query.sort.as_deref(), Some(r#"{"createdAt":-1}"#));
    }

    #[test]
    fn parse_aggregation_save_input_requires_id() {
        let err = parse_aggregation_save_input(r#"{ "pipeline": [] }"#, SavedScope::Shared)
            .expect_err("expected missing id");
        assert!(
            err.to_string()
                .contains("field 'id' must be a non-empty string")
        );
    }

    #[test]
    fn parse_aggregation_save_input_requires_pipeline_array() {
        let err = parse_aggregation_save_input(
            r#"{ "id": "orders_by_user", "pipeline": {} }"#,
            SavedScope::Shared,
        )
        .expect_err("expected array");
        assert!(
            err.to_string()
                .contains("field 'pipeline' must be a JSON array")
        );
    }
}
