use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits};
use figma_dev_mcp_tools::{GetStylesInput, GetVariablesInput, GetVariablesResult, McpService};
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

#[test]
fn public_contracts_default_style_source_and_alias_resolution() {
    let defaulted =
        serde_json::from_value::<GetStylesInput>(json!({})).expect("source defaults to both");
    assert_eq!(
        serde_json::to_value(&defaulted).unwrap()["source"],
        json!("both")
    );
    for source in ["local", "referenced", "both"] {
        let parsed = serde_json::from_value::<GetStylesInput>(json!({"source": source}))
            .unwrap_or_else(|_| panic!("source {source} must deserialize"));
        assert_eq!(
            serde_json::to_value(parsed).unwrap()["source"],
            json!(source)
        );
    }
    assert!(serde_json::from_value::<GetStylesInput>(json!({"source": "all"})).is_err());

    let aliases = serde_json::from_value::<GetVariablesInput>(json!({}))
        .expect("resolveAliases defaults to false");
    assert_eq!(
        serde_json::to_value(aliases).unwrap()["resolveAliases"],
        json!(false)
    );

    let with_errors = serde_json::from_value::<GetVariablesResult>(json!({
        "collections": [{
            "id": "C:theme",
            "name": "Theme",
            "modes": [
                {"id": "M:missing", "name": "Missing"},
                {"id": "M:cycle", "name": "Cycle"}
            ],
            "variables": [{
                "id": "V:broken",
                "name": "broken",
                "collectionId": "C:theme",
                "scopes": [],
                "values": [
                    {
                        "modeId": "M:missing",
                        "source": {"kind": "alias", "value": "V:gone"},
                        "error": {"code": "NODE_NOT_FOUND", "retryable": false}
                    },
                    {
                        "modeId": "M:cycle",
                        "source": {"kind": "alias", "value": "V:a"},
                        "error": {"code": "LIMIT_EXCEEDED", "retryable": false}
                    }
                ],
                "codeSyntax": []
            }]
        }],
        "truncated": false,
        "observation": {
            "startedAt": "2026-08-16T10:00:00.000Z",
            "completedAt": "2026-08-16T10:00:00.001Z"
        }
    }))
    .expect("cycle and missing alias errors must deserialize on mode values");
    let encoded = serde_json::to_value(with_errors).unwrap();
    assert_eq!(
        encoded["collections"][0]["variables"][0]["values"][0]["error"],
        json!({"code": "NODE_NOT_FOUND", "retryable": false})
    );
    assert_eq!(
        encoded["collections"][0]["variables"][0]["values"][1]["error"],
        json!({"code": "LIMIT_EXCEEDED", "retryable": false})
    );
    assert!(
        encoded["collections"][0]["variables"][0]["values"][0]
            .get("resolved")
            .is_none()
    );
}

#[tokio::test]
async fn styles_and_variables_round_trip_through_server_broker_and_plugin() {
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
                "get_styles" => {
                    assert_eq!(
                        request["operation"]["input"]["selector"],
                        json!({"nodeId": "1:1"})
                    );
                    assert_eq!(request["operation"]["input"]["source"], "referenced");
                    json!({
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
                    })
                }
                "get_variables" => {
                    assert_eq!(request["operation"]["input"]["resolveAliases"], true);
                    json!({
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
                                    "source": {"kind": "alias", "value": "V:leaf"},
                                    "resolved": {
                                        "kind": "color",
                                        "value": {"r": 1.0, "g": 1.0, "b": 1.0, "a": 1.0}
                                    }
                                }],
                                "codeSyntax": [{"platform": "WEB", "code": "var(--bg)"}]
                            }]
                        }],
                        "truncated": false,
                        "observation": observation()
                    })
                }
                other => panic!("unexpected design-system operation {other}"),
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

    let styles = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_styles").with_arguments(
                serde_json::Map::from_iter([
                    connection.clone(),
                    ("selector".to_owned(), json!({"nodeId": "1:1"})),
                    ("source".to_owned(), json!("referenced")),
                ]),
            ),
        )
        .await
        .unwrap();
    let styles_value = styles.structured_content.clone().unwrap();
    assert_ne!(styles.is_error, Some(true));
    assert_eq!(styles_value["styles"][0]["id"], "S:brand");
    assert_eq!(styles_value["styles"][0]["styleType"], "paint");
    assert_eq!(styles_value["styles"][0]["description"], "Brand fill");
    assert_eq!(styles_value["styles"][0]["remote"], false);
    assert_eq!(styles_value["styles"][0]["key"], "paint-key");
    assert_eq!(styles_value["truncated"], false);

    let variables = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("get_variables").with_arguments(
                serde_json::Map::from_iter([
                    connection,
                    ("resolveAliases".to_owned(), json!(true)),
                ]),
            ),
        )
        .await
        .unwrap();
    let variables_value = variables.structured_content.clone().unwrap();
    assert_ne!(variables.is_error, Some(true));
    assert_eq!(
        variables_value["collections"][0]["variables"][0]["values"][0]["source"],
        json!({"kind": "alias", "value": "V:leaf"})
    );
    assert_eq!(
        variables_value["collections"][0]["variables"][0]["values"][0]["resolved"]["kind"],
        "color"
    );

    plugin_task.await.unwrap();
    server_task.abort();
    broker_task.abort();
}
