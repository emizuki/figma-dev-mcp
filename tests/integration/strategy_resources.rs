//! The strategy playbooks served over `resources/list` and `resources/read`.
//!
//! `prompts/get` is user-invoked: a client surfaces it as a slash command and
//! the model cannot reach it on its own. The same three bodies are therefore
//! also addressable as resources, which a model can fetch itself.

use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits};
use figma_dev_mcp_prompts::{RESOURCE_MIME_TYPE, RESOURCE_URI_PREFIX};
use figma_dev_mcp_protocol::PROMPT_NAMES;
use figma_dev_mcp_tools::McpService;
use rmcp::RoleServer;
use rmcp::ServiceExt;
use rmcp::model::{
    CacheScope, ClientCapabilities, ErrorCode, GetPromptRequestParams, Implementation,
    ProtocolVersion, ReadResourceRequestParams, RequestMetaObject, ResourceContents,
};
use rmcp::service::{RunningService, ServiceError};

struct ResourceClient {
    client: RunningService<rmcp::RoleClient, ()>,
    _server: tokio::task::JoinHandle<RunningService<RoleServer, McpService>>,
}

async fn connected_client() -> ResourceClient {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server =
        tokio::spawn(async move { McpService::new(broker).serve(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    ResourceClient {
        client,
        _server: server,
    }
}

fn expected_uris() -> Vec<String> {
    PROMPT_NAMES
        .iter()
        .map(|name| format!("{RESOURCE_URI_PREFIX}{name}"))
        .collect()
}

#[tokio::test]
async fn resources_list_is_the_three_strategies_and_is_publicly_cacheable() {
    let ResourceClient { client, _server } = connected_client().await;
    let discovery = client
        .discover(RequestMetaObject::with_client_context(
            ProtocolVersion::V_2026_07_28,
            Implementation::new("integration-test", "0.1.0"),
            ClientCapabilities::default(),
        ))
        .await
        .unwrap();
    let resources_capability = discovery
        .capabilities
        .resources
        .expect("resources capability must be advertised");
    assert_ne!(
        resources_capability.list_changed,
        Some(true),
        "the strategy catalog is static; do not advertise list_changed"
    );
    assert_ne!(
        resources_capability.subscribe,
        Some(true),
        "the strategy catalog is static; do not advertise subscribe"
    );

    let listed = client.list_resources(None).await.unwrap();
    let uris: Vec<_> = listed
        .resources
        .iter()
        .map(|resource| resource.uri.clone())
        .collect();
    assert_eq!(uris, expected_uris());
    assert_eq!(listed.ttl_ms, Some(86_400_000));
    assert_eq!(listed.cache_scope, Some(CacheScope::Public));

    for resource in &listed.resources {
        assert!(
            PROMPT_NAMES.contains(&resource.name.as_str()),
            "{} must be named after its prompt",
            resource.uri
        );
        assert_eq!(
            resource.mime_type.as_deref(),
            Some(RESOURCE_MIME_TYPE),
            "{}",
            resource.uri
        );
        assert!(
            resource
                .description
                .as_ref()
                .is_some_and(|description| !description.is_empty()),
            "{} must carry a description",
            resource.uri
        );
    }
}

#[tokio::test]
async fn resources_read_serves_the_same_text_as_prompts_get() {
    let ResourceClient { client, _server } = connected_client().await;
    for name in PROMPT_NAMES {
        let uri = format!("{RESOURCE_URI_PREFIX}{name}");
        let read = client
            .read_resource(ReadResourceRequestParams::new(uri.clone()))
            .await
            .unwrap_or_else(|error| panic!("{uri} must resolve: {error}"));
        assert_eq!(read.contents.len(), 1, "{uri}");
        let ResourceContents::TextResourceContents {
            uri: content_uri,
            mime_type,
            text,
            ..
        } = &read.contents[0]
        else {
            panic!("{uri} must be text, not a blob");
        };
        assert_eq!(content_uri, &uri);
        assert_eq!(mime_type.as_deref(), Some(RESOURCE_MIME_TYPE));

        let prompt = client
            .get_prompt(GetPromptRequestParams::new(name))
            .await
            .unwrap_or_else(|error| panic!("{name} must resolve: {error}"));
        let body = &prompt.messages[0]
            .content
            .as_text()
            .unwrap_or_else(|| panic!("{name} must be text"))
            .text;
        assert_eq!(
            text, body,
            "{uri} and prompt {name} must serve one body, not two copies"
        );
    }
}

#[tokio::test]
async fn resources_read_reports_unknown_uris_as_invalid_params() {
    let ResourceClient { client, _server } = connected_client().await;
    for uri in [
        "figma://strategy/design_strategy",
        "figma://tool/get_metadata",
        "file:///etc/passwd",
    ] {
        let error = client
            .read_resource(ReadResourceRequestParams::new(uri))
            .await
            .expect_err("unknown resource must be a protocol error");
        match error {
            ServiceError::McpError(error) => {
                assert_eq!(error.code, ErrorCode::INVALID_PARAMS, "{uri}");
                let message = error.message.to_ascii_lowercase();
                assert!(
                    message.contains("not found") || message.contains("unknown resource"),
                    "resource-not-found message for {uri}: {}",
                    error.message
                );
            }
            other => panic!("expected MCP invalid_params for {uri}, got {other:?}"),
        }
    }
}
