use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits};
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
            "type": "hello", "protocolVersion": "1", "connectionId": connection_id,
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

#[tokio::test]
async fn components_and_fonts_round_trip_through_server_broker_and_plugin() {
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
        for _ in 0..2 {
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
                "get_components" => {
                    assert_eq!(
                        request["operation"]["input"]["selector"],
                        json!({"pageIds": ["0:1", "0:3"]})
                    );
                    json!({
                        "components": [{
                            "id": "2:1",
                            "name": "Button",
                            "componentSetId": "2:0",
                            "description": "Primary button",
                            "documentation": [{
                                "uri": "https://docs.example/button",
                                "label": "Button"
                            }],
                            "variantProperties": [
                                {"name": "Size", "value": "Small"}
                            ],
                            "propertyDefinitions": [{
                                "name": "Size",
                                "defaultValue": {"kind": "variant", "value": "Small"},
                                "preferredValues": [
                                    {"kind": "variant", "value": "Small"},
                                    {"kind": "variant", "value": "Large"}
                                ]
                            }]
                        }],
                        "instances": [{
                            "instanceId": "4:1",
                            "componentId": "2:1"
                        }],
                        "truncated": false,
                        "observation": observation()
                    })
                }
                "get_fonts" => {
                    assert_eq!(
                        request["operation"]["input"]["selector"],
                        json!({"nodeId": "5:1"})
                    );
                    json!({
                        "fonts": [{
                            "font": {"family": "Inter", "style": "Regular"},
                            "availability": "unavailable",
                            "nodeIds": ["5:1"]
                        }],
                        "truncated": false,
                        "observation": observation()
                    })
                }
                other => panic!("unexpected components/fonts operation {other}"),
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

    let components = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_components").with_arguments(
                serde_json::Map::from_iter([
                    connection.clone(),
                    ("selector".to_owned(), json!({"pageIds": ["0:1", "0:3"]})),
                ]),
            ),
        )
        .await
        .unwrap();
    let components_value = components.structured_content.clone().unwrap();
    assert_ne!(components.is_error, Some(true));
    assert_eq!(components_value["components"][0]["id"], "2:1");
    assert_eq!(
        components_value["components"][0]["description"],
        "Primary button"
    );
    assert_eq!(
        components_value["components"][0]["documentation"][0]["uri"],
        "https://docs.example/button"
    );
    assert_eq!(
        components_value["components"][0]["variantProperties"][0]["name"],
        "Size"
    );
    assert_eq!(
        components_value["components"][0]["propertyDefinitions"][0]["defaultValue"],
        json!({"kind": "variant", "value": "Small"})
    );
    assert_eq!(
        components_value["instances"][0],
        json!({"instanceId": "4:1", "componentId": "2:1"})
    );
    assert_eq!(components_value["truncated"], false);

    let fonts = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_fonts").with_arguments(
                serde_json::Map::from_iter([
                    connection,
                    ("selector".to_owned(), json!({"nodeId": "5:1"})),
                ]),
            ),
        )
        .await
        .unwrap();
    let fonts_value = fonts.structured_content.clone().unwrap();
    assert_ne!(fonts.is_error, Some(true));
    assert_eq!(fonts_value["fonts"][0]["font"]["family"], "Inter");
    assert_eq!(fonts_value["fonts"][0]["availability"], "unavailable");
    assert_eq!(fonts_value["fonts"][0]["nodeIds"], json!(["5:1"]));
    assert_eq!(fonts_value["truncated"], false);

    plugin_task.await.unwrap();
    server_task.abort();
    broker_task.abort();
}
