use figma_dev_mcp_prompts::{
    CACHE_TTL_MS, RESOURCE_MIME_TYPE, RESOURCE_URI_PREFIX, prompt_by_name, prompt_definitions,
    read_resource_result, resource_uri, resources_catalog,
};
use figma_dev_mcp_protocol::PROMPT_NAMES;
use rmcp::model::{CacheScope, ResourceContents};

fn text_contents(contents: &ResourceContents) -> (&str, Option<&str>, &str) {
    match contents {
        ResourceContents::TextResourceContents {
            uri,
            mime_type,
            text,
            ..
        } => (uri.as_str(), mime_type.as_deref(), text.as_str()),
        other => panic!("strategy resources are text, got {other:?}"),
    }
}

#[test]
fn resource_catalog_is_one_prefixed_uri_per_prompt_and_publicly_cacheable() {
    let result = resources_catalog();
    let uris: Vec<_> = result
        .resources
        .iter()
        .map(|resource| resource.uri.as_str())
        .collect();
    let expected: Vec<String> = PROMPT_NAMES
        .iter()
        .map(|name| format!("{RESOURCE_URI_PREFIX}{name}"))
        .collect();
    assert_eq!(uris, expected);
    assert_eq!(result.ttl_ms, Some(CACHE_TTL_MS));
    assert_eq!(result.cache_scope, Some(CacheScope::Public));

    for resource in &result.resources {
        let prompt = prompt_by_name(&resource.name)
            .unwrap_or_else(|| panic!("{} must name a prompt", resource.name));
        assert_eq!(resource.uri, resource_uri(prompt.name));
        assert_eq!(resource.description.as_deref(), Some(prompt.description));
        assert_eq!(resource.mime_type.as_deref(), Some(RESOURCE_MIME_TYPE));
        assert_eq!(resource.size, Some(prompt.body.len() as u64));
    }
}

#[test]
fn reading_a_strategy_resource_returns_the_prompt_body_verbatim() {
    for prompt in prompt_definitions() {
        let uri = resource_uri(prompt.name);
        let result = read_resource_result(&uri)
            .unwrap_or_else(|| panic!("{uri} must resolve to the {} body", prompt.name));
        assert_eq!(result.contents.len(), 1, "{uri}");
        let (content_uri, mime_type, text) = text_contents(&result.contents[0]);
        assert_eq!(content_uri, uri);
        assert_eq!(mime_type, Some(RESOURCE_MIME_TYPE));
        assert_eq!(text, prompt.body);
        assert_eq!(result.ttl_ms, Some(CACHE_TTL_MS));
        assert_eq!(result.cache_scope, Some(CacheScope::Public));
    }
}

#[test]
fn uris_outside_the_strategy_prefix_do_not_resolve() {
    for uri in [
        "figma://strategy/design_strategy",
        "figma://strategy/",
        "figma://strategy/read_design_strategy/extra",
        "read_design_strategy",
        "file:///etc/passwd",
        "figma://tool/get_metadata",
    ] {
        assert!(
            read_resource_result(uri).is_none(),
            "{uri} must not resolve"
        );
    }
}
