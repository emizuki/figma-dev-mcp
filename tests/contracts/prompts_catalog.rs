use figma_dev_mcp_prompts::{CACHE_TTL_MS, prompt_definitions, prompts_catalog};
use figma_dev_mcp_protocol::PROMPT_NAMES;
use rmcp::model::CacheScope;
use serde_json::Value;

#[test]
fn prompts_catalog_is_sorted_argumentless_and_publicly_cacheable() {
    let result = prompts_catalog();
    let names: Vec<_> = result
        .prompts
        .iter()
        .map(|prompt| prompt.name.as_str())
        .collect();
    assert_eq!(names, PROMPT_NAMES);
    assert_eq!(result.ttl_ms, Some(CACHE_TTL_MS));
    assert_eq!(result.cache_scope, Some(CacheScope::Public));
    for prompt in &result.prompts {
        assert!(prompt.arguments.is_none(), "{}", prompt.name);
        assert!(
            prompt
                .description
                .as_ref()
                .is_some_and(|description| !description.is_empty()),
            "{}",
            prompt.name
        );
    }
}

#[test]
fn prompt_snapshot_is_stable() {
    let catalog = prompts_catalog();
    let bodies = prompt_definitions()
        .iter()
        .map(|prompt| {
            (
                prompt.name.to_owned(),
                Value::String(prompt.body.to_owned()),
            )
        })
        .collect();
    let snapshot = Value::Object(
        [
            ("bodies".to_owned(), Value::Object(bodies)),
            (
                "catalog".to_owned(),
                serde_json::to_value(&catalog).unwrap(),
            ),
        ]
        .into_iter()
        .collect(),
    );
    assert_eq!(canonical(snapshot), expected_snapshot());
}

fn expected_snapshot() -> Value {
    serde_json::from_str(include_str!("snapshots/prompts.json")).unwrap()
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
