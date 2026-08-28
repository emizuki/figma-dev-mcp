use std::net::SocketAddr;

use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits, PLUGIN_PROTOCOL_VERSION};
use figma_dev_mcp_protocol::domain::ConnectionId;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async, tungstenite::Message, tungstenite::client::IntoClientRequest,
};

async fn running_broker() -> (SocketAddr, Broker, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let config = BrokerConfig::for_test(Limits::reduced_for_test()).unwrap();
    let broker = Broker::new(config);
    let server = broker.clone();
    let task = tokio::spawn(async move {
        server.serve(listener).await.unwrap();
    });
    (address, broker, task)
}

fn request(
    address: SocketAddr,
    origin: Option<&str>,
) -> tokio_tungstenite::tungstenite::handshake::client::Request {
    let mut request = format!("ws://{address}/").into_client_request().unwrap();
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("Origin", origin.parse().unwrap());
    }
    request
}

#[tokio::test]
async fn accepts_exact_null_origin_and_rejects_missing_or_other_origins() {
    let (address, _broker, task) = running_broker().await;

    let (mut socket, _) = connect_async(request(address, Some("null"))).await.unwrap();
    socket.send(Message::Text(serde_json::to_string(&json!({
        "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION, "connectionId": "123e4567-e89b-42d3-a456-426614174000",
        "displayName": "File", "fileName": "File", "currentPage": {"id": "0:1", "name": "Page"},
        "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
    })).unwrap().into())).await.unwrap();
    assert!(socket.next().await.is_some());
    socket.close(None).await.unwrap();

    assert!(connect_async(request(address, None)).await.is_err());
    assert!(
        connect_async(request(address, Some("https://example.test")))
            .await
            .is_err()
    );
    task.abort();
}

#[tokio::test]
async fn first_frame_must_be_hello_and_protocol_mismatch_is_rejected() {
    let (address, _broker, task) = running_broker().await;
    let (mut socket, _) = connect_async(request(address, Some("null"))).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::to_string(&json!({"type": "ping", "nonce": 1}))
                .unwrap()
                .into(),
        ))
        .await
        .unwrap();
    assert!(matches!(
        socket.next().await,
        None | Some(Ok(Message::Close(_))) | Some(Err(_))
    ));

    let (mut socket, _) = connect_async(request(address, Some("null"))).await.unwrap();
    socket.send(Message::Text(serde_json::to_string(&json!({
        "type": "hello", "protocolVersion": "999", "connectionId": "123e4567-e89b-42d3-a456-426614174000",
        "displayName": "File", "fileName": "File", "currentPage": {"id": "0:1", "name": "Page"},
        "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
    })).unwrap().into())).await.unwrap();
    assert!(matches!(
        socket.next().await,
        None | Some(Ok(Message::Close(_))) | Some(Err(_))
    ));
    task.abort();
}

#[tokio::test]
async fn a_plugin_announcing_an_old_wire_version_is_refused() {
    // The check runs before the session is registered. What this pins is that
    // it is reachable with the version a shipped-but-stale plugin actually
    // announces, and that such a plugin never appears in the registry: the
    // failure mode being prevented is a silent session drop several requests
    // later, once a frame the old plugin cannot decode crosses the socket.
    let (address, broker, task) = running_broker().await;
    let (mut stale, _) = connect_async(request(address, Some("null"))).await.unwrap();
    stale.send(Message::Text(serde_json::to_string(&json!({
        "type": "hello", "protocolVersion": "1", "connectionId": "123e4567-e89b-42d3-a456-426614174000",
        "displayName": "Stale", "fileName": "Stale", "currentPage": {"id": "0:1", "name": "Page"},
        "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
    })).unwrap().into())).await.unwrap();
    // The socket ending is the broker's answer, and reaching it proves the
    // hello was read rather than still sitting in a buffer.
    assert!(matches!(
        stale.next().await,
        None | Some(Ok(Message::Close(_))) | Some(Err(_))
    ));
    assert_eq!(broker.live_file_count().await, 0);

    // The same body at the current version registers, so the refusal above is
    // the version and not some other defect in the frame.
    let (mut current, _) = connect_async(request(address, Some("null"))).await.unwrap();
    current
        .send(hello_frame(
            "123e4567-e89b-42d3-a456-426614174000",
            "Current",
        ))
        .await
        .unwrap();
    for _ in 0..20 {
        if broker.live_file_count().await == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert_eq!(broker.live_file_count().await, 1);
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn close_and_heartbeat_expiry_remove_the_registered_session() {
    let (address, broker, task) = running_broker().await;
    let (mut socket, _) = connect_async(request(address, Some("null"))).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::to_string(&json!({
                "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION, "connectionId": "123e4567-e89b-42d3-a456-426614174000",
                "displayName": "File", "fileName": "File", "currentPage": {"id": "0:1", "name": "Page"},
                "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
            }))
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
    for _ in 0..20 {
        if broker.live_file_count().await == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(broker.live_file_count().await, 1);
    socket.close(None).await.unwrap();
    for _ in 0..10 {
        if broker.live_file_count().await == 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(broker.live_file_count().await, 0);

    let (mut stale, _) = connect_async(request(address, Some("null"))).await.unwrap();
    stale
        .send(Message::Text(
            serde_json::to_string(&json!({
                "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION, "connectionId": "123e4567-e89b-42d3-a456-426614174001",
                "displayName": "Stale", "fileName": "Stale", "currentPage": {"id": "0:1", "name": "Page"},
                "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
            }))
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
    for _ in 0..20 {
        if broker.live_file_count().await == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(broker.live_file_count().await, 1);
    tokio::time::advance(std::time::Duration::from_millis(101)).await;
    for _ in 0..20 {
        if broker.live_file_count().await == 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(broker.live_file_count().await, 0);
    task.abort();
}

fn hello_frame(connection_id: &str, file_name: &str) -> Message {
    Message::Text(
        serde_json::to_string(&json!({
            "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION, "connectionId": connection_id,
            "displayName": file_name, "fileName": file_name,
            "currentPage": {"id": "0:1", "name": "Page"},
            "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
        }))
        .unwrap()
        .into(),
    )
}

fn metadata_request(request_id: &str) -> figma_dev_mcp_protocol::wire::Request {
    serde_json::from_value(json!({
        "requestId": request_id,
        "deadlineMs": 1000,
        "target": {},
        "operation": {"operation": "get_metadata", "input": {}}
    }))
    .unwrap()
}

#[tokio::test]
async fn wrong_socket_response_cannot_complete_a_real_pending_request() {
    let (address, broker, task) = running_broker().await;
    let (mut first, _) = connect_async(request(address, Some("null"))).await.unwrap();
    first
        .send(hello_frame("123e4567-e89b-42d3-a456-426614174000", "First"))
        .await
        .unwrap();
    let (mut second, _) = connect_async(request(address, Some("null"))).await.unwrap();
    second
        .send(hello_frame(
            "123e4567-e89b-42d3-a456-426614174001",
            "Second",
        ))
        .await
        .unwrap();
    for _ in 0..20 {
        if broker.live_file_count().await == 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }

    let first_id = ConnectionId::try_from("123e4567-e89b-42d3-a456-426614174000").unwrap();
    let mut call = broker
        .invoke(&first_id, metadata_request("request-1"))
        .await
        .unwrap();
    let wrong_response = serde_json::to_string(&json!({
        "type": "error", "requestId": "request-1",
        "error": {"code": "INTERNAL_ERROR", "retryable": false}
    }))
    .unwrap();
    second
        .send(Message::Text(wrong_response.into()))
        .await
        .unwrap();
    second.close(None).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        loop {
            if broker.live_file_count().await == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("wrong socket frame must be processed before the isolation assertion");
    tokio::task::yield_now().await;
    assert!(matches!(
        call.result.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    first
        .send(Message::Text(
            serde_json::to_string(&json!({
                "type": "error", "requestId": "request-1",
                "error": {"code": "INTERNAL_ERROR", "retryable": false}
            }))
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();
    assert!((&mut call.result).await.unwrap().is_err());
    task.abort();
}

#[tokio::test]
async fn broker_shutdown_and_deadlines_resolve_pending_requests() {
    let (address, broker, task) = running_broker().await;
    let (mut socket, _) = connect_async(request(address, Some("null"))).await.unwrap();
    socket
        .send(hello_frame("123e4567-e89b-42d3-a456-426614174000", "First"))
        .await
        .unwrap();
    for _ in 0..20 {
        if broker.live_file_count().await == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    let connection_id = ConnectionId::try_from("123e4567-e89b-42d3-a456-426614174000").unwrap();
    let mut timeout = broker
        .invoke(&connection_id, metadata_request("timeout"))
        .await
        .unwrap();
    let mut shutdown = broker
        .invoke(&connection_id, metadata_request("shutdown"))
        .await
        .unwrap();
    broker.shutdown().await;
    assert!((&mut timeout.result).await.unwrap().is_err());
    assert!((&mut shutdown.result).await.unwrap().is_err());
    task.abort();
}

#[tokio::test]
async fn cancellation_reaches_the_owning_plugin_and_resolves_once() {
    let (address, broker, task) = running_broker().await;
    let (mut socket, _) = connect_async(request(address, Some("null"))).await.unwrap();
    socket
        .send(hello_frame("123e4567-e89b-42d3-a456-426614174000", "First"))
        .await
        .unwrap();
    for _ in 0..20 {
        if broker.live_file_count().await == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    let connection_id = ConnectionId::try_from("123e4567-e89b-42d3-a456-426614174000").unwrap();
    let request_id = figma_dev_mcp_protocol::domain::RequestId::try_from("cancel-me").unwrap();
    let mut call = broker
        .invoke(&connection_id, metadata_request(request_id.as_str()))
        .await
        .unwrap();
    assert!(broker.cancel(&connection_id, &request_id).await);
    assert!((&mut call.result).await.unwrap().is_err());

    let mut saw_cancel = false;
    for _ in 0..4 {
        let Some(Ok(Message::Text(frame))) = socket.next().await else {
            break;
        };
        let frame: serde_json::Value = serde_json::from_str(&frame).unwrap();
        if frame.get("type") == Some(&json!("cancel"))
            && frame.get("requestId") == Some(&json!("cancel-me"))
        {
            saw_cancel = true;
            break;
        }
    }
    assert!(saw_cancel);
    task.abort();
}

#[tokio::test]
async fn write_failure_cleans_session_and_allows_same_connection_id_to_reconnect() {
    let (address, broker, task) = running_broker().await;
    let (mut socket, _) = connect_async(request(address, Some("null"))).await.unwrap();
    socket
        .send(hello_frame("123e4567-e89b-42d3-a456-426614174000", "First"))
        .await
        .unwrap();
    for _ in 0..20 {
        if broker.live_file_count().await == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    let connection_id = ConnectionId::try_from("123e4567-e89b-42d3-a456-426614174000").unwrap();
    let mut call = broker
        .invoke(&connection_id, metadata_request("reset-request"))
        .await
        .unwrap();
    // Force a TCP reset so the server's pending WebSocket write observes an error.
    #[allow(deprecated)]
    if let tokio_tungstenite::MaybeTlsStream::Plain(stream) = socket.get_mut() {
        stream.set_linger(Some(std::time::Duration::ZERO)).unwrap();
    }
    drop(socket);
    tokio::time::timeout(std::time::Duration::from_millis(100), async {
        loop {
            if broker.live_file_count().await == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("reset socket must be removed");
    assert!((&mut call.result).await.unwrap().is_err());

    let (mut replacement, _) = connect_async(request(address, Some("null"))).await.unwrap();
    replacement
        .send(hello_frame(
            "123e4567-e89b-42d3-a456-426614174000",
            "Replacement",
        ))
        .await
        .unwrap();
    for _ in 0..20 {
        if broker.live_file_count().await == 1 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert_eq!(broker.live_file_count().await, 1);
    task.abort();
}

#[tokio::test]
async fn shutdown_interrupts_a_tcp_peer_stalled_before_websocket_upgrade() {
    let (address, broker, task) = running_broker().await;
    let _stalled_peer = tokio::net::TcpStream::connect(address).await.unwrap();
    tokio::task::yield_now().await;

    broker.shutdown().await;
    tokio::time::timeout(std::time::Duration::from_millis(100), task)
        .await
        .expect("broker shutdown must not wait for a stalled upgrade")
        .unwrap();
}
