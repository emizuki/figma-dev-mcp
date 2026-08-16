use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits};
use figma_dev_mcp_tools::McpService;
use futures_util::{SinkExt, StreamExt};
use rmcp::ServiceExt;
use rmcp::model::ContentBlock;
use serde_json::{Value, json};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async, tungstenite::Message, tungstenite::client::IntoClientRequest,
};

const SAFE_SVG: &str = include_str!("../contracts/fixtures/safe.svg");
const TINY_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

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

fn decoded_len(base64: &str) -> usize {
    let padding = base64
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'=')
        .count();
    (base64.len() * 3 / 4).saturating_sub(padding)
}

#[tokio::test]
async fn screenshot_round_trips_raster_bytes_and_validated_svg_source() {
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
        for _ in 0..4 {
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
            assert_eq!(operation, "get_screenshot");
            let request_id = request["requestId"].as_str().unwrap();
            let input = &request["operation"]["input"];
            let result = match input["selector"] {
                Value::Object(ref selector) if selector.get("selection") == Some(&json!(true)) => {
                    json!({
                        "assets": [],
                        "truncated": false,
                        "observation": observation()
                    })
                }
                Value::Object(ref selector) if selector.get("nodeIds").is_some() => {
                    json!({
                        "assets": [
                            {
                                "status": "success",
                                "value": {
                                    "format": "png",
                                    "nodeId": "1:1",
                                    "dataBase64": TINY_PNG_BASE64,
                                    "width": 1,
                                    "height": 1
                                }
                            },
                            {
                                "status": "error",
                                "error": {
                                    "code": "NODE_NOT_FOUND",
                                    "message": "The requested node was not found.",
                                    "retryable": false
                                }
                            },
                            {
                                "status": "success",
                                "value": {
                                    "format": "png",
                                    "nodeId": "1:3",
                                    "dataBase64": TINY_PNG_BASE64,
                                    "width": 1,
                                    "height": 1
                                }
                            }
                        ],
                        "truncated": false,
                        "observation": observation()
                    })
                }
                _ if input["format"] == "svg" => {
                    json!({
                        "assets": [{
                            "status": "success",
                            "value": {
                                "format": "svg",
                                "nodeId": "9:1",
                                "source": SAFE_SVG
                            }
                        }],
                        "truncated": false,
                        "observation": observation()
                    })
                }
                _ => json!({
                    "assets": [{
                        "status": "success",
                        "value": {
                            "format": "png",
                            "nodeId": "8:1",
                            "dataBase64": TINY_PNG_BASE64,
                            "width": 1,
                            "height": 1
                        }
                    }],
                    "truncated": false,
                    "observation": observation()
                }),
            };
            plugin
                .send(Message::Text(
                    json!({
                        "type": "response",
                        "requestId": request_id,
                        "result": {
                            "operation": "get_screenshot",
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

    let raster = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_screenshot").with_arguments(
                serde_json::Map::from_iter([
                    connection.clone(),
                    ("format".to_owned(), json!("png")),
                    ("selector".to_owned(), json!({"nodeId": "8:1"})),
                    ("scale".to_owned(), json!(2.0)),
                ]),
            ),
        )
        .await
        .unwrap();
    assert_ne!(raster.is_error, Some(true));
    let raster_value = raster.structured_content.clone().unwrap();
    let raster_asset = &raster_value["assets"][0]["value"];
    assert_eq!(raster_asset["format"], "png");
    assert_eq!(raster_asset["nodeId"], "8:1");
    assert_eq!(raster_asset["width"], 1);
    assert_eq!(raster_asset["height"], 1);
    assert_eq!(raster_asset["decodedBytes"], decoded_len(TINY_PNG_BASE64));
    assert_eq!(raster_asset["base64Bytes"], TINY_PNG_BASE64.len());
    assert!(raster_asset.get("dataBase64").is_none());
    let images: Vec<_> = raster
        .content
        .iter()
        .filter_map(ContentBlock::as_image)
        .collect();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].data, TINY_PNG_BASE64);
    assert_eq!(images[0].mime_type, "image/png");

    let svg = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_screenshot").with_arguments(
                serde_json::Map::from_iter([
                    connection.clone(),
                    ("format".to_owned(), json!("svg")),
                    ("selector".to_owned(), json!({"nodeId": "9:1"})),
                ]),
            ),
        )
        .await
        .unwrap();
    assert_ne!(svg.is_error, Some(true));
    let svg_value = svg.structured_content.clone().unwrap();
    assert_eq!(svg_value["assets"][0]["value"]["source"], SAFE_SVG);
    let svg_text = svg
        .content
        .iter()
        .find_map(ContentBlock::as_text)
        .expect("compatibility JSON text");
    let compatibility: Value =
        serde_json::from_str(&svg_text.text).expect("compatibility text is JSON");
    assert_eq!(compatibility["assets"][0]["value"]["source"], SAFE_SVG);
    assert!(svg_text.text.contains("url(#gradient)"));
    let svg_images: Vec<_> = svg
        .content
        .iter()
        .filter_map(ContentBlock::as_image)
        .collect();
    assert_eq!(svg_images.len(), 1);
    assert_eq!(svg_images[0].mime_type, "image/svg+xml");

    let empty = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_screenshot").with_arguments(
                serde_json::Map::from_iter([
                    connection.clone(),
                    ("format".to_owned(), json!("png")),
                    ("selector".to_owned(), json!({"selection": true})),
                ]),
            ),
        )
        .await
        .unwrap();
    assert_ne!(empty.is_error, Some(true));
    assert_eq!(empty.structured_content.unwrap()["assets"], json!([]));
    assert!(empty.content.iter().all(|block| block.as_image().is_none()));

    let batch = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_screenshot").with_arguments(
                serde_json::Map::from_iter([
                    connection,
                    ("format".to_owned(), json!("png")),
                    (
                        "selector".to_owned(),
                        json!({"nodeIds": ["1:1", "1:2", "1:3"]}),
                    ),
                ]),
            ),
        )
        .await
        .unwrap();
    let batch_value = batch.structured_content.unwrap();
    assert_eq!(batch_value["assets"][0]["status"], "success");
    assert_eq!(batch_value["assets"][1]["error"]["code"], "NODE_NOT_FOUND");
    assert_eq!(batch_value["assets"][2]["status"], "success");
    assert_eq!(
        batch
            .content
            .iter()
            .filter(|block| block.as_image().is_some())
            .count(),
        2
    );

    plugin_task.await.unwrap();
    drop(client);
    server_task.abort();
    broker_task.abort();
}

#[tokio::test]
async fn screenshot_rejects_svg_scale_before_dispatch() {
    let (address, broker, broker_task) = running_broker().await;
    let mut request = format!("ws://{address}/").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "null".parse().unwrap());
    let (mut plugin, _) = connect_async(request).await.unwrap();
    plugin
        .send(hello("123e4567-e89b-42d3-a456-426614174001"))
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

    let err = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_screenshot").with_arguments(
                serde_json::Map::from_iter([
                    (
                        "connectionId".to_owned(),
                        json!("123e4567-e89b-42d3-a456-426614174001"),
                    ),
                    ("format".to_owned(), json!("svg")),
                    ("selector".to_owned(), json!({"nodeId": "9:1"})),
                    ("scale".to_owned(), json!(2.0)),
                ]),
            ),
        )
        .await
        .expect_err("SVG scale must be rejected before the plugin is invoked");
    assert!(
        err.to_string().to_lowercase().contains("invalid") || err.to_string().contains("scale")
    );

    drop(client);
    drop(plugin);
    server_task.abort();
    broker_task.abort();
}
