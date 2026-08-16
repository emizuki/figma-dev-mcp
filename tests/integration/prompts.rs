use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits};
use figma_dev_mcp_protocol::PROMPT_NAMES;
use figma_dev_mcp_tools::McpService;
use rmcp::RoleServer;
use rmcp::ServiceExt;
use rmcp::model::{
    CacheScope, ClientCapabilities, ErrorCode, GetPromptRequestParams, Implementation,
    ProtocolVersion, RequestMetaObject, Role,
};
use rmcp::service::{RunningService, ServiceError};

struct PromptClient {
    client: RunningService<rmcp::RoleClient, ()>,
    _server: tokio::task::JoinHandle<RunningService<RoleServer, McpService>>,
}

async fn connected_client() -> PromptClient {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server =
        tokio::spawn(async move { McpService::new(broker).serve(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    PromptClient {
        client,
        _server: server,
    }
}

#[tokio::test]
async fn prompts_list_is_sorted_argumentless_and_publicly_cacheable() {
    let PromptClient { client, _server } = connected_client().await;
    let discovery = client
        .discover(RequestMetaObject::with_client_context(
            ProtocolVersion::V_2026_07_28,
            Implementation::new("integration-test", "0.1.0"),
            ClientCapabilities::default(),
        ))
        .await
        .unwrap();
    let prompts_capability = discovery
        .capabilities
        .prompts
        .expect("prompts capability must be advertised");
    assert_ne!(
        prompts_capability.list_changed,
        Some(true),
        "prompt catalog is static; do not advertise list_changed"
    );

    let listed = client.list_prompts(None).await.unwrap();
    let names: Vec<_> = listed
        .prompts
        .iter()
        .map(|prompt| prompt.name.as_str())
        .collect();
    assert_eq!(names, PROMPT_NAMES);
    assert_eq!(listed.ttl_ms, Some(86_400_000));
    assert_eq!(listed.cache_scope, Some(CacheScope::Public));

    for prompt in &listed.prompts {
        assert!(
            prompt.arguments.as_ref().is_none_or(|args| args.is_empty()),
            "{} must accept no arguments",
            prompt.name
        );
        assert!(
            prompt
                .description
                .as_ref()
                .is_some_and(|description| !description.is_empty()),
            "{} must have a stable description",
            prompt.name
        );
    }
}

#[tokio::test]
async fn prompts_get_returns_one_user_text_message_and_rejects_unknown_names() {
    let PromptClient { client, _server } = connected_client().await;
    for name in PROMPT_NAMES {
        let result = client
            .get_prompt(GetPromptRequestParams::new(name))
            .await
            .unwrap_or_else(|error| panic!("{name} must resolve: {error}"));
        assert_eq!(result.messages.len(), 1, "{name}");
        assert_eq!(result.messages[0].role, Role::User, "{name}");
        let body = &result.messages[0]
            .content
            .as_text()
            .unwrap_or_else(|| panic!("{name} must be text"))
            .text;
        assert!(!body.trim().is_empty(), "{name} body must be non-empty");
        match name {
            "read_design_strategy" => assert_read_design_strategy(body),
            "prototype_flow_strategy" => assert_prototype_flow_strategy(body),
            "style_audit_strategy" => assert_style_audit_strategy(body),
            other => panic!("unexpected prompt {other}"),
        }
    }

    let error = client
        .get_prompt(GetPromptRequestParams::new("design_strategy"))
        .await
        .expect_err("unknown prompt must be a protocol error");
    match error {
        ServiceError::McpError(error) => {
            assert_eq!(error.code, ErrorCode::INVALID_PARAMS);
            let message = error.message.to_ascii_lowercase();
            assert!(
                message.contains("not found") || message.contains("unknown prompt"),
                "prompt-not-found message: {}",
                error.message
            );
        }
        other => panic!("expected MCP invalid_params, got {other:?}"),
    }
}

fn assert_read_design_strategy(body: &str) {
    for tool in [
        "`list_files`",
        "`get_metadata`",
        "`get_design_context`",
        "`search_nodes`",
        "`get_nodes`",
        "`get_styles`",
        "`get_variables`",
        "`get_components`",
        "`get_fonts`",
        "`get_dev_mode_data`",
        "`get_reactions`",
        "`get_screenshot`",
    ] {
        assert!(body.contains(tool), "read_design_strategy missing {tool}");
    }
    for detail in ["minimal", "compact", "full"] {
        assert!(
            body.contains(detail),
            "read_design_strategy must explain {detail}"
        );
    }
    assert!(
        body.contains("dedupeComponents") || body.contains("deduplication"),
        "read_design_strategy must explain component deduplication"
    );
    assert!(
        !body.to_ascii_lowercase().contains("whole document")
            && !body.to_ascii_lowercase().contains("entire document"),
        "read_design_strategy must not instruct whole-document traversal"
    );
}

fn assert_prototype_flow_strategy(body: &str) {
    for tool in [
        "`get_reactions`",
        "`get_motion`",
        "`get_nodes`",
        "`get_design_context`",
        "`get_screenshot`",
    ] {
        assert!(
            body.contains(tool),
            "prototype_flow_strategy missing {tool}"
        );
    }
    assert!(
        body.contains("CAPABILITY_UNAVAILABLE"),
        "prototype_flow_strategy must continue after CAPABILITY_UNAVAILABLE"
    );
    assert!(
        body.to_ascii_lowercase().contains("second"),
        "prototype_flow_strategy must treat motion times as seconds"
    );
    assert!(
        !body.contains("durationMs"),
        "prototype_flow_strategy must not tell clients to expect durationMs"
    );
    for required in [
        "journey",
        "source",
        "trigger",
        "action",
        "destination",
        "Mermaid",
        "unresolved",
        "dangling",
        "non-navigation",
        "truncat",
    ] {
        assert!(
            body.to_ascii_lowercase()
                .contains(&required.to_ascii_lowercase()),
            "prototype_flow_strategy missing {required}"
        );
    }
}

fn assert_style_audit_strategy(body: &str) {
    for tool in [
        "`get_selection`",
        "`get_styles`",
        "`get_variables`",
        "`get_design_context`",
    ] {
        assert!(body.contains(tool), "style_audit_strategy missing {tool}");
    }
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("empty") && lower.contains("scope"),
        "style_audit_strategy must ask for scope when the selection is empty"
    );
    assert!(
        lower.contains("never widen") || lower.contains("do not widen"),
        "style_audit_strategy must not widen an empty selection"
    );
    for required in ["node", "category", "confidence", "gap"] {
        assert!(
            lower.contains(required),
            "style_audit_strategy missing {required}"
        );
    }
    assert!(
        lower.contains("designer"),
        "style_audit_strategy recommendations must be for a designer"
    );
}
