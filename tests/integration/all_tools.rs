//! Invoke every catalog tool and prompt through production `McpService`.

use std::net::SocketAddr;
use std::time::Duration;

use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits};
use figma_dev_mcp_protocol::error::ToolError;
use figma_dev_mcp_protocol::limits::{INACTIVITY_TIMEOUT_SECS, MAX_IN_FLIGHT, MAX_QUEUE};
use figma_dev_mcp_protocol::{PROMPT_NAMES, TOOL_NAMES};
use figma_dev_mcp_tools::{McpService, tools_catalog};
use futures_util::{SinkExt, StreamExt};
use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ClientRequest, ContentBlock, GetPromptRequestParams,
    Request,
};
use rmcp::service::PeerRequestOptions;
use serde_json::{Map, Value, json};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

const FIRST: &str = "123e4567-e89b-42d3-a456-426614174000";
const SECOND: &str = "123e4567-e89b-42d3-a456-426614174001";
const TINY_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";
const SAFE_SVG: &str = include_str!("../contracts/fixtures/safe.svg");

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

fn decoded_len(base64: &str) -> usize {
    let padding = base64
        .bytes()
        .rev()
        .take_while(|byte| *byte == b'=')
        .count();
    (base64.len() * 3 / 4).saturating_sub(padding)
}

fn output_schema(name: &str) -> Value {
    let tool = tools_catalog()
        .tools
        .into_iter()
        .find(|tool| tool.name == name)
        .unwrap_or_else(|| panic!("{name} must be cataloged"));
    Value::Object(tool.output_schema.unwrap().as_ref().clone())
}

fn assert_output_schema(name: &str, value: &Value) {
    let schema = output_schema(name);
    let validator = jsonschema::validator_for(&schema).expect("output schema");
    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "{name} structured result failed schema: {errors:?}\n{value}"
    );
}

fn assert_tool_error(result: &CallToolResult) -> &str {
    assert_eq!(result.is_error, Some(true));
    let value = result
        .structured_content
        .as_ref()
        .expect("structured error");
    let schema = serde_json::to_value(schemars::schema_for!(ToolError)).unwrap();
    let validator = jsonschema::validator_for(&schema).expect("ToolError schema");
    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "error must match ToolError: {errors:?}\n{value}"
    );
    assert_eq!(result.content[0].as_text().unwrap().text, value.to_string());
    value["code"].as_str().expect("error code")
}

fn assert_structured_success(name: &str, result: &CallToolResult) -> Value {
    assert_ne!(
        result.is_error,
        Some(true),
        "{name} must succeed for the supported fixture"
    );
    let structured = result
        .structured_content
        .clone()
        .unwrap_or_else(|| panic!("{name} must have structured content"));
    assert_output_schema(name, &structured);
    assert_eq!(
        result.content[0]
            .as_text()
            .unwrap_or_else(|| panic!("{name} must keep compatibility JSON text"))
            .text,
        structured.to_string(),
        "{name} compatibility text must equal structured JSON"
    );
    assert!(
        structured.get("code").and_then(Value::as_str) != Some("CAPABILITY_UNAVAILABLE"),
        "{name} must not return the milestone capability error"
    );
    assert!(
        !structured.to_string().contains("durationMs"),
        "{name} must use the amended Motion shape"
    );
    structured
}

fn args(connection: &str, extra: Value) -> Map<String, Value> {
    let mut map = Map::from_iter([("connectionId".to_owned(), json!(connection))]);
    if let Value::Object(fields) = extra {
        map.extend(fields);
    }
    map
}

fn fixture_result(operation: &str, input: &Value) -> Value {
    match operation {
        "get_metadata" => json!({
            "file": {"name": "Checkout flow", "editorType": "dev"},
            "pages": [{"id": "0:1", "name": "Home"}, {"id": "0:2", "name": "Checkout"}],
            "currentPageId": "0:2",
            "pluginVersion": "0.1.0",
            "capabilities": {},
            "truncated": false,
            "observation": observation()
        }),
        "get_selection" => json!({
            "detail": input.get("detail").cloned().unwrap_or(json!("minimal")),
            "nodes": [minimal_node("1:1", "Card")],
            "truncated": false,
            "observation": observation()
        }),
        "get_nodes" => json!({
            "detail": input.get("detail").cloned().unwrap_or(json!("minimal")),
            "items": [
                {"status": "success", "value": minimal_node("1:1", "Card")},
                {"status": "error", "error": {
                    "code": "NODE_NOT_FOUND",
                    "message": "The requested node was not found.",
                    "retryable": false
                }}
            ],
            "truncated": false,
            "observation": observation()
        }),
        "search_nodes" => json!({
            "matches": [{
                "node": {
                    "id": "1:2",
                    "name": "Card",
                    "nodeType": "FRAME",
                    "visible": true
                },
                "reasons": ["name"]
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_design_context" => json!({
            "detail": input.get("detail").cloned().unwrap_or(json!("minimal")),
            "roots": [minimal_node("1:1", "Card")],
            "truncated": false,
            "observation": observation()
        }),
        "get_styles" => json!({
            "styles": [{
                "styleType": "paint",
                "id": "S:brand",
                "name": "Brand",
                "description": "Brand fill",
                "remote": false,
                "key": "paint-key",
                "paints": [{
                    "type": "solid",
                    "color": {"r": 1.0, "g": 0.0, "b": 0.0, "a": 1.0},
                    "opacity": 1.0
                }]
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_variables" => json!({
            "collections": [{
                "id": "C:theme",
                "name": "Theme",
                "modes": [{"id": "M:default", "name": "Default"}],
                "variables": [{
                    "id": "V:bg",
                    "name": "bg",
                    "collectionId": "C:theme",
                    "scopes": ["ALL_FILLS"],
                    "values": [{
                        "modeId": "M:default",
                        "source": {"kind": "color", "value": {
                            "r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0
                        }}
                    }],
                    "codeSyntax": [{"platform": "WEB", "code": "var(--bg)"}]
                }]
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_components" => json!({
            "components": [{
                "id": "2:1",
                "name": "Button",
                "componentSetId": "2:0",
                "description": "Primary button",
                "documentation": [{
                    "uri": "https://docs.example/button",
                    "label": "Button"
                }],
                "variantProperties": [{"name": "Size", "value": "Small"}],
                "propertyDefinitions": [{
                    "name": "Size",
                    "defaultValue": {"kind": "variant", "value": "Small"},
                    "preferredValues": [
                        {"kind": "variant", "value": "Small"},
                        {"kind": "variant", "value": "Large"}
                    ]
                }]
            }],
            "instances": [{"instanceId": "4:1", "componentId": "2:1"}],
            "truncated": false,
            "observation": observation()
        }),
        "get_fonts" => json!({
            "fonts": [{
                "font": {"family": "Inter", "style": "Regular"},
                "availability": "unavailable",
                "nodeIds": ["5:1"]
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_dev_mode_data" => json!({
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
            "truncated": false,
            "observation": observation()
        }),
        "get_reactions" => json!({
            "items": [{
                "status": "success",
                "value": {
                    "nodeId": "5:1",
                    "reactions": [{
                        "trigger": "click",
                        "action": {"type": "navigate", "destinationId": "5:9"},
                        "destinationAccessible": true
                    }]
                }
            }],
            "truncated": false,
            "observation": observation()
        }),
        "get_motion" => json!({
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
            "truncated": false,
            "observation": observation()
        }),
        "get_screenshot" => {
            if input.get("format") == Some(&json!("svg")) {
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
            } else if input
                .get("selector")
                .and_then(|selector| selector.get("nodeIds"))
                .is_some()
            {
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
                        }
                    ],
                    "truncated": false,
                    "observation": observation()
                })
            } else {
                json!({
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
                })
            }
        }
        other => panic!("unsupported fixture operation {other}"),
    }
}

#[derive(Debug)]
enum PluginInbound {
    Request {
        id: String,
        operation: String,
        input: Value,
    },
    Cancel(String),
}

enum PluginCommand {
    Respond {
        id: String,
        operation: String,
        input: Value,
    },
    Close,
}

struct ScriptedPlugin {
    inbound: mpsc::UnboundedReceiver<PluginInbound>,
    commands: mpsc::UnboundedSender<PluginCommand>,
}

async fn running_broker() -> (SocketAddr, Broker, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(Limits::production()).unwrap());
    let server = broker.clone();
    let task = tokio::spawn(async move { server.serve(listener).await.unwrap() });
    (address, broker, task)
}

fn hello(connection_id: &str, file_name: &str) -> Message {
    Message::Text(
        json!({
            "type": "hello", "protocolVersion": "1", "connectionId": connection_id,
            "displayName": file_name, "fileName": file_name,
            "currentPage": {"id": "0:2", "name": "Checkout"},
            "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
        })
        .to_string()
        .into(),
    )
}

async fn connect_scripted_plugin(
    address: SocketAddr,
    connection_id: &str,
    file_name: &str,
    auto_respond: bool,
) -> ScriptedPlugin {
    let mut request = format!("ws://{address}/").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "null".parse().unwrap());
    let (mut plugin, _) = connect_async(request).await.unwrap();
    plugin.send(hello(connection_id, file_name)).await.unwrap();

    let (inbound_tx, inbound) = mpsc::unbounded_channel();
    let (commands, mut command_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                incoming = plugin.next() => {
                    let Some(Ok(Message::Text(text))) = incoming else {
                        break;
                    };
                    let value: Value = serde_json::from_str(&text).unwrap();
                    match value["type"].as_str() {
                        Some("ping") => {
                            let nonce = value["nonce"].clone();
                            let _ = plugin.send(Message::Text(
                                json!({"type": "pong", "nonce": nonce}).to_string().into(),
                            )).await;
                        }
                        Some("request") => {
                            let id = value["requestId"].as_str().unwrap().to_owned();
                            let operation = value["operation"]["operation"]
                                .as_str()
                                .unwrap()
                                .to_owned();
                            let input = value["operation"]["input"].clone();
                            if auto_respond {
                                let result = fixture_result(&operation, &input);
                                let _ = plugin.send(Message::Text(
                                    json!({
                                        "type": "response",
                                        "requestId": id,
                                        "result": {
                                            "operation": operation,
                                            "result": result
                                        }
                                    })
                                    .to_string()
                                    .into(),
                                )).await;
                            } else {
                                let _ = inbound_tx.send(PluginInbound::Request { id, operation, input });
                            }
                        }
                        Some("cancel") => {
                            let _ = inbound_tx.send(PluginInbound::Cancel(
                                value["requestId"].as_str().unwrap().to_owned(),
                            ));
                        }
                        _ => {}
                    }
                }
                command = command_rx.recv() => {
                    let Some(command) = command else { break };
                    match command {
                        PluginCommand::Respond { id, operation, input } => {
                            let result = fixture_result(&operation, &input);
                            let _ = plugin.send(Message::Text(
                                json!({
                                    "type": "response",
                                    "requestId": id,
                                    "result": {
                                        "operation": operation,
                                        "result": result
                                    }
                                })
                                .to_string()
                                .into(),
                            )).await;
                        }
                        PluginCommand::Close => {
                            let _ = plugin.close(None).await;
                            break;
                        }
                    }
                }
            }
        }
    });
    ScriptedPlugin { inbound, commands }
}

async fn wait_for_sessions(broker: &Broker, count: usize) {
    for _ in 0..400 {
        if broker.live_file_count().await == count {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("expected {count} plugin sessions");
}

async fn start_mcp(
    broker: Broker,
) -> (
    rmcp::service::RunningService<rmcp::RoleClient, ()>,
    tokio::task::JoinHandle<rmcp::service::RunningService<rmcp::RoleServer, McpService>>,
) {
    let (server_io, client_io) = tokio::io::duplex(1024 * 1024);
    let server_task =
        tokio::spawn(async move { McpService::new(broker).serve(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    (client, server_task)
}

async fn recv_request(plugin: &mut ScriptedPlugin) -> (String, String, Value) {
    loop {
        match tokio::time::timeout(Duration::from_secs(2), plugin.inbound.recv()).await {
            Ok(Some(PluginInbound::Request {
                id,
                operation,
                input,
            })) => {
                return (id, operation, input);
            }
            Ok(Some(PluginInbound::Cancel(_))) => {}
            other => panic!("expected plugin request, got {other:?}"),
        }
    }
}

async fn expect_cancel(plugin: &mut ScriptedPlugin, request_id: &str) {
    tokio::time::resume();
    let received = tokio::time::timeout(Duration::from_secs(1), plugin.inbound.recv()).await;
    tokio::time::pause();
    match received {
        Ok(Some(PluginInbound::Cancel(cancelled))) => assert_eq!(cancelled, request_id),
        other => panic!("timeout must cancel the plugin request, got {other:?}"),
    }
}

#[tokio::test]
async fn every_tool_and_prompt_round_trips_through_mcp_service() {
    assert_eq!(TOOL_NAMES.len(), 14);
    assert_eq!(PROMPT_NAMES.len(), 3);
    let (address, broker, broker_task) = running_broker().await;
    let _plugin = connect_scripted_plugin(address, FIRST, "Checkout flow", true).await;
    wait_for_sessions(&broker, 1).await;
    let (client, server_task) = start_mcp(broker).await;

    let files = client
        .call_tool(CallToolRequestParams::new("list_files"))
        .await
        .unwrap();
    let files_value = assert_structured_success("list_files", &files);
    assert_eq!(files_value["files"][0]["fileName"], "Checkout flow");

    let metadata = client
        .call_tool(
            CallToolRequestParams::new("get_metadata").with_arguments(args(FIRST, json!({}))),
        )
        .await
        .unwrap();
    assert_structured_success("get_metadata", &metadata);

    let selection = client
        .call_tool(
            CallToolRequestParams::new("get_selection")
                .with_arguments(args(FIRST, json!({"detail": "minimal", "depth": 0}))),
        )
        .await
        .unwrap();
    assert_structured_success("get_selection", &selection);

    let nodes = client
        .call_tool(CallToolRequestParams::new("get_nodes").with_arguments(args(
            FIRST,
            json!({"nodeIds": ["1:1", "missing"], "detail": "minimal"}),
        )))
        .await
        .unwrap();
    let nodes_value = assert_structured_success("get_nodes", &nodes);
    assert_eq!(nodes_value["items"][1]["error"]["code"], "NODE_NOT_FOUND");

    let search = client
        .call_tool(
            CallToolRequestParams::new("search_nodes").with_arguments(args(
                FIRST,
                json!({
                    "scope": {"pageId": "0:1"},
                    "query": "Card", "match": "exact", "limit": 50
                }),
            )),
        )
        .await
        .unwrap();
    assert_structured_success("search_nodes", &search);

    let context = client
        .call_tool(
            CallToolRequestParams::new("get_design_context").with_arguments(args(
                FIRST,
                json!({
                    "selector": {"nodeId": "1:1"},
                    "detail": "minimal",
                    "includeHidden": false,
                    "dedupeComponents": true
                }),
            )),
        )
        .await
        .unwrap();
    assert_structured_success("get_design_context", &context);

    let styles = client
        .call_tool(
            CallToolRequestParams::new("get_styles").with_arguments(args(
                FIRST,
                json!({"selector": {"nodeId": "1:1"}, "source": "referenced"}),
            )),
        )
        .await
        .unwrap();
    assert_structured_success("get_styles", &styles);

    let variables = client
        .call_tool(
            CallToolRequestParams::new("get_variables")
                .with_arguments(args(FIRST, json!({"resolveAliases": true}))),
        )
        .await
        .unwrap();
    assert_structured_success("get_variables", &variables);

    let components = client
        .call_tool(
            CallToolRequestParams::new("get_components").with_arguments(args(
                FIRST,
                json!({"selector": {"pageIds": ["0:1", "0:3"]}}),
            )),
        )
        .await
        .unwrap();
    assert_structured_success("get_components", &components);

    let fonts = client
        .call_tool(
            CallToolRequestParams::new("get_fonts")
                .with_arguments(args(FIRST, json!({"selector": {"nodeId": "5:1"}}))),
        )
        .await
        .unwrap();
    assert_structured_success("get_fonts", &fonts);

    let dev_mode = client
        .call_tool(
            CallToolRequestParams::new("get_dev_mode_data")
                .with_arguments(args(FIRST, json!({"selector": {"nodeId": "4:1"}}))),
        )
        .await
        .unwrap();
    assert_structured_success("get_dev_mode_data", &dev_mode);

    let reactions = client
        .call_tool(
            CallToolRequestParams::new("get_reactions")
                .with_arguments(args(FIRST, json!({"selector": {"nodeId": "5:1"}}))),
        )
        .await
        .unwrap();
    assert_structured_success("get_reactions", &reactions);

    let motion = client
        .call_tool(
            CallToolRequestParams::new("get_motion").with_arguments(args(
                FIRST,
                json!({"selector": {"nodeId": "6:1"}, "includeAvailableStyles": false}),
            )),
        )
        .await
        .unwrap();
    let motion_value = assert_structured_success("get_motion", &motion);
    assert_eq!(
        motion_value["items"][0]["value"]["timelines"][0]["duration"],
        0.4
    );

    let raster = client
        .call_tool(
            CallToolRequestParams::new("get_screenshot").with_arguments(args(
                FIRST,
                json!({"format": "png", "selector": {"nodeId": "8:1"}, "scale": 2.0}),
            )),
        )
        .await
        .unwrap();
    let raster_value = assert_structured_success("get_screenshot", &raster);
    assert!(
        raster_value["assets"][0]["value"]
            .get("dataBase64")
            .is_none()
    );
    assert_eq!(
        raster_value["assets"][0]["value"]["decodedBytes"],
        decoded_len(TINY_PNG_BASE64)
    );
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
            CallToolRequestParams::new("get_screenshot").with_arguments(args(
                FIRST,
                json!({"format": "svg", "selector": {"nodeId": "9:1"}}),
            )),
        )
        .await
        .unwrap();
    let svg_value = assert_structured_success("get_screenshot", &svg);
    assert_eq!(svg_value["assets"][0]["value"]["source"], SAFE_SVG);
    let svg_images: Vec<_> = svg
        .content
        .iter()
        .filter_map(ContentBlock::as_image)
        .collect();
    assert_eq!(svg_images.len(), 1);
    assert_eq!(svg_images[0].mime_type, "image/svg+xml");

    for name in PROMPT_NAMES {
        let prompt = client
            .get_prompt(GetPromptRequestParams::new(name))
            .await
            .unwrap_or_else(|error| panic!("{name} must resolve: {error}"));
        assert_eq!(prompt.messages.len(), 1);
        assert!(
            !prompt.messages[0]
                .content
                .as_text()
                .unwrap()
                .text
                .trim()
                .is_empty()
        );
    }

    let missing = client
        .call_tool(
            CallToolRequestParams::new("get_metadata")
                .with_arguments(args("123e4567-e89b-42d3-a456-426614174099", json!({}))),
        )
        .await
        .unwrap();
    assert_eq!(assert_tool_error(&missing), "CONNECTION_NOT_FOUND");

    server_task.abort();
    broker_task.abort();
}

#[tokio::test]
async fn two_clients_and_two_files_are_explicitly_routed() {
    let (address, broker, broker_task) = running_broker().await;
    let _first = connect_scripted_plugin(address, FIRST, "First file", true).await;
    let _second = connect_scripted_plugin(address, SECOND, "Second file", true).await;
    wait_for_sessions(&broker, 2).await;
    let (first_client, first_server) = start_mcp(broker.clone()).await;
    let (second_client, second_server) = start_mcp(broker).await;

    let listed = first_client
        .call_tool(CallToolRequestParams::new("list_files"))
        .await
        .unwrap();
    let files = listed.structured_content.unwrap()["files"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(files.len(), 2);

    let ambiguous = first_client
        .call_tool(CallToolRequestParams::new("get_metadata"))
        .await
        .unwrap();
    assert_eq!(assert_tool_error(&ambiguous), "AMBIGUOUS_CONNECTION");

    let first_meta = first_client
        .call_tool(
            CallToolRequestParams::new("get_metadata").with_arguments(args(FIRST, json!({}))),
        )
        .await
        .unwrap();
    assert_structured_success("get_metadata", &first_meta);

    let second_meta = second_client
        .call_tool(
            CallToolRequestParams::new("get_metadata").with_arguments(args(SECOND, json!({}))),
        )
        .await
        .unwrap();
    assert_structured_success("get_metadata", &second_meta);

    first_server.abort();
    second_server.abort();
    broker_task.abort();
}

#[tokio::test]
async fn screenshot_partial_batch_keeps_successes_and_item_errors() {
    let (address, broker, broker_task) = running_broker().await;
    let _plugin = connect_scripted_plugin(address, FIRST, "Checkout flow", true).await;
    wait_for_sessions(&broker, 1).await;
    let (client, server_task) = start_mcp(broker).await;
    let batch = client
        .call_tool(
            CallToolRequestParams::new("get_screenshot").with_arguments(args(
                FIRST,
                json!({"format": "png", "selector": {"nodeIds": ["1:1", "1:2"]}}),
            )),
        )
        .await
        .unwrap();
    let value = assert_structured_success("get_screenshot", &batch);
    assert_eq!(value["assets"][0]["status"], "success");
    assert_eq!(value["assets"][1]["error"]["code"], "NODE_NOT_FOUND");
    assert_eq!(
        batch
            .content
            .iter()
            .filter(|block| block.as_image().is_some())
            .count(),
        1
    );
    server_task.abort();
    broker_task.abort();
}

#[tokio::test]
async fn queue_saturation_returns_retryable_limit_exceeded() {
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address, FIRST, "Checkout flow", false).await;
    wait_for_sessions(&broker, 1).await;
    let (client, server_task) = start_mcp(broker).await;
    let peer = client.peer().clone();
    let mut calls = Vec::new();
    for _ in 0..(MAX_IN_FLIGHT + MAX_QUEUE + 1) {
        let peer = peer.clone();
        calls.push(tokio::spawn(async move {
            peer.call_tool(
                CallToolRequestParams::new("get_metadata").with_arguments(args(FIRST, json!({}))),
            )
            .await
        }));
    }
    let mut held = Vec::new();
    for _ in 0..MAX_IN_FLIGHT {
        held.push(recv_request(&mut plugin).await);
    }
    let overflow = calls.pop().unwrap().await.unwrap().unwrap();
    assert_eq!(assert_tool_error(&overflow), "LIMIT_EXCEEDED");
    assert_eq!(
        overflow.structured_content.as_ref().unwrap()["retryable"],
        true
    );
    for (id, operation, input) in held {
        plugin
            .commands
            .send(PluginCommand::Respond {
                id,
                operation,
                input,
            })
            .unwrap();
    }
    server_task.abort();
    broker_task.abort();
}

#[tokio::test]
async fn cancellation_and_disconnect_use_stable_tool_errors() {
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address, FIRST, "Checkout flow", false).await;
    wait_for_sessions(&broker, 1).await;
    let (client, server_task) = start_mcp(broker.clone()).await;
    let handle = client
        .peer()
        .send_request_with_option(
            ClientRequest::CallToolRequest(Request::new(
                CallToolRequestParams::new("get_metadata").with_arguments(args(FIRST, json!({}))),
            )),
            PeerRequestOptions::no_options(),
        )
        .await
        .unwrap();
    let (request_id, _, _) = recv_request(&mut plugin).await;
    handle
        .peer
        .notify_cancelled(rmcp::model::CancelledNotificationParam::new(
            Some(handle.id.clone()),
            Some("user cancelled".into()),
        ))
        .await
        .unwrap();
    match tokio::time::timeout(Duration::from_secs(2), plugin.inbound.recv()).await {
        Ok(Some(PluginInbound::Cancel(cancelled))) => assert_eq!(cancelled, request_id),
        other => panic!("cancel must reach the plugin, got {other:?}"),
    }
    match handle.await_response().await {
        Ok(rmcp::model::ServerResult::CallToolResult(result)) => {
            assert_eq!(assert_tool_error(&result), "CANCELLED");
        }
        Err(rmcp::ServiceError::Cancelled { .. }) => {}
        other => panic!("cancel must resolve as CANCELLED, got {other:?}"),
    }

    let lost = tokio::spawn({
        let peer = client.peer().clone();
        async move {
            peer.call_tool(
                CallToolRequestParams::new("get_metadata").with_arguments(args(FIRST, json!({}))),
            )
            .await
        }
    });
    let _ = recv_request(&mut plugin).await;
    plugin.commands.send(PluginCommand::Close).unwrap();
    let lost = lost.await.unwrap().unwrap();
    assert_eq!(assert_tool_error(&lost), "CONNECTION_LOST");

    server_task.abort();
    broker_task.abort();
}

#[tokio::test(start_paused = true)]
async fn inactivity_timeout_uses_the_stable_timeout_error() {
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address, FIRST, "Checkout flow", false).await;
    wait_for_sessions(&broker, 1).await;
    let (client, server_task) = start_mcp(broker).await;
    let call = tokio::spawn({
        let peer = client.peer().clone();
        async move {
            peer.call_tool(
                CallToolRequestParams::new("get_metadata").with_arguments(args(FIRST, json!({}))),
            )
            .await
        }
    });
    let (request_id, _, _) = recv_request(&mut plugin).await;
    tokio::time::advance(Duration::from_secs(INACTIVITY_TIMEOUT_SECS)).await;
    let result = tokio::time::timeout(Duration::from_secs(1), call)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(assert_tool_error(&result), "TIMEOUT");
    expect_cancel(&mut plugin, &request_id).await;
    server_task.abort();
    broker_task.abort();
}
