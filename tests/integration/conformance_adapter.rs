//! Evidence split for official lifecycle smoke versus the production stdio product.
//!
//! Upstream server conformance accepts `--url`, so the two selected official
//! scenarios verify lifecycle/handler behavior through the test-only HTTP
//! adapter. `stdio_eras.rs` and `all_tools.rs` verify the actual production
//! transport, exact product catalog, and Figma-facing behavior. The adapter
//! must not add fixture-only tools, prompts, resources, or capabilities;
//! otherwise a green run would describe the adapter rather than the product.

use std::{fs, path::PathBuf};

use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits};
use figma_dev_mcp_prompts::resource_uri;
use figma_dev_mcp_protocol::{PROMPT_NAMES, TOOL_NAMES};
use figma_dev_mcp_tools::{McpService, tools_catalog};
use rmcp::ServiceExt;
use rmcp::model::{
    ClientCapabilities, GetPromptRequestParams, Implementation, ProtocolVersion,
    ReadResourceRequestParams, RequestMetaObject,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("tests crate sits in the workspace")
        .to_path_buf()
}

fn read(path: &str) -> String {
    fs::read_to_string(workspace_root().join(path)).unwrap_or_else(|error| {
        panic!(
            "{} must exist: {error}",
            workspace_root().join(path).display()
        )
    })
}

#[test]
fn production_crate_does_not_enable_http_transport_features() {
    let cargo = read("crates/figma-dev-mcp/Cargo.toml");
    assert!(
        !cargo.contains("transport-streamable-http-server"),
        "production crate must not enable HTTP transport features"
    );
    assert!(
        !cargo.contains("transport-streamable-http-client"),
        "production crate must not enable HTTP client transport features"
    );
    let cli = read("crates/figma-dev-mcp/src/cli.rs");
    for forbidden in ["bind", "port", "http", "listen", "url"] {
        assert!(
            !cli.to_ascii_lowercase().contains(forbidden),
            "production CLI must not expose an HTTP option ({forbidden})"
        );
    }
    let runtime = read("crates/figma-dev-mcp/src/runtime.rs");
    assert!(runtime.contains("transport::stdio"));
    assert!(!runtime.contains("streamable_http"));
}

#[test]
fn tests_crate_isolates_http_features_and_the_adapter_binary() {
    let cargo = read("tests/Cargo.toml");
    assert!(
        cargo.contains("transport-streamable-http-server"),
        "HTTP server transport belongs only on the tests crate"
    );
    assert!(
        cargo.contains("\"client\"") || cargo.contains("features = [\"client\""),
        "rmcp client belongs on the tests crate"
    );
    let adapter = read("tests/src/bin/conformance-server.rs");
    assert!(
        adapter.contains("McpService::new"),
        "adapter must compose the exact production McpService"
    );
    assert!(
        adapter.contains("streamable_http_server") || adapter.contains("StreamableHttpService"),
        "adapter must use rmcp Streamable HTTP"
    );
    for forbidden in [
        "everything",
        "sample_tool",
        "conformance_only",
        "audio",
        "resources/",
        "enable_resources",
        "enable_completions",
    ] {
        assert!(
            !adapter.contains(forbidden),
            "adapter must not add fixture-only capabilities ({forbidden})"
        );
    }
}

#[test]
fn conformance_scripts_pin_the_two_lifecycle_scenarios_not_the_full_suite() {
    let package = read("conformance/package.json");
    assert!(package.contains("server-stateless"));
    assert!(package.contains("2026-07-28"));
    assert!(package.contains("server-initialize"));
    assert!(
        !package.contains("--suite all"),
        "do not replace the explicit scenarios with --suite all"
    );
    let script = read("scripts/run-conformance.sh");
    assert!(script.contains("conformance-server"));
    assert!(script.contains("bun run modern"));
    assert!(script.contains("bun run legacy"));
    assert!(script.contains("trap"));
    assert!(!script.contains("--suite all"));
    assert!(!script.contains("expected-fail"));
}

#[tokio::test]
async fn adapter_catalog_is_the_production_tools_prompts_and_resources_surface() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server =
        tokio::spawn(async move { McpService::new(broker).serve(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let discovery = client
        .discover(RequestMetaObject::with_client_context(
            ProtocolVersion::V_2026_07_28,
            Implementation::new("adapter-isolation", "0.1.0"),
            ClientCapabilities::default(),
        ))
        .await
        .unwrap();
    assert!(discovery.capabilities.tools.is_some());
    assert!(discovery.capabilities.prompts.is_some());
    assert!(discovery.capabilities.resources.is_some());
    assert!(discovery.capabilities.completions.is_none());

    let tools = client.list_tools(None).await.unwrap();
    let names: Vec<_> = tools.tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(names, TOOL_NAMES);
    assert_eq!(tools_catalog().tools.len(), 14);

    let prompts = client.list_prompts(None).await.unwrap();
    let prompt_names: Vec<_> = prompts
        .prompts
        .iter()
        .map(|prompt| prompt.name.as_str())
        .collect();
    assert_eq!(prompt_names, PROMPT_NAMES);
    for name in PROMPT_NAMES {
        client
            .get_prompt(GetPromptRequestParams::new(name))
            .await
            .unwrap();
    }

    let resources = client.list_resources(None).await.unwrap();
    let resource_uris: Vec<_> = resources
        .resources
        .iter()
        .map(|resource| resource.uri.clone())
        .collect();
    assert_eq!(
        resource_uris,
        PROMPT_NAMES
            .iter()
            .map(|name| resource_uri(name))
            .collect::<Vec<_>>(),
        "the resource surface is exactly the three strategy playbooks"
    );
    for uri in resource_uris {
        client
            .read_resource(ReadResourceRequestParams::new(uri))
            .await
            .unwrap();
    }

    drop(client);
    server.abort();
}
