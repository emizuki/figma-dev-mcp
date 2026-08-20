use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits, PLUGIN_PROTOCOL_VERSION};
use figma_dev_mcp_tools::McpService;
use futures_util::{SinkExt, StreamExt};
use rmcp::ServiceExt;
use serde_json::{Value, json};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async, tungstenite::Message, tungstenite::client::IntoClientRequest,
};

async fn running_broker() -> (SocketAddr, Broker, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let server = broker.clone();
    let task = tokio::spawn(async move { server.serve(listener).await.unwrap() });
    (address, broker, task)
}

fn hello(connection_id: &str) -> Message {
    Message::Text(
        json!({
            "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION, "connectionId": connection_id,
            "displayName": "Checkout flow", "fileName": "Checkout flow",
            "currentPage": {"id": "0:2", "name": "Checkout"},
            "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
        })
        .to_string()
        .into(),
    )
}

#[tokio::test]
async fn get_metadata_round_trips_through_server_broker_and_plugin() {
    let (address, broker, broker_task) = running_broker().await;
    let mut request = format!("ws://{address}/").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "null".parse().unwrap());
    let (mut plugin, _) = connect_async(request).await.unwrap();
    plugin
        .send(hello("123e4567-e89b-42d3-a456-426614174000"))
        .await
        .unwrap();
    for _ in 0..20 {
        if broker.live_file_count().await == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }

    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server_task = tokio::spawn(async move {
        McpService::new(broker.clone())
            .serve(server_io)
            .await
            .unwrap()
    });
    let client = ().serve(client_io).await.unwrap();

    let discovery = client
        .discover(rmcp::model::RequestMetaObject::with_client_context(
            rmcp::model::ProtocolVersion::V_2026_07_28,
            rmcp::model::Implementation::new("integration-test", "0.1.0"),
            rmcp::model::ClientCapabilities::default(),
        ))
        .await
        .unwrap();
    assert_eq!(discovery.ttl_ms, 86_400_000);
    assert_eq!(discovery.cache_scope, rmcp::model::CacheScope::Public);
    assert_eq!(
        discovery
            .supported_versions
            .iter()
            .map(rmcp::model::ProtocolVersion::as_str)
            .collect::<Vec<_>>(),
        ["2025-11-25", "2026-07-28"]
    );

    let files = client
        .call_tool(rmcp::model::CallToolRequestParams::new("list_files"))
        .await
        .unwrap();
    let first_file = &files.structured_content.as_ref().unwrap()["files"][0];
    assert_eq!(first_file["fileName"], "Checkout flow");
    assert_eq!(first_file["currentPage"]["id"], "0:2");
    assert!(first_file["connectedAt"].as_str().is_some());
    assert!(first_file["lastSeenAt"].as_str().is_some());

    assert!(
        client
            .call_tool(rmcp::model::CallToolRequestParams::new("unknown_tool"))
            .await
            .is_err()
    );
    assert!(
        client
            .call_tool(
                rmcp::model::CallToolRequestParams::new("get_metadata").with_arguments(
                    serde_json::Map::from_iter([("unexpected".to_owned(), json!(true))]),
                ),
            )
            .await
            .is_err()
    );
    let missing = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_metadata").with_arguments(
                serde_json::Map::from_iter([(
                    "connectionId".to_owned(),
                    json!("123e4567-e89b-42d3-a456-426614174099"),
                )]),
            ),
        )
        .await
        .unwrap();
    assert_eq!(missing.is_error, Some(true));
    assert_eq!(
        missing.structured_content.as_ref().unwrap()["code"],
        "CONNECTION_NOT_FOUND"
    );

    let plugin_task = tokio::spawn(async move {
        let request = loop {
            let Some(Ok(Message::Text(frame))) = plugin.next().await else {
                panic!("plugin did not receive request")
            };
            let request: Value = serde_json::from_str(&frame).unwrap();
            if request["type"] == "request" {
                break request;
            }
        };
        assert_eq!(request["type"], "request");
        assert_eq!(request["operation"]["operation"], "get_metadata");
        let request_id = request["requestId"].as_str().unwrap();
        plugin
            .send(Message::Text(
                json!({
                    "type": "response", "requestId": request_id,
                    "result": {
                        "operation": "get_metadata",
                        "result": {
                            "file": {"name": "Checkout flow", "editorType": "dev"},
                            "pages": [{"id": "0:1", "name": "Home"}, {"id": "0:2", "name": "Checkout"}],
                            "currentPageId": "0:2", "capabilities": {}, "truncated": false,
                            "pluginVersion": "0.1.0",
                            "observation": {"startedAt": "2026-08-16T10:00:00.000Z", "completedAt": "2026-08-16T10:00:00.001Z"}
                        }
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
    });

    let result = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_metadata").with_arguments(
                serde_json::Map::from_iter([(
                    "connectionId".to_owned(),
                    json!("123e4567-e89b-42d3-a456-426614174000"),
                )]),
            ),
        )
        .await
        .unwrap();
    let structured = result.structured_content.clone().unwrap();
    assert_eq!(structured["file"]["name"], "Checkout flow");
    assert_eq!(structured["pluginVersion"], "0.1.0");
    assert_eq!(result.content.len(), 1);
    assert_eq!(
        result.content[0].as_text().unwrap().text,
        structured.to_string()
    );

    plugin_task.await.unwrap();
    server_task.abort();
    broker_task.abort();
}
