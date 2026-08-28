use figma_dev_mcp_protocol::TOOL_NAMES;
use figma_dev_mcp_tools::{CACHE_TTL_MS, GetScreenshotInput, tools_catalog};
use rmcp::model::CacheScope;
use serde_json::Value;
use serde_json::json;

#[test]
fn tools_catalog_is_complete_sorted_read_only_and_cacheable() {
    let result = tools_catalog();
    let names: Vec<_> = result.tools.iter().map(|tool| tool.name.as_ref()).collect();

    assert_eq!(names, TOOL_NAMES);
    assert_eq!(result.ttl_ms, Some(CACHE_TTL_MS));
    assert_eq!(result.cache_scope, Some(CacheScope::Public));

    for tool in &result.tools {
        let annotations = tool.annotations.as_ref().expect("annotations are required");
        assert_eq!(annotations.read_only_hint, Some(true), "{}", tool.name);
        assert_eq!(annotations.destructive_hint, Some(false), "{}", tool.name);
        assert_eq!(annotations.open_world_hint, Some(false), "{}", tool.name);

        let input = Value::Object((*tool.input_schema).clone());
        assert_closed_object_schema(&input, tool.name.as_ref(), "input");

        let output = Value::Object(
            tool.output_schema
                .as_ref()
                .expect("output schema is required")
                .as_ref()
                .clone(),
        );
        assert_object_schema(&output, tool.name.as_ref(), "output");
    }
}

#[test]
fn screenshot_schema_accepts_each_format_without_top_level_property_blocking() {
    let schema = tools_catalog()
        .tools
        .into_iter()
        .find(|tool| tool.name == "get_screenshot")
        .expect("screenshot tool must be cataloged")
        .input_schema;
    // The formats are merged into one flat object, so closing the root closes
    // the schema without hiding any format's own properties.
    assert_eq!(
        schema.get("additionalProperties"),
        Some(&Value::Bool(false))
    );

    let validator = jsonschema::validator_for(&Value::Object((*schema).clone()))
        .expect("screenshot schema must be a valid JSON Schema");
    for input in [
        json!({"format": "png", "selector": {"nodeId": "1:2"}}),
        json!({"format": "jpeg", "selector": {"selection": true}, "scale": 2.0}),
        json!({"format": "svg", "selector": {"nodeIds": ["1:2"]}}),
    ] {
        assert!(validator.is_valid(&input), "schema rejected {input}");
    }
    assert!(
        !validator.is_valid(&json!({
            "format": "png",
            "selector": {"nodeId": "1:2"},
            "unknown": true
        })),
        "a closed root must still reject an unknown field"
    );

    // Merging the variants costs the schema its per-format pairing: `scale` is
    // a raster field, but a flat object cannot say so. The contract type is now
    // the only thing that enforces it, so that is pinned here alongside the
    // widening it pays for.
    let crossed = json!({"format": "svg", "selector": {"nodeId": "1:2"}, "scale": 2.0});
    assert!(
        validator.is_valid(&crossed),
        "the flattened schema deliberately no longer carries the pairing"
    );
    assert!(
        serde_json::from_value::<GetScreenshotInput>(crossed).is_err(),
        "a raster field on an svg request must still be rejected"
    );
}

fn assert_closed_object_schema(schema: &Value, tool: &str, direction: &str) {
    assert_object_schema(schema, tool, direction);
    assert_eq!(
        schema.get("additionalProperties"),
        Some(&Value::Bool(false)),
        "{tool} {direction} schema must be closed"
    );
    // The Anthropic API rejects a root combinator outright, and Claude Code
    // then drops the tool from the catalog it offers the model. Branches must
    // be merged into one flat object instead.
    for keyword in ["oneOf", "anyOf", "allOf"] {
        assert!(
            schema.get(keyword).is_none(),
            "{tool} {direction} schema must not put {keyword} at the root"
        );
    }
}

fn assert_object_schema(schema: &Value, tool: &str, direction: &str) {
    assert_eq!(
        schema.get("type").and_then(Value::as_str),
        Some("object"),
        "{tool} {direction} schema must be an object"
    );
}

#[test]
fn schema_snapshots_are_stable() {
    let catalog = tools_catalog();
    let tools = serde_json::to_value(&catalog.tools).unwrap();
    let inputs = Value::Object(
        catalog
            .tools
            .iter()
            .map(|tool| {
                (
                    tool.name.to_string(),
                    Value::Object((*tool.input_schema).clone()),
                )
            })
            .collect(),
    );
    let outputs = Value::Object(
        catalog
            .tools
            .iter()
            .map(|tool| {
                (
                    tool.name.to_string(),
                    Value::Object(tool.output_schema.as_ref().unwrap().as_ref().clone()),
                )
            })
            .collect(),
    );

    let tools = canonical(tools);
    let inputs = canonical(inputs);
    let outputs = canonical(outputs);
    if std::env::var("UPDATE_SNAPSHOTS").as_deref() == Ok("1") {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("contracts/snapshots");
        std::fs::write(
            dir.join("tools.json"),
            serde_json::to_string_pretty(&tools).unwrap() + "\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("input-schemas.json"),
            serde_json::to_string_pretty(&inputs).unwrap() + "\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("output-schemas.json"),
            serde_json::to_string_pretty(&outputs).unwrap() + "\n",
        )
        .unwrap();
        return;
    }
    assert_eq!(tools, snapshot("snapshots/tools.json"));
    assert_eq!(inputs, snapshot("snapshots/input-schemas.json"));
    assert_eq!(outputs, snapshot("snapshots/output-schemas.json"));
}

fn snapshot(path: &str) -> Value {
    serde_json::from_str(match path {
        "snapshots/tools.json" => include_str!("snapshots/tools.json"),
        "snapshots/input-schemas.json" => include_str!("snapshots/input-schemas.json"),
        "snapshots/output-schemas.json" => include_str!("snapshots/output-schemas.json"),
        _ => unreachable!(),
    })
    .unwrap()
}

fn canonical(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect(),
        ),
        scalar => scalar,
    }
}

#[test]
fn structured_image_content_preserves_the_compatibility_text_block() {
    let structured = json!({"asset": {"id": "1:2", "format": "png"}});
    let result = figma_dev_mcp_tools::structured_with_image(
        structured.clone(),
        "iVBORw==",
        "image/png",
        &figma_dev_mcp_tools::EnvelopeContext {
            request_id: rmcp::model::RequestId::Number(0),
            protocol_version: rmcp::model::ProtocolVersion::V_2026_07_28,
        },
    )
    .unwrap();

    assert_eq!(result.structured_content, Some(structured.clone()));
    assert_eq!(
        result.content[0].as_text().unwrap().text,
        structured.to_string()
    );
    assert_eq!(result.content[1].as_image().unwrap().data, "iVBORw==");
}
