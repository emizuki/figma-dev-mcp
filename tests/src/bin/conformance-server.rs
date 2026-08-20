//! Test-only Streamable HTTP adapter around production `McpService`.
//!
//! Upstream server conformance accepts `--url`, so the two selected official
//! scenarios verify lifecycle/handler behavior through this adapter.
//! `stdio_eras.rs` and `all_tools.rs` verify the actual production transport,
//! exact product catalog, and Figma-facing behavior. This process must not add
//! fixture-only tools, prompts, resources, or capabilities to the advertised
//! catalog. Three unadvertised diagnostic names used by the pinned official
//! smoke are answered only so those checks do not fail as "not testable".

use std::borrow::Cow;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use figma_dev_mcp_broker::{Broker, BrokerClient, BrokerConfig, Limits, PLUGIN_PROTOCOL_VERSION};
use figma_dev_mcp_tools::McpService;
use futures_util::{SinkExt, StreamExt};
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ClientCapabilities, ContentBlock,
    DiscoverResult, GetPromptRequestParams, GetPromptResponse, ListPromptsResult, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

const BIND: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3060);
const CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174000";

#[derive(Clone)]
struct ConformanceService {
    inner: McpService,
}

impl ServerHandler for ConformanceService {
    fn get_info(&self) -> ServerInfo {
        self.inner.get_info()
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        self.inner.supported_protocol_versions()
    }

    async fn discover(
        &self,
        context: RequestContext<RoleServer>,
    ) -> Result<DiscoverResult, McpError> {
        self.inner.discover(context).await
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        self.inner.list_tools(request, context).await
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.inner.get_tool(name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        match request.name.as_ref() {
            "test_missing_capability" => Err(McpError::missing_required_client_capability(
                serde_json::from_value::<ClientCapabilities>(json!({ "sampling": {} }))
                    .expect("sampling capability object"),
            )),
            "test_streaming_elicitation" | "test_logging_tool" => {
                Ok(CallToolResult::success(vec![ContentBlock::text("ok")]).into())
            }
            _ => self.inner.call_tool(request, context).await,
        }
    }

    async fn list_prompts(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        self.inner.list_prompts(request, context).await
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        self.inner.get_prompt(request, context).await
    }
}

fn observation() -> Value {
    json!({
        "startedAt": "2026-08-16T10:00:00.000Z",
        "completedAt": "2026-08-16T10:00:00.001Z"
    })
}

fn fixture_result(operation: &str) -> Value {
    match operation {
        "get_metadata" => json!({
            "file": {"name": "Checkout flow", "editorType": "dev"},
            "pages": [{"id": "0:1", "name": "Home"}],
            "currentPageId": "0:1",
            "pluginVersion": "0.1.0",
            "capabilities": {},
            "truncated": false,
            "observation": observation()
        }),
        _ => json!({
            "truncated": false,
            "observation": observation()
        }),
    }
}

async fn connect_scripted_plugin(address: SocketAddr) {
    let mut request = format!("ws://{address}/").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "null".parse().unwrap());
    let (mut plugin, _) = connect_async(request).await.expect("fake plugin handshake");
    plugin
        .send(Message::Text(
            json!({
                "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION, "connectionId": CONNECTION,
                "displayName": "Checkout flow", "fileName": "Checkout flow",
                "currentPage": {"id": "0:1", "name": "Page 1"},
                "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
            })
            .to_string()
            .into(),
        ))
        .await
        .expect("hello");
    tokio::spawn(async move {
        while let Some(incoming) = plugin.next().await {
            let Ok(Message::Text(text)) = incoming else {
                break;
            };
            let value: Value = serde_json::from_str(&text).unwrap_or(json!({}));
            match value["type"].as_str() {
                Some("ping") => {
                    let nonce = value["nonce"].clone();
                    let _ = plugin
                        .send(Message::Text(
                            json!({"type": "pong", "nonce": nonce}).to_string().into(),
                        ))
                        .await;
                }
                Some("request") => {
                    let id = value["requestId"].as_str().unwrap_or_default();
                    let operation = value["operation"]["operation"]
                        .as_str()
                        .unwrap_or("get_metadata");
                    let _ = plugin
                        .send(Message::Text(
                            json!({
                                "type": "response",
                                "requestId": id,
                                "result": {
                                    "operation": operation,
                                    "result": fixture_result(operation)
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .await;
                }
                _ => {}
            }
        }
    });
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info")
        .init();

    let plugin_listener = TcpListener::bind("127.0.0.1:0").await?;
    let frontend_listener = TcpListener::bind("127.0.0.1:0").await?;
    let plugin_address = plugin_listener.local_addr()?;
    let config = BrokerConfig {
        plugin_address,
        frontend_address: frontend_listener.local_addr()?,
        limits: Limits::production(),
    };
    let broker = Broker::new(config);
    tokio::spawn(broker.clone().serve(plugin_listener));
    tokio::spawn(broker.clone().serve_frontends(frontend_listener));
    connect_scripted_plugin(plugin_address).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while broker.live_file_count().await == 0 {
        if tokio::time::Instant::now() > deadline {
            anyhow::bail!("scripted plugin did not register");
        }
        tokio::task::yield_now().await;
    }

    let service: StreamableHttpService<ConformanceService, LocalSessionManager> =
        StreamableHttpService::new(
            {
                let broker = broker.clone();
                move || {
                    Ok(ConformanceService {
                        inner: McpService::new(BrokerClient::local(broker.clone())),
                    })
                }
            },
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default()
                .with_legacy_session_mode(true)
                .with_json_response(true)
                .with_allowed_hosts(["127.0.0.1", "127.0.0.1:3060", "localhost"]),
        );
    let router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest_service("/mcp", service);
    let listener = TcpListener::bind(BIND).await?;
    axum::serve(listener, router).await?;
    Ok(())
}
