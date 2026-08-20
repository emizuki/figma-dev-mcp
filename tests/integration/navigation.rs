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

fn observation() -> Value {
    json!({
        "startedAt": "2026-08-16T10:00:00.000Z",
        "completedAt": "2026-08-16T10:00:00.001Z"
    })
}

fn minimal_node(id: &str, name: &str) -> Value {
    json!({
        "summary": {
            "id": id,
            "name": name,
            "nodeType": "FRAME",
            "visible": true
        },
        "data": {},
        "children": [],
        "childrenTruncated": false
    })
}

#[tokio::test]
async fn navigation_tools_round_trip_through_server_broker_and_plugin() {
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

    let plugin_task = tokio::spawn(async move {
        for _ in 0..3 {
            let request = loop {
                let Some(Ok(Message::Text(frame))) = plugin.next().await else {
                    panic!("plugin did not receive request")
                };
                let request: Value = serde_json::from_str(&frame).unwrap();
                if request["type"] == "request" {
                    break request;
                }
            };
            let operation = request["operation"]["operation"].as_str().unwrap();
            let request_id = request["requestId"].as_str().unwrap();
            let result = match operation {
                "get_selection" => {
                    assert_eq!(request["operation"]["input"]["detail"], "minimal");
                    json!({
                        "detail": "minimal",
                        "nodes": [],
                        "truncated": false,
                        "observation": observation()
                    })
                }
                "get_nodes" => {
                    assert_eq!(
                        request["operation"]["input"]["nodeIds"],
                        json!(["1:1", "missing"])
                    );
                    json!({
                        "detail": "minimal",
                        "items": [
                            {
                                "status": "success",
                                "value": minimal_node("1:1", "Card")
                            },
                            {
                                "status": "error",
                                "error": {
                                    "code": "NODE_NOT_FOUND",
                                    "message": "The requested node was not found.",
                                    "retryable": false
                                }
                            }
                        ],
                        "truncated": false,
                        "observation": observation()
                    })
                }
                "get_design_context" => {
                    assert_eq!(
                        request["operation"]["input"]["selector"],
                        json!({"nodeId": "1:1"})
                    );
                    assert_eq!(request["operation"]["input"]["includeHidden"], false);
                    assert_eq!(request["operation"]["input"]["dedupeComponents"], true);
                    json!({
                        "detail": "minimal",
                        "roots": [minimal_node("1:1", "Card")],
                        "truncated": false,
                        "observation": observation()
                    })
                }
                other => panic!("unexpected navigation operation {other}"),
            };
            plugin
                .send(Message::Text(
                    json!({
                        "type": "response",
                        "requestId": request_id,
                        "result": {
                            "operation": operation,
                            "result": result
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .unwrap();
        }
    });

    let connection = (
        "connectionId".to_owned(),
        json!("123e4567-e89b-42d3-a456-426614174000"),
    );

    let selection = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_selection").with_arguments(
                serde_json::Map::from_iter([
                    connection.clone(),
                    ("detail".to_owned(), json!("minimal")),
                    ("depth".to_owned(), json!(0)),
                ]),
            ),
        )
        .await
        .unwrap();
    let selection_value = selection.structured_content.clone().unwrap();
    assert_ne!(selection.is_error, Some(true));
    assert_eq!(selection_value["detail"], "minimal");
    assert_eq!(selection_value["nodes"], json!([]));
    assert_eq!(selection_value["truncated"], false);

    let nodes = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_nodes").with_arguments(
                serde_json::Map::from_iter([
                    connection.clone(),
                    ("nodeIds".to_owned(), json!(["1:1", "missing"])),
                    ("detail".to_owned(), json!("minimal")),
                ]),
            ),
        )
        .await
        .unwrap();
    let nodes_value = nodes.structured_content.clone().unwrap();
    assert_eq!(nodes_value["items"][0]["status"], "success");
    assert_eq!(nodes_value["items"][0]["value"]["summary"]["id"], "1:1");
    assert_eq!(nodes_value["items"][1]["status"], "error");
    assert_eq!(nodes_value["items"][1]["error"]["code"], "NODE_NOT_FOUND");

    let context = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_design_context").with_arguments(
                serde_json::Map::from_iter([
                    connection,
                    ("selector".to_owned(), json!({"nodeId": "1:1"})),
                    ("detail".to_owned(), json!("minimal")),
                    ("includeHidden".to_owned(), json!(false)),
                    ("dedupeComponents".to_owned(), json!(true)),
                ]),
            ),
        )
        .await
        .unwrap();
    let context_value = context.structured_content.clone().unwrap();
    assert_eq!(context_value["detail"], "minimal");
    assert_eq!(context_value["roots"][0]["summary"]["id"], "1:1");
    assert_eq!(context_value["truncated"], false);

    plugin_task.await.unwrap();
    server_task.abort();
    broker_task.abort();
}
