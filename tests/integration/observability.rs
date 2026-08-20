use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits, PLUGIN_PROTOCOL_VERSION};
use figma_dev_mcp_tools::McpService;
use futures_util::{SinkExt, StreamExt};
use rmcp::ServiceExt;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174000";
const SENTINEL_FILE_NAME: &str = "SENTINEL_FILE_NAME_CHECKOUT.fig";
const SENTINEL_NODE_TEXT: &str = "SENTINEL_NODE_TEXT_PAY_NOW";
const SENTINEL_VARIABLE_VALUE: &str = "SENTINEL_VARIABLE_VALUE_#FF00AA";
const SENTINEL_SVG_SOURCE: &str = "<svg id='SENTINEL_SVG_SOURCE'/>";
const SENTINEL_BASE64: &str = "SENTINEL_BASE64_iVBORw0KGgo=";

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("capture mutex").extend_from_slice(buf);
        std::io::stderr().write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::stderr().flush()
    }
}

fn install_stderr_subscriber() -> Captured {
    // SAFETY: this integration binary is the only reader of FIGMA_DEV_MCP_LOG.
    unsafe { std::env::set_var("FIGMA_DEV_MCP_LOG", "debug") }
    let filter = figma_dev_mcp::logging::filter_from_env();
    let rendered = filter.to_string();
    assert!(
        rendered.contains("rmcp=info"),
        "FIGMA_DEV_MCP_LOG=debug must pin rmcp: {rendered}"
    );
    let captured = Captured::default();
    let writer = captured.clone();
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(move || writer.clone())
                .with_ansi(false)
                .with_target(true),
        )
        .with(filter)
        .try_init();
    captured
}

#[tokio::test]
async fn tool_logs_are_schema_safe_and_stdout_stays_protocol_only() {
    let captured = install_stderr_subscriber();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(Limits::production()).unwrap());
    let server = broker.clone();
    let broker_task = tokio::spawn(async move { server.serve(listener).await.unwrap() });

    let mut request = format!("ws://{address}/").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "null".parse().unwrap());
    let (mut plugin, _) = connect_async(request).await.unwrap();
    plugin
        .send(Message::Text(
            json!({
                "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION, "connectionId": CONNECTION,
                "displayName": SENTINEL_FILE_NAME, "fileName": SENTINEL_FILE_NAME,
                "currentPage": {"id": "0:1", "name": SENTINEL_NODE_TEXT},
                "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
    for _ in 0..200 {
        if broker.live_file_count().await == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }

    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server_task =
        tokio::spawn(async move { McpService::new(broker).serve(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let plugin_task = tokio::spawn(async move {
        loop {
            let Some(Ok(Message::Text(frame))) = plugin.next().await else {
                break;
            };
            let request: serde_json::Value = serde_json::from_str(&frame).unwrap();
            match request["type"].as_str() {
                Some("ping") => {
                    plugin
                        .send(Message::Text(
                            json!({"type": "pong", "nonce": request["nonce"]})
                                .to_string()
                                .into(),
                        ))
                        .await
                        .unwrap();
                }
                Some("request") => {
                    let request_id = request["requestId"].as_str().unwrap();
                    plugin
                        .send(Message::Text(
                            json!({
                                "type": "response", "requestId": request_id,
                                "result": {
                                    "operation": "get_metadata",
                                    "result": {
                                        "file": {"name": SENTINEL_FILE_NAME, "editorType": "dev"},
                                        "pages": [{"id": "0:1", "name": SENTINEL_NODE_TEXT}],
                                        "currentPageId": "0:1", "capabilities": {},
                                        "truncated": false, "pluginVersion": "0.1.0",
                                        "observation": {
                                            "startedAt": SENTINEL_VARIABLE_VALUE,
                                            "completedAt": SENTINEL_SVG_SOURCE
                                        }
                                    }
                                }
                            })
                            .to_string()
                            .into(),
                        ))
                        .await
                        .unwrap();
                    break;
                }
                _ => {}
            }
        }
    });

    let result = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_metadata").with_arguments(
                serde_json::Map::from_iter([("connectionId".to_owned(), json!(CONNECTION))]),
            ),
        )
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(false));
    plugin_task.await.unwrap();

    let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    for required in [
        "request_id",
        "tool_name",
        "connection_id",
        "duration",
        "item_count",
        "byte",
        "error_code",
        "get_metadata",
        CONNECTION,
    ] {
        assert!(
            logs.contains(required),
            "stderr logs must include {required}: {logs}"
        );
    }
    for sentinel in [
        SENTINEL_FILE_NAME,
        SENTINEL_NODE_TEXT,
        SENTINEL_VARIABLE_VALUE,
        SENTINEL_SVG_SOURCE,
        SENTINEL_BASE64,
    ] {
        assert!(
            !logs.contains(sentinel),
            "stderr must not contain {sentinel}: {logs}"
        );
    }

    let logging = include_str!("../../crates/figma-dev-mcp/src/logging.rs");
    assert!(logging.contains("FIGMA_DEV_MCP_LOG"));
    assert!(logging.contains("std::io::stderr"));
    assert!(!logging.contains("stdout"));
    assert!(!logging.contains("telemetry"));
    let workspace = include_str!("../../Cargo.toml");
    assert!(!workspace.contains("opentelemetry"));
    assert!(!workspace.contains("sentry"));
    assert!(!workspace.contains("telemetry"));
    let tools_manifest = include_str!("../../crates/tools/Cargo.toml");
    assert!(!tools_manifest.contains("reqwest"));
    assert!(!tools_manifest.contains("telemetry"));

    server_task.abort();
    broker_task.abort();
    let _ = Duration::from_millis(1);
}
