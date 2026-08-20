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
async fn dev_mode_reactions_and_motion_round_trip_through_server_broker_and_plugin() {
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
                "get_dev_mode_data" => {
                    assert_eq!(
                        request["operation"]["input"]["selector"],
                        json!({"nodeId": "4:1"})
                    );
                    json!({
                        "items": [{
                            "status": "success",
                            "value": {
                                "nodeId": "4:1",
                                "description": "Primary card",
                                "descriptionMarkdown": "**Primary** card",
                                "annotations": [{
                                    "id": "4:1:annotation:0",
                                    "categoryId": "cat-note",
                                    "text": "Match padding"
                                }],
                                "annotationCategories": [{"id": "cat-note", "label": "Note"}],
                                "documentation": [{
                                    "name": "Guide",
                                    "uri": "https://docs.example/card"
                                }],
                                "devResources": [{
                                    "name": "Storybook",
                                    "uri": "https://storybook.example/card"
                                }],
                                "ownerNodeId": "4:1",
                                "inheritedFromNodeId": "2:1"
                            }
                        }],
                        "visitedNodes": 1,
                        "truncated": false,
                        "observation": observation()
                    })
                }
                "get_reactions" => {
                    assert_eq!(
                        request["operation"]["input"]["selector"],
                        json!({"nodeId": "5:1"})
                    );
                    json!({
                        "items": [{
                            "status": "success",
                            "value": {
                                "nodeId": "5:1",
                                "reactions": [
                                    {
                                        "trigger": "click",
                                        "action": {"type": "navigate", "destinationId": "5:9"},
                                        "transitionId": "SMART_ANIMATE",
                                        "destinationAccessible": true
                                    },
                                    {
                                        "trigger": "hover",
                                        "action": {"type": "openOverlay", "destinationId": "5:8"},
                                        "destinationAccessible": true,
                                        "overlay": {
                                            "relativePosition": {"x": 8.0, "y": 12.0},
                                            "positionType": "bottomRight",
                                            "background": {
                                                "type": "solidColor",
                                                "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.4}
                                            },
                                            "backgroundInteraction": "closeOnClickOutside"
                                        }
                                    },
                                    {
                                        "trigger": "press",
                                        "action": {"type": "closeOverlay"},
                                        "destinationAccessible": true
                                    },
                                    {
                                        "trigger": "drag",
                                        "action": {"type": "back"},
                                        "destinationAccessible": true
                                    },
                                    {
                                        "trigger": "afterDelay",
                                        "action": {"type": "changeTo", "destinationId": "5:7"},
                                        "destinationAccessible": true
                                    },
                                    {
                                        "trigger": "click",
                                        "action": {"type": "navigate"},
                                        "destinationAccessible": false
                                    }
                                ]
                            }
                        }],
                        "visitedNodes": 1,
                        "truncated": false,
                        "observation": observation()
                    })
                }
                "get_motion" => {
                    assert_eq!(
                        request["operation"]["input"]["selector"],
                        json!({"nodeId": "6:1"})
                    );
                    assert_eq!(
                        request["operation"]["input"]["includeAvailableStyles"],
                        true
                    );
                    json!({
                        "items": [{
                            "status": "success",
                            "value": {
                                "nodeId": "6:1",
                                "animationStyles": [{
                                    "id": "applied-1",
                                    "styleId": "S:fade",
                                    "name": "Fade in",
                                    "duration": 0.4,
                                    "timelineOffset": 0.1,
                                    "props": [{"name": "direction", "value": "right"}]
                                }],
                                "animations": [{
                                    "field": {"type": "property", "name": "TRANSLATION_X"},
                                    "baseValue": {"type": "FLOAT", "value": 0.0},
                                    "timelineDuration": 0.4,
                                    "tracks": [{
                                        "id": "track-1",
                                        "keyframeOperation": "SET",
                                        "keyframes": [{
                                            "id": "kf-1",
                                            "timelinePosition": 0.4,
                                            "value": {"type": "FLOAT", "value": 120.0},
                                            "easing": {"type": "EASE_IN_BACK"}
                                        }]
                                    }]
                                }],
                                "manualKeyframeTracks": [{
                                    "field": {"type": "indexedItem", "collection": "fills", "index": 0},
                                    "id": "manual-1",
                                    "baseValue": {"type": "unsupported", "tag": "MESH"},
                                    "keyframes": []
                                }],
                                "timelines": [{"id": "tl-1", "duration": 0.4}]
                            }
                        }],
                        "availableStyles": [{
                            "styleId": "S:fade",
                            "name": "Fade in",
                            "description": "Catalog fade",
                            "props": [{"name": "direction", "value": "string"}]
                        }],
                        "visitedNodes": 1,
                        "truncated": false,
                        "observation": observation()
                    })
                }
                other => panic!("unexpected dev-mode operation {other}"),
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

    let dev_mode = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_dev_mode_data").with_arguments(
                serde_json::Map::from_iter([
                    connection.clone(),
                    ("selector".to_owned(), json!({"nodeId": "4:1"})),
                ]),
            ),
        )
        .await
        .unwrap();
    let dev_value = dev_mode.structured_content.clone().unwrap();
    assert_ne!(dev_mode.is_error, Some(true));
    assert_eq!(dev_value["items"][0]["value"]["nodeId"], "4:1");
    assert_eq!(
        dev_value["items"][0]["value"]["description"],
        "Primary card"
    );
    assert_eq!(
        dev_value["items"][0]["value"]["descriptionMarkdown"],
        "**Primary** card"
    );
    assert_eq!(
        dev_value["items"][0]["value"]["annotations"][0]["text"],
        "Match padding"
    );
    assert_eq!(
        dev_value["items"][0]["value"]["devResources"][0]["uri"],
        "https://storybook.example/card"
    );
    assert_eq!(dev_value["items"][0]["value"]["inheritedFromNodeId"], "2:1");

    let reactions = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_reactions").with_arguments(
                serde_json::Map::from_iter([
                    connection.clone(),
                    ("selector".to_owned(), json!({"nodeId": "5:1"})),
                ]),
            ),
        )
        .await
        .unwrap();
    let reactions_value = reactions.structured_content.clone().unwrap();
    assert_ne!(reactions.is_error, Some(true));
    assert_eq!(
        reactions_value["items"][0]["value"]["reactions"][0]["action"],
        json!({"type": "navigate", "destinationId": "5:9"})
    );
    assert_eq!(
        reactions_value["items"][0]["value"]["reactions"][1]["action"]["type"],
        "openOverlay"
    );
    assert_eq!(
        reactions_value["items"][0]["value"]["reactions"][1]["overlay"],
        json!({
            "relativePosition": {"x": 8.0, "y": 12.0},
            "positionType": "bottomRight",
            "background": {
                "type": "solidColor",
                "color": {"r": 0.0, "g": 0.0, "b": 0.0, "a": 0.4}
            },
            "backgroundInteraction": "closeOnClickOutside"
        })
    );
    assert_eq!(
        reactions_value["items"][0]["value"]["reactions"][2]["action"]["type"],
        "closeOverlay"
    );
    assert_eq!(
        reactions_value["items"][0]["value"]["reactions"][3]["action"]["type"],
        "back"
    );
    assert_eq!(
        reactions_value["items"][0]["value"]["reactions"][4]["action"]["type"],
        "changeTo"
    );
    assert_eq!(
        reactions_value["items"][0]["value"]["reactions"][5],
        json!({
            "trigger": "click",
            "action": {"type": "navigate"},
            "destinationAccessible": false
        })
    );

    let motion = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_motion").with_arguments(
                serde_json::Map::from_iter([
                    connection,
                    ("selector".to_owned(), json!({"nodeId": "6:1"})),
                    ("includeAvailableStyles".to_owned(), json!(true)),
                ]),
            ),
        )
        .await
        .unwrap();
    let motion_value = motion.structured_content.clone().unwrap();
    assert_ne!(motion.is_error, Some(true));
    assert_eq!(
        motion_value["items"][0]["value"]["animationStyles"][0]["duration"],
        0.4
    );
    assert_eq!(
        motion_value["items"][0]["value"]["animationStyles"][0]["styleId"],
        "S:fade"
    );
    assert_eq!(
        motion_value["items"][0]["value"]["animations"][0]["timelineDuration"],
        0.4
    );
    assert_eq!(
        motion_value["items"][0]["value"]["timelines"][0],
        json!({"id": "tl-1", "duration": 0.4})
    );
    assert_eq!(motion_value["availableStyles"][0]["styleId"], "S:fade");
    assert!(
        motion_value["items"][0]["value"]["animationStyles"][0]
            .get("durationMs")
            .is_none()
    );

    plugin_task.await.unwrap();
    drop(client);
    server_task.abort();
    broker_task.abort();
}
