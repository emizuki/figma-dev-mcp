//! Leader loss and restart do not replay in-flight plugin work.

use std::time::Duration;

use figma_dev_mcp_broker::{BrokerClient, BrokerConfig, ElectionOutcome, Limits, elect};
use figma_dev_mcp_protocol::error::ErrorCode;
use figma_dev_mcp_tools::McpService;
use futures_util::{SinkExt, StreamExt};
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use super::multi_client::connect_plugin;

const CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174040";

async fn next_plugin_request(
    plugin: &mut super::multi_client::PluginSocket,
) -> figma_dev_mcp_protocol::wire::Request {
    loop {
        let message = plugin.next().await.unwrap().unwrap();
        let Message::Text(text) = message else {
            continue;
        };
        if let figma_dev_mcp_protocol::wire::BrokerToPlugin::Request(request) =
            serde_json::from_str(&text).unwrap()
        {
            return request;
        }
    }
}

async fn send_metadata(plugin: &mut super::multi_client::PluginSocket, request_id: &str) {
    plugin
        .send(Message::Text(
            json!({
                "type": "response",
                "requestId": request_id,
                "result": {
                    "operation": "get_metadata",
                    "result": {
                        "file": {"name": "Restarted", "editorType": "dev"},
                        "pages": [{"id": "0:1", "name": "Page 1"}],
                        "currentPageId": "0:1",
                        "pluginVersion": "0.1.0",
                        "capabilities": {},
                        "truncated": false,
                        "observation": {
                            "startedAt": "2026-08-16T10:00:00.000Z",
                            "completedAt": "2026-08-16T10:00:00.001Z"
                        }
                    }
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .unwrap();
}

#[tokio::test]
async fn leader_restart_fails_inflight_work_and_accepts_a_fresh_session() {
    let plugin_reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = BrokerConfig {
        plugin_address: plugin_reservation.local_addr().unwrap(),
        frontend_address: frontend_reservation.local_addr().unwrap(),
        limits: Limits::reduced_for_test(),
    };
    drop(plugin_reservation);
    drop(frontend_reservation);

    let first = elect(config.clone()).await.unwrap();
    let ElectionOutcome::Leader(leader) = first else {
        panic!("first process must lead");
    };
    let broker = leader.broker.clone();
    let plugin_task = tokio::spawn(broker.clone().serve(leader.plugin_listener));
    let frontend_task = tokio::spawn(broker.clone().serve_frontends(leader.frontend_listener));
    let mut plugin = connect_plugin(config.plugin_address, CONNECTION, "Before restart").await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server_task = tokio::spawn({
        let broker = broker.clone();
        async move {
            McpService::new(BrokerClient::local(broker))
                .serve(server_io)
                .await
                .unwrap()
        }
    });
    let client = ().serve(client_io).await.unwrap();
    let inflight = tokio::spawn({
        let peer = client.peer().clone();
        async move {
            peer.call_tool(CallToolRequestParams::new("get_metadata").with_arguments(
                serde_json::Map::from_iter([("connectionId".to_owned(), json!(CONNECTION))]),
            ))
            .await
        }
    });
    let first_request = next_plugin_request(&mut plugin).await;

    broker.shutdown().await;
    let _ = plugin_task.await;
    let _ = frontend_task.await;
    drop(plugin);

    let lost = tokio::time::timeout(Duration::from_secs(2), inflight)
        .await
        .expect("leader shutdown must resolve in-flight MCP work")
        .unwrap()
        .unwrap();
    assert_eq!(lost.is_error, Some(true));
    assert_eq!(
        lost.structured_content.as_ref().unwrap()["code"],
        "CONNECTION_LOST"
    );
    assert_eq!(
        serde_json::from_value::<figma_dev_mcp_protocol::error::ToolError>(
            lost.structured_content.clone().unwrap()
        )
        .unwrap()
        .code(),
        ErrorCode::ConnectionLost
    );
    let _ = first_request;

    let second = elect(config.clone()).await.unwrap();
    let ElectionOutcome::Leader(leader) = second else {
        panic!("restarted process must lead after the previous listeners close");
    };
    let broker = leader.broker.clone();
    let plugin_task = tokio::spawn(broker.clone().serve(leader.plugin_listener));
    let frontend_task = tokio::spawn(broker.clone().serve_frontends(leader.frontend_listener));
    let mut plugin = connect_plugin(config.plugin_address, CONNECTION, "After restart").await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let restarted_server = tokio::spawn({
        let broker = broker.clone();
        async move {
            McpService::new(BrokerClient::local(broker))
                .serve(server_io)
                .await
                .unwrap()
        }
    });
    let restarted = ().serve(client_io).await.unwrap();
    let retry = tokio::spawn({
        let peer = restarted.peer().clone();
        async move {
            peer.call_tool(CallToolRequestParams::new("get_metadata").with_arguments(
                serde_json::Map::from_iter([("connectionId".to_owned(), json!(CONNECTION))]),
            ))
            .await
        }
    });
    let request = next_plugin_request(&mut plugin).await;
    send_metadata(&mut plugin, request.request_id.as_str()).await;
    let retry = retry.await.unwrap().unwrap();
    assert_ne!(retry.is_error, Some(true));
    assert_eq!(
        retry.structured_content.unwrap()["file"]["name"],
        "Restarted"
    );

    drop(restarted);
    restarted_server.abort();
    server_task.abort();
    drop(plugin);
    broker.shutdown().await;
    plugin_task.await.unwrap().unwrap();
    frontend_task.await.unwrap().unwrap();
}
