use std::sync::Arc;
use std::time::Duration;

use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits, PLUGIN_PROTOCOL_VERSION};
use figma_dev_mcp_protocol::limits::{
    INACTIVITY_TIMEOUT_SECS, MAX_IN_FLIGHT, MAX_QUEUE, TOTAL_TIMEOUT_SECS,
};
use figma_dev_mcp_tools::McpService;
use futures_util::{SinkExt, StreamExt};
use rmcp::{
    ClientHandler, ServiceExt,
    model::{
        CallToolRequestParams, ClientRequest, NumberOrString, ProgressNotificationParam,
        ProgressToken, Request, RequestMetaObject,
    },
    service::PeerRequestOptions,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

const CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174000";
const SENTINEL_NODE_TEXT: &str = "SENTINEL_NODE_TEXT_CHECKOUT_LABEL";

#[derive(Debug, Clone)]
enum PluginInbound {
    Request(String),
    Cancel(String),
}

struct ScriptedPlugin {
    inbound: mpsc::UnboundedReceiver<PluginInbound>,
    commands: mpsc::UnboundedSender<PluginCommand>,
}

enum PluginCommand {
    Respond(String),
    Progress {
        request_id: String,
        completed: u32,
        total: Option<u32>,
        message: Option<String>,
    },
    Close,
}

async fn running_broker() -> (std::net::SocketAddr, Broker, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(Limits::production()).unwrap());
    let server = broker.clone();
    let task = tokio::spawn(async move { server.serve(listener).await.unwrap() });
    (address, broker, task)
}

fn hello() -> Message {
    Message::Text(
        json!({
            "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION, "connectionId": CONNECTION,
            "displayName": "Checkout flow", "fileName": "Checkout flow",
            "currentPage": {"id": "0:2", "name": "Checkout"},
            "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
        })
        .to_string()
        .into(),
    )
}

fn metadata_args() -> serde_json::Map<String, Value> {
    serde_json::Map::from_iter([("connectionId".to_owned(), json!(CONNECTION))])
}

fn metadata_response(request_id: &str) -> Message {
    Message::Text(
        json!({
            "type": "response", "requestId": request_id,
            "result": {
                "operation": "get_metadata",
                "result": {
                    "file": {"name": "Checkout flow", "editorType": "dev"},
                    "pages": [{"id": "0:1", "name": "Home"}],
                    "currentPageId": "0:1", "capabilities": {}, "truncated": false,
                    "pluginVersion": "0.1.0",
                    "observation": {"startedAt": "1", "completedAt": "2"}
                }
            }
        })
        .to_string()
        .into(),
    )
}

async fn connect_scripted_plugin(address: std::net::SocketAddr) -> ScriptedPlugin {
    let mut request = format!("ws://{address}/").into_client_request().unwrap();
    request
        .headers_mut()
        .insert("Origin", "null".parse().unwrap());
    let (mut plugin, _) = connect_async(request).await.unwrap();
    plugin.send(hello()).await.unwrap();

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
                            let _ = inbound_tx.send(PluginInbound::Request(
                                value["requestId"].as_str().unwrap().to_owned(),
                            ));
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
                        PluginCommand::Respond(request_id) => {
                            let _ = plugin.send(metadata_response(&request_id)).await;
                        }
                        PluginCommand::Progress { request_id, completed, total, message } => {
                            let mut frame = json!({
                                "type": "progress",
                                "requestId": request_id,
                                "completed": completed,
                            });
                            if let Some(total) = total {
                                frame["total"] = json!(total);
                            }
                            if let Some(message) = message {
                                frame["message"] = json!(message);
                            }
                            let _ = plugin.send(Message::Text(frame.to_string().into())).await;
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

async fn wait_for_session(broker: &Broker) {
    for _ in 0..200 {
        if broker.live_file_count().await == 1 {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("plugin session did not register");
}

async fn expect_cancel(plugin: &mut ScriptedPlugin, request_id: &str, what: &str) {
    tokio::time::resume();
    let received = tokio::time::timeout(Duration::from_secs(1), plugin.inbound.recv()).await;
    tokio::time::pause();
    match received {
        Ok(Some(PluginInbound::Cancel(cancelled))) => assert_eq!(cancelled, request_id),
        other => panic!("{what}, got {other:?}"),
    }
}

async fn recv_requests(plugin: &mut ScriptedPlugin, count: usize) -> Vec<String> {
    let mut ids = Vec::with_capacity(count);
    while ids.len() < count {
        match tokio::time::timeout(Duration::from_secs(2), plugin.inbound.recv()).await {
            Ok(Some(PluginInbound::Request(id))) => ids.push(id),
            Ok(Some(PluginInbound::Cancel(_))) => {}
            other => panic!("expected plugin request, got {other:?}"),
        }
    }
    ids
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

async fn call_metadata(
    peer: rmcp::service::Peer<rmcp::RoleClient>,
) -> Result<rmcp::model::CallToolResult, rmcp::ServiceError> {
    peer.call_tool(CallToolRequestParams::new("get_metadata").with_arguments(metadata_args()))
        .await
}

fn tool_error_code(result: &rmcp::model::CallToolResult) -> &str {
    result.structured_content.as_ref().unwrap()["code"]
        .as_str()
        .unwrap()
}

#[tokio::test]
async fn four_active_and_sixteen_queued_then_retryable_overflow() {
    assert_eq!(MAX_IN_FLIGHT, 4);
    assert_eq!(MAX_QUEUE, 16);
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address).await;
    wait_for_session(&broker).await;
    let (client, server_task) = start_mcp(broker).await;
    let peer = client.peer().clone();

    let mut calls = Vec::new();
    for _ in 0..(MAX_IN_FLIGHT + MAX_QUEUE + 1) {
        let peer = peer.clone();
        calls.push(tokio::spawn(async move { call_metadata(peer).await }));
    }

    let dispatched = recv_requests(&mut plugin, MAX_IN_FLIGHT).await;
    assert_eq!(dispatched.len(), MAX_IN_FLIGHT);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), plugin.inbound.recv())
            .await
            .is_err(),
        "queued work must not dispatch before a permit is free"
    );

    let overflow = calls
        .pop()
        .unwrap()
        .await
        .unwrap()
        .expect("overflow is a tool result");
    assert_eq!(overflow.is_error, Some(true));
    assert_eq!(tool_error_code(&overflow), "LIMIT_EXCEEDED");
    assert_eq!(
        overflow.structured_content.as_ref().unwrap()["retryable"],
        true
    );

    for request_id in &dispatched {
        plugin
            .commands
            .send(PluginCommand::Respond(request_id.clone()))
            .unwrap();
    }
    let next = recv_requests(&mut plugin, 1).await;
    plugin
        .commands
        .send(PluginCommand::Respond(next[0].clone()))
        .unwrap();

    server_task.abort();
    broker_task.abort();
}

#[tokio::test]
async fn queue_admission_is_fifo_and_drop_cancels_queued_work() {
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address).await;
    wait_for_session(&broker).await;
    let (client, server_task) = start_mcp(broker).await;
    let peer = client.peer().clone();

    let mut holders = Vec::new();
    for _ in 0..MAX_IN_FLIGHT {
        let peer = peer.clone();
        holders.push(tokio::spawn(async move { call_metadata(peer).await }));
    }
    let first_wave = recv_requests(&mut plugin, MAX_IN_FLIGHT).await;

    let first_queued = tokio::spawn(call_metadata(peer.clone()));
    let second_queued = tokio::spawn(call_metadata(peer.clone()));
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    let cancelled_queued = peer
        .send_request_with_option(
            ClientRequest::CallToolRequest(Request::new(
                CallToolRequestParams::new("get_metadata").with_arguments(metadata_args()),
            )),
            PeerRequestOptions::no_options(),
        )
        .await
        .unwrap();
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    let cancel_peer = cancelled_queued.peer.clone();
    let cancel_id = cancelled_queued.id.clone();
    cancel_peer
        .notify_cancelled(rmcp::model::CancelledNotificationParam::new(
            Some(cancel_id),
            Some("drop queued work".into()),
        ))
        .await
        .unwrap();
    match cancelled_queued.await_response().await {
        Ok(rmcp::model::ServerResult::CallToolResult(cancelled)) => {
            assert_eq!(tool_error_code(&cancelled), "CANCELLED");
        }
        Err(rmcp::ServiceError::Cancelled { .. }) => {}
        other => panic!("queued cancel must resolve as CANCELLED, got {other:?}"),
    }

    plugin
        .commands
        .send(PluginCommand::Respond(first_wave[0].clone()))
        .unwrap();
    let next = recv_requests(&mut plugin, 1).await;
    assert_ne!(next[0], first_wave[0]);
    plugin
        .commands
        .send(PluginCommand::Respond(next[0].clone()))
        .unwrap();
    let first_queued = first_queued.await.unwrap().unwrap();
    assert_eq!(first_queued.is_error, Some(false));

    let second_id = recv_requests(&mut plugin, 1).await.pop().unwrap();
    plugin
        .commands
        .send(PluginCommand::Respond(second_id))
        .unwrap();
    assert_eq!(second_queued.await.unwrap().unwrap().is_error, Some(false));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), plugin.inbound.recv())
            .await
            .is_err(),
        "cancelled queued work must never dispatch"
    );
    drop(holders);

    server_task.abort();
    broker_task.abort();
}

#[tokio::test(start_paused = true)]
async fn inactivity_deadline_is_fifteen_seconds_without_progress() {
    assert_eq!(INACTIVITY_TIMEOUT_SECS, 15);
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address).await;
    wait_for_session(&broker).await;
    let (client, server_task) = start_mcp(broker).await;
    let call = tokio::spawn(call_metadata(client.peer().clone()));
    let request_id = recv_requests(&mut plugin, 1).await.pop().unwrap();

    tokio::time::advance(Duration::from_secs(INACTIVITY_TIMEOUT_SECS)).await;
    let result = tokio::time::timeout(Duration::from_secs(1), call)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(tool_error_code(&result), "TIMEOUT");
    expect_cancel(
        &mut plugin,
        &request_id,
        "timeout must cancel the same socket request",
    )
    .await;

    server_task.abort();
    broker_task.abort();
}

#[tokio::test(start_paused = true)]
async fn progress_resets_inactivity_but_not_the_total_deadline() {
    assert_eq!(TOTAL_TIMEOUT_SECS, 120);
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address).await;
    wait_for_session(&broker).await;
    let (client, server_task) = start_mcp(broker).await;
    let call = tokio::spawn(call_metadata(client.peer().clone()));
    let request_id = recv_requests(&mut plugin, 1).await.pop().unwrap();

    for completed in 1..=3 {
        plugin
            .commands
            .send(PluginCommand::Progress {
                request_id: request_id.clone(),
                completed,
                total: Some(12),
                message: Some(SENTINEL_NODE_TEXT.into()),
            })
            .unwrap();
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(Duration::from_secs(10)).await;
    }
    assert!(
        !call.is_finished(),
        "valid progress must reset the 15s inactivity deadline"
    );
    tokio::time::advance(Duration::from_secs(TOTAL_TIMEOUT_SECS - 30)).await;

    let result = tokio::time::timeout(Duration::from_secs(2), call)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(tool_error_code(&result), "TIMEOUT");
    expect_cancel(&mut plugin, &request_id, "total deadline must still cancel").await;

    server_task.abort();
    broker_task.abort();
}

#[derive(Clone, Default)]
struct ProgressClient {
    frames: Arc<Mutex<Vec<ProgressNotificationParam>>>,
}

impl ClientHandler for ProgressClient {
    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: rmcp::service::NotificationContext<rmcp::RoleClient>,
    ) {
        self.frames.lock().await.push(params);
    }
}

#[tokio::test]
async fn bounded_progress_is_forwarded_without_design_content() {
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address).await;
    wait_for_session(&broker).await;
    let (server_io, client_io) = tokio::io::duplex(1024 * 1024);
    let server_task =
        tokio::spawn(async move { McpService::new(broker).serve(server_io).await.unwrap() });
    let client = ProgressClient::default().serve(client_io).await.unwrap();
    let token = ProgressToken(NumberOrString::String("progress-1".into()));
    let mut params = CallToolRequestParams::new("get_metadata").with_arguments(metadata_args());
    params.meta = Some(RequestMetaObject::with_progress_token(token.clone()));
    let call = tokio::spawn({
        let peer = client.peer().clone();
        async move { peer.call_tool(params).await }
    });
    let request_id = recv_requests(&mut plugin, 1).await.pop().unwrap();
    plugin
        .commands
        .send(PluginCommand::Progress {
            request_id: request_id.clone(),
            completed: 3,
            total: Some(9),
            message: Some(SENTINEL_NODE_TEXT.into()),
        })
        .unwrap();
    for _ in 0..50 {
        if !client.service().frames.lock().await.is_empty() {
            break;
        }
        tokio::task::yield_now().await;
    }
    let frames = client.service().frames.lock().await.clone();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].progress, 3.0);
    assert_eq!(frames[0].total, Some(9.0));
    let message = frames[0].message.clone().unwrap();
    assert!(
        matches!(
            message.as_str(),
            "queued" | "reading" | "serializing" | "encoding" | "completing"
        ),
        "progress must use a short phase enum, got {message}"
    );
    assert!(!message.contains(SENTINEL_NODE_TEXT));
    plugin
        .commands
        .send(PluginCommand::Respond(request_id))
        .unwrap();
    call.await.unwrap().unwrap();

    server_task.abort();
    broker_task.abort();
}

#[tokio::test]
async fn mcp_cancellation_reaches_the_plugin() {
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address).await;
    wait_for_session(&broker).await;
    let (client, server_task) = start_mcp(broker).await;
    let handle = client
        .peer()
        .send_request_with_option(
            ClientRequest::CallToolRequest(Request::new(
                CallToolRequestParams::new("get_metadata").with_arguments(metadata_args()),
            )),
            PeerRequestOptions::no_options(),
        )
        .await
        .unwrap();
    let request_id = recv_requests(&mut plugin, 1).await.pop().unwrap();
    let peer = handle.peer.clone();
    let id = handle.id.clone();
    peer.notify_cancelled(rmcp::model::CancelledNotificationParam::new(
        Some(id),
        Some("user cancelled".into()),
    ))
    .await
    .unwrap();
    match tokio::time::timeout(Duration::from_secs(2), plugin.inbound.recv()).await {
        Ok(Some(PluginInbound::Cancel(cancelled))) => assert_eq!(cancelled, request_id),
        other => panic!("MCP cancel must reach the plugin, got {other:?}"),
    }
    match handle.await_response().await {
        Ok(rmcp::model::ServerResult::CallToolResult(result)) => {
            assert_eq!(tool_error_code(&result), "CANCELLED");
        }
        Err(rmcp::ServiceError::Cancelled { .. }) => {}
        other => panic!("MCP cancel must resolve as CANCELLED, got {other:?}"),
    }

    server_task.abort();
    broker_task.abort();
}

#[tokio::test]
async fn disconnect_fails_inflight_and_late_responses_are_discarded() {
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address).await;
    wait_for_session(&broker).await;
    let (client, server_task) = start_mcp(broker.clone()).await;
    let call = tokio::spawn(call_metadata(client.peer().clone()));
    let request_id = recv_requests(&mut plugin, 1).await.pop().unwrap();
    plugin.commands.send(PluginCommand::Close).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(2), call)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(tool_error_code(&result), "CONNECTION_LOST");

    let plugin = connect_scripted_plugin(address).await;
    wait_for_session(&broker).await;
    plugin
        .commands
        .send(PluginCommand::Respond(request_id))
        .unwrap();
    let retry = tokio::spawn(call_metadata(client.peer().clone()));
    let mut plugin = plugin;
    let fresh = recv_requests(&mut plugin, 1).await.pop().unwrap();
    plugin.commands.send(PluginCommand::Respond(fresh)).unwrap();
    let retry = retry.await.unwrap().unwrap();
    assert_eq!(retry.is_error, Some(false));
    assert_eq!(
        retry.structured_content.as_ref().unwrap()["file"]["name"],
        "Checkout flow"
    );

    server_task.abort();
    broker_task.abort();
}

#[tokio::test(start_paused = true)]
async fn late_response_after_timeout_is_discarded() {
    let (address, broker, broker_task) = running_broker().await;
    let mut plugin = connect_scripted_plugin(address).await;
    wait_for_session(&broker).await;
    let (client, server_task) = start_mcp(broker).await;
    let first = tokio::spawn(call_metadata(client.peer().clone()));
    let stale_id = recv_requests(&mut plugin, 1).await.pop().unwrap();
    tokio::time::advance(Duration::from_secs(INACTIVITY_TIMEOUT_SECS)).await;
    let timed_out = first.await.unwrap().unwrap();
    assert_eq!(tool_error_code(&timed_out), "TIMEOUT");
    plugin
        .commands
        .send(PluginCommand::Respond(stale_id))
        .unwrap();
    tokio::task::yield_now().await;

    let second = tokio::spawn(call_metadata(client.peer().clone()));
    let fresh = recv_requests(&mut plugin, 1).await.pop().unwrap();
    plugin.commands.send(PluginCommand::Respond(fresh)).unwrap();
    let retry = second.await.unwrap().unwrap();
    assert_eq!(retry.is_error, Some(false));

    server_task.abort();
    broker_task.abort();
}
