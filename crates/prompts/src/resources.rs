//! The same strategy bodies the prompt catalog serves, addressed as resources.
//!
//! `prompts/get` is user-invoked: a client surfaces a prompt as a slash command
//! and a model cannot reach it on its own. Exposing each body at a stable URI
//! lets a model fetch the playbook itself. There is one body per strategy; this
//! module reads [`crate::prompt_definitions`] rather than keeping a second copy.

use crate::catalog::{CACHE_TTL_MS, prompt_by_name, prompt_definitions};
use rmcp::model::{
    CacheScope, ListResourcesResult, ReadResourceResult, Resource, ResourceContents,
};

/// URI prefix for the strategy playbooks. A resource URI is this prefix
/// followed by the prompt name, with nothing else appended.
pub const RESOURCE_URI_PREFIX: &str = "figma://strategy/";

/// The bodies are the Markdown files under `crates/prompts/bodies`.
pub const RESOURCE_MIME_TYPE: &str = "text/markdown";

/// The resource URI for a prompt name. The name is not validated here; an
/// unknown name simply fails to resolve in [`read_resource_result`].
pub fn resource_uri(name: &str) -> String {
    format!("{RESOURCE_URI_PREFIX}{name}")
}

pub fn resources_catalog() -> ListResourcesResult {
    let resources = prompt_definitions()
        .iter()
        .map(|prompt| {
            Resource::new(resource_uri(prompt.name), prompt.name)
                .with_description(prompt.description)
                .with_mime_type(RESOURCE_MIME_TYPE)
                .with_size(prompt.body.len() as u64)
        })
        .collect();
    ListResourcesResult::with_all_items(resources)
        .with_ttl_ms(CACHE_TTL_MS)
        .with_cache_scope(CacheScope::Public)
}

pub fn read_resource_result(uri: &str) -> Option<ReadResourceResult> {
    let prompt = uri
        .strip_prefix(RESOURCE_URI_PREFIX)
        .and_then(prompt_by_name)?;
    Some(
        ReadResourceResult::new(vec![
            ResourceContents::text(prompt.body, uri).with_mime_type(RESOURCE_MIME_TYPE),
        ])
        .with_ttl_ms(CACHE_TTL_MS)
        .with_cache_scope(CacheScope::Public),
    )
}
