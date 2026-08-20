use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits, PLUGIN_PROTOCOL_VERSION};
use figma_dev_mcp_tools::{McpService, SearchNodesInput, tools_catalog};
use futures_util::{SinkExt, StreamExt};
use rmcp::ServiceExt;
use serde_json::{Value, json};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async, tungstenite::Message, tungstenite::client::IntoClientRequest,
};

use super::multi_client::connect_plugin;

const FIRST_CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174000";
const SECOND_CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174001";

async fn running_broker() -> (SocketAddr, Broker, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let server = broker.clone();
    let task = tokio::spawn(async move { server.serve(listener).await.unwrap() });
    (address, broker, task)
}

async fn wait_for_sessions(broker: &Broker, count: usize) {
    for _ in 0..50 {
        if broker.live_file_count().await == count {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert_eq!(broker.live_file_count().await, count);
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

fn search_schema() -> jsonschema::Validator {
    let schema = tools_catalog()
        .tools
        .into_iter()
        .find(|tool| tool.name == "search_nodes")
        .expect("search_nodes must be cataloged")
        .input_schema;
    jsonschema::validator_for(&Value::Object((*schema).clone()))
        .expect("search_nodes schema must be a valid JSON Schema")
}

#[test]
fn search_nodes_schema_exposes_flat_query_filters_limit_and_cursor() {
    let validator = search_schema();
    let schema = tools_catalog()
        .tools
        .into_iter()
        .find(|tool| tool.name == "search_nodes")
        .expect("search_nodes must be cataloged")
        .input_schema;
    let rendered = Value::Object((*schema).clone());
    let rendered = rendered.to_string();
    assert!(
        rendered.contains("\"oneOf\""),
        "scope schema must be a oneOf: {rendered}"
    );
    assert!(
        rendered.contains("\"query\""),
        "query must be public: {rendered}"
    );
    assert!(
        rendered.contains("\"types\""),
        "types must be public: {rendered}"
    );
    assert!(
        rendered.contains("\"match\""),
        "match must be public: {rendered}"
    );
    assert!(
        rendered.contains("\"limit\""),
        "limit must be public: {rendered}"
    );
    assert!(
        rendered.contains("\"cursor\""),
        "cursor must be public: {rendered}"
    );

    assert!(validator.is_valid(&json!({
        "scope": {"pageId": "0:1"},
        "query": "Commission"
    })));
    assert!(validator.is_valid(&json!({
        "scope": {"nodeId": "1:1"},
        "types": ["FRAME", "COMPONENT"],
        "match": "exact",
        "limit": 25,
        "cursor": "opaque-cursor"
    })));
    assert!(!validator.is_valid(&json!({
        "scope": {"pageId": "0:1"},
        "query": {"text": {"value": "Commission", "mode": "contains"}}
    })));
    assert!(!validator.is_valid(&json!({
        "scope": {"pageId": "0:1"},
        "query": "Commission",
        "caseSensitive": true
    })));
    assert!(!validator.is_valid(&json!({
        "scope": {"pageId": "0:1"},
        "query": "Commission",
        "limit": 0
    })));
    assert!(!validator.is_valid(&json!({
        "scope": {"pageId": "0:1"},
        "query": "Commission",
        "limit": 2001
    })));

    for invalid in [
        json!({"query": "Card"}),
        json!({"scope":{"pageId":"0:1","nodeId":"1:1"},"query": "Card"}),
        json!({"scope":{"pageIds":["0:1"]},"query": "Card"}),
        json!({"scope":{"pageIds":["0:1","0:2"]},"query": "Card"}),
        json!({"scope":{"document":true},"query": "Card"}),
        json!({"scope":{"pageId":"0:1"}}),
    ] {
        assert!(
            !validator.is_valid(&invalid),
            "schema accepted invalid search input {invalid}"
        );
    }
}

#[test]
fn search_nodes_public_contract_trims_defaults_and_rejects_invalid_values() {
    assert!(
        serde_json::from_value::<SearchNodesInput>(json!({
            "scope": {"pageId": "0:1"},
            "types": ["   "]
        }))
        .is_err()
    );
    let parsed = serde_json::from_value::<SearchNodesInput>(json!({
        "scope": {"pageId": "0:1"},
        "query": " Commission ",
        "types": ["FRAME "],
        "cursor": " cursor-token "
    }))
    .expect("padded node type must deserialize");
    assert_eq!(
        serde_json::to_value(parsed).unwrap(),
        json!({
            "scope": {"pageId": "0:1"},
            "query": "Commission",
            "types": ["FRAME"],
            "match": "contains",
            "limit": 50,
            "cursor": "cursor-token"
        })
    );
}

#[tokio::test]
async fn search_nodes_without_connection_id_round_trips_and_rejects_invalid_scopes_before_dispatch()
{
    let (address, broker, broker_task) = running_broker().await;
    let mut request = format!("ws://{address}/").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "null".parse().unwrap());
    let (mut plugin, _) = connect_async(request).await.unwrap();
    plugin.send(hello(FIRST_CONNECTION)).await.unwrap();
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
        let request = loop {
            let Some(Ok(Message::Text(frame))) = plugin.next().await else {
                panic!("plugin did not receive request")
            };
            let request: Value = serde_json::from_str(&frame).unwrap();
            if request["type"] == "request" {
                break request;
            }
        };
        assert_eq!(request["operation"]["operation"], "search_nodes");
        assert_eq!(
            request["operation"]["input"]["scope"],
            json!({"pageId": "0:1"})
        );
        assert_eq!(request["operation"]["input"]["query"], json!("Card"));
        assert_eq!(request["operation"]["input"]["match"], "exact");
        assert_eq!(request["operation"]["input"]["limit"], 25);
        let request_id = request["requestId"].as_str().unwrap();
        plugin
            .send(Message::Text(
                json!({
                    "type": "response",
                    "requestId": request_id,
                    "result": {
                        "operation": "search_nodes",
                        "result": {
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
                        }
                    }
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
    });

    let connection = ("connectionId".to_owned(), json!(FIRST_CONNECTION));
    for (label, arguments) in [
        (
            "omitted scope",
            serde_json::Map::from_iter([connection.clone(), ("query".to_owned(), json!("Card"))]),
        ),
        (
            "both scopes",
            serde_json::Map::from_iter([
                connection.clone(),
                (
                    "scope".to_owned(),
                    json!({"pageId": "0:1", "nodeId": "1:1"}),
                ),
                ("query".to_owned(), json!("Card")),
            ]),
        ),
        (
            "pageIds",
            serde_json::Map::from_iter([
                connection.clone(),
                ("scope".to_owned(), json!({"pageIds": ["0:1"]})),
                ("query".to_owned(), json!("Card")),
            ]),
        ),
        (
            "document scope",
            serde_json::Map::from_iter([
                connection.clone(),
                ("scope".to_owned(), json!({"document": true})),
                ("query".to_owned(), json!("Card")),
            ]),
        ),
        (
            "multi-page scope",
            serde_json::Map::from_iter([
                connection.clone(),
                ("scope".to_owned(), json!({"pageIds": ["0:1", "0:2"]})),
                ("query".to_owned(), json!("Card")),
            ]),
        ),
        (
            "legacy nested query",
            serde_json::Map::from_iter([
                connection.clone(),
                ("scope".to_owned(), json!({"pageId": "0:1"})),
                (
                    "query".to_owned(),
                    json!({"name": {"value": "Card", "mode": "contains"}}),
                ),
            ]),
        ),
        (
            "empty query",
            serde_json::Map::from_iter([
                connection.clone(),
                ("scope".to_owned(), json!({"pageId": "0:1"})),
                ("query".to_owned(), json!("   ")),
            ]),
        ),
        (
            "whitespace nodeTypes",
            serde_json::Map::from_iter([
                connection.clone(),
                ("scope".to_owned(), json!({"pageId": "0:1"})),
                ("types".to_owned(), json!(["   "])),
            ]),
        ),
    ] {
        let result = client
            .call_tool(
                rmcp::model::CallToolRequestParams::new("search_nodes").with_arguments(arguments),
            )
            .await;
        assert!(result.is_err(), "{label} must fail before broker dispatch");
    }

    let search = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("search_nodes").with_arguments(
                serde_json::Map::from_iter([
                    ("scope".to_owned(), json!({"pageId": "0:1"})),
                    ("query".to_owned(), json!("Card")),
                    ("match".to_owned(), json!("exact")),
                    ("limit".to_owned(), json!(25)),
                ]),
            ),
        )
        .await
        .unwrap();
    let value = search.structured_content.clone().unwrap();
    assert_ne!(search.is_error, Some(true));
    assert_eq!(value["matches"][0]["node"]["id"], "1:2");
    assert_eq!(value["matches"][0]["reasons"], json!(["name"]));
    assert_eq!(value["truncated"], false);

    plugin_task.await.unwrap();
    server_task.abort();
    broker_task.abort();
}

#[tokio::test]
async fn search_nodes_without_connection_id_rejects_ambiguous_sessions() {
    let (address, broker, broker_task) = running_broker().await;
    let _first = connect_plugin(address, FIRST_CONNECTION, "First file").await;
    let _second = connect_plugin(address, SECOND_CONNECTION, "Second file").await;
    wait_for_sessions(&broker, 2).await;

    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server_task = tokio::spawn(async move {
        McpService::new(broker.clone())
            .serve(server_io)
            .await
            .unwrap()
    });
    let client = ().serve(client_io).await.unwrap();

    let result = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("search_nodes").with_arguments(
                serde_json::Map::from_iter([
                    ("scope".to_owned(), json!({"pageId": "0:1"})),
                    ("query".to_owned(), json!("Card")),
                ]),
            ),
        )
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.unwrap()["code"],
        "AMBIGUOUS_CONNECTION"
    );

    server_task.abort();
    broker_task.abort();
}

#[tokio::test]
async fn search_nodes_without_connection_id_routes_to_the_reconnected_session() {
    let (address, broker, broker_task) = running_broker().await;
    let first = connect_plugin(address, FIRST_CONNECTION, "Before reconnect").await;
    wait_for_sessions(&broker, 1).await;
    drop(first);
    wait_for_sessions(&broker, 0).await;

    let mut second = connect_plugin(address, SECOND_CONNECTION, "After reconnect").await;
    wait_for_sessions(&broker, 1).await;

    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let server_task = tokio::spawn(async move {
        McpService::new(broker.clone())
            .serve(server_io)
            .await
            .unwrap()
    });
    let client = ().serve(client_io).await.unwrap();

    let plugin_task = tokio::spawn(async move {
        let request = loop {
            let Some(Ok(Message::Text(frame))) = second.next().await else {
                panic!("reconnected plugin did not receive request")
            };
            let request: Value = serde_json::from_str(&frame).unwrap();
            if request["type"] == "request" {
                break request;
            }
        };
        assert_eq!(request["operation"]["operation"], "search_nodes");
        assert_eq!(request["operation"]["input"]["query"], "Card");
        let request_id = request["requestId"].as_str().unwrap();
        second
            .send(Message::Text(
                json!({
                    "type": "response",
                    "requestId": request_id,
                    "result": {
                        "operation": "search_nodes",
                        "result": {
                            "matches": [],
                            "truncated": false,
                            "observation": observation()
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
            rmcp::model::CallToolRequestParams::new("search_nodes").with_arguments(
                serde_json::Map::from_iter([
                    ("scope".to_owned(), json!({"pageId": "0:1"})),
                    ("query".to_owned(), json!("Card")),
                ]),
            ),
        )
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    assert_eq!(result.structured_content.unwrap()["matches"], json!([]));

    plugin_task.await.unwrap();
    server_task.abort();
    broker_task.abort();
}
