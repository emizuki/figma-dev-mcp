use std::net::SocketAddr;
use std::time::Duration;

use figma_dev_mcp_broker::{Broker, BrokerConfig, Limits, PLUGIN_PROTOCOL_VERSION};
use figma_dev_mcp_protocol::domain::ConnectionId;
use figma_dev_mcp_protocol::limits::{HEARTBEAT_SECS, STALE_SESSION_SECS};
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

type PluginSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Production heartbeat timings, so a real-time control-frame test is not
/// racing the 100ms staleness window `Limits::reduced_for_test` sets.
async fn broker_with_production_heartbeat() -> (SocketAddr, Broker, tokio::task::JoinHandle<()>) {
    serving(Limits::production()).await
}

/// A two-second staleness window with a heartbeat an order of magnitude
/// shorter, for the virtual-time liveness tests. Both are far below the
/// production ceilings `Limits::checked` enforces, and neither is ever reached
/// in wall-clock time: those tests move the clock themselves.
async fn broker_with_short_staleness() -> (SocketAddr, Broker, tokio::task::JoinHandle<()>) {
    let limits = Limits::checked(
        64 * 1024,
        64 * 1024,
        4,
        Duration::from_millis(200),
        Duration::from_secs(2),
    )
    .expect("a 200ms heartbeat and a 2s staleness window are below production");
    serving(limits).await
}

async fn serving(limits: Limits) -> (SocketAddr, Broker, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(limits).unwrap());
    let server = broker.clone();
    let task = tokio::spawn(async move {
        server.serve(listener).await.unwrap();
    });
    (address, broker, task)
}

/// Connects a plugin socket, sends its hello, and waits — bounded, and
/// panicking here rather than somewhere confusing later — for the session to
/// appear in the registry. `yield_now` rather than `sleep` so the same helper
/// works under `start_paused`, where a sleep would move the clock the liveness
/// tests below are measuring.
async fn established_plugin_socket(
    address: SocketAddr,
    broker: &Broker,
    connection_id: &str,
    file_name: &str,
) -> PluginSocket {
    let (mut socket, _) = connect_async(request(address, Some("null"))).await.unwrap();
    socket
        .send(hello_frame(connection_id, file_name))
        .await
        .unwrap();
    for _ in 0..400 {
        if broker.live_file_count().await == 1 {
            return socket;
        }
        tokio::task::yield_now().await;
    }
    panic!("the plugin session must register before the test proceeds");
}

/// Reads frames until the broker asks the plugin for `request_id`, skipping the
/// heartbeat pings that share the socket. Reaching the frame is the proof the
/// session is still routable, which `live_file_count` alone would not give.
async fn expect_broker_request(socket: &mut PluginSocket, request_id: &str) {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(frame))) => {
                    let value: serde_json::Value = serde_json::from_str(&frame).unwrap();
                    if value["type"] == "request" && value["requestId"] == request_id {
                        return;
                    }
                }
                Some(Ok(_)) => {}
                other => panic!("socket ended before the request arrived: {other:?}"),
            }
        }
    })
    .await
    .expect("the broker must still route to this session");
}

/// `handle_incoming` refreshes a session's liveness from three separate arms —
/// text, pong and ping — and only the text arm is on the path the rest of the
/// suite drives. A WebSocket control frame is a legal thing for a plugin socket
/// to send at any time, and the two control arms answer for whether one is
/// taken or treated as a protocol violation. Turning either arm into
/// `NonTextProtocolFrame` left the whole workspace green.
///
/// This is the *acceptance* half only: it fails when the frame is refused, and
/// the assertion that actually fires is the session count — `broker.invoke`
/// and the frame read both still succeed against a session already on its way
/// out, so `expect_broker_request` loses the race with `cleanup_socket` and is
/// here for the routing claim rather than for detection. The *liveness* half —
/// an accepted frame that fails to refresh the clock — is a different property
/// and is pinned separately below.
#[tokio::test]
async fn an_unsolicited_control_pong_keeps_the_session_routable() {
    const CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174030";
    let (address, broker, task) = broker_with_production_heartbeat().await;
    let mut socket = established_plugin_socket(address, &broker, CONNECTION, "Control pong").await;

    socket
        .send(Message::Pong(b"unsolicited".to_vec().into()))
        .await
        .unwrap();

    let connection = ConnectionId::try_from(CONNECTION).unwrap();
    let _call = broker
        .invoke(&connection, metadata_request("pong-request"))
        .await
        .expect("a control pong must not unregister the session");
    expect_broker_request(&mut socket, "pong-request").await;
    assert_eq!(
        broker.live_file_count().await,
        1,
        "a control pong must not close the plugin session"
    );
    task.abort();
}

/// The ping arm of the same match, and it owes the sender a reply as well as a
/// live session: a plugin that pings and is answered with a closed socket has
/// no way to tell a healthy broker from a wedged one.
#[tokio::test]
async fn a_control_ping_is_answered_and_leaves_the_session_routable() {
    const CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174031";
    let (address, broker, task) = broker_with_production_heartbeat().await;
    let mut socket = established_plugin_socket(address, &broker, CONNECTION, "Control ping").await;

    socket
        .send(Message::Ping(b"figma".to_vec().into()))
        .await
        .unwrap();
    let payload = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Pong(payload))) => return payload,
                Some(Ok(_)) => {}
                other => panic!("socket ended before the pong arrived: {other:?}"),
            }
        }
    })
    .await
    .expect("a control ping must be answered");
    assert_eq!(payload.as_ref(), b"figma");

    let connection = ConnectionId::try_from(CONNECTION).unwrap();
    let _call = broker
        .invoke(&connection, metadata_request("ping-request"))
        .await
        .expect("a control ping must not unregister the session");
    expect_broker_request(&mut socket, "ping-request").await;
    assert_eq!(
        broker.live_file_count().await,
        1,
        "a control ping must not close the plugin session"
    );
    task.abort();
}

/// The liveness half of the pong arm, and the half the acceptance test above
/// cannot see: keep accepting the frame, keep returning `Ok`, and delete only
/// the two-line `last_seen`/`touch_socket` refresh, and the whole workspace
/// stays green. In production that is the more likely regression — a plugin
/// idle except for browser-level control frames is then reaped by
/// `HeartbeatExpired` after `stale_after`, with nothing to say so.
///
/// Virtual time, not wall-clock, so the assertion cannot lose a race with a
/// loaded machine and report a liveness regression that is not there: the clock
/// moves only where this test moves it, and each round opens a 500ms gap inside
/// a 2s staleness window. Six rounds carry the session 3s past its registration
/// — one and a half full windows — so a dropped refresh is reaped during round
/// five while a working one is never within 1.5s of the deadline.
#[tokio::test(start_paused = true)]
async fn control_pongs_alone_hold_a_session_across_staleness_windows() {
    const CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174032";
    let (address, broker, task) = broker_with_short_staleness().await;
    let mut socket = established_plugin_socket(address, &broker, CONNECTION, "Pong liveness").await;

    for round in 0..6 {
        socket
            .send(Message::Pong(b"keepalive".to_vec().into()))
            .await
            .unwrap();
        settle().await;
        tokio::time::advance(Duration::from_millis(500)).await;
        settle().await;
        assert_eq!(
            broker.live_file_count().await,
            1,
            "round {round}: a control pong must refresh the session's liveness clock"
        );
    }
    task.abort();
}

/// The ping arm of the same property. A ping is answered by the broker whether
/// or not the arm refreshes anything, so the reply assertion in the acceptance
/// test says nothing about liveness; this is what says it.
#[tokio::test(start_paused = true)]
async fn control_pings_alone_hold_a_session_across_staleness_windows() {
    const CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174033";
    let (address, broker, task) = broker_with_short_staleness().await;
    let mut socket = established_plugin_socket(address, &broker, CONNECTION, "Ping liveness").await;

    for round in 0..6 {
        socket
            .send(Message::Ping(b"keepalive".to_vec().into()))
            .await
            .unwrap();
        settle().await;
        tokio::time::advance(Duration::from_millis(500)).await;
        settle().await;
        assert_eq!(
            broker.live_file_count().await,
            1,
            "round {round}: a control ping must refresh the session's liveness clock"
        );
    }
    task.abort();
}

/// Lets the broker task run the frame that was just written without moving the
/// clock. Under `start_paused` tokio only auto-advances when the runtime has
/// nothing to do, so keeping a task ready is what stops the staleness timer
/// from firing before the frame has been read.
async fn settle() {
    for _ in 0..100 {
        tokio::task::yield_now().await;
    }
}

/// The text arm dispatches four accepted plugin frames — progress, response,
/// error and pong — and the error frame is the one nothing pinned. It looks
/// pinned: `wrong_socket_response_cannot_complete_a_real_pending_request` ends
/// with `assert!((&mut call.result).await.unwrap().is_err())` after sending an
/// error frame on the owning socket. But that broker runs on
/// `Limits::reduced_for_test`, so 100ms later the socket goes stale,
/// `cleanup_socket` fails the pending call with `CONNECTION_LOST`, and the
/// `is_err()` is satisfied by the session dying rather than by the frame
/// arriving. Skipping the `complete` call entirely leaves the workspace at
/// 264/0. The fix is to assert the code the plugin actually sent, on a broker
/// whose staleness window the test cannot outlive.
#[tokio::test]
async fn a_plugin_error_frame_completes_the_call_with_the_code_the_plugin_sent() {
    const CONNECTION: &str = "123e4567-e89b-42d3-a456-426614174034";
    let (address, broker, task) = broker_with_production_heartbeat().await;
    let mut socket = established_plugin_socket(address, &broker, CONNECTION, "Error frame").await;

    let connection = ConnectionId::try_from(CONNECTION).unwrap();
    let mut call = broker
        .invoke(&connection, metadata_request("error-request"))
        .await
        .expect("a live session must accept an invocation");
    expect_broker_request(&mut socket, "error-request").await;

    socket
        .send(Message::Text(
            serde_json::to_string(&json!({
                "type": "error",
                "requestId": "error-request",
                "error": {"code": "NODE_NOT_FOUND", "retryable": false}
            }))
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();

    let error = tokio::time::timeout(Duration::from_secs(5), &mut call.result)
        .await
        .expect("a plugin error frame must complete its pending call")
        .expect("the pending sender must not be dropped")
        .expect_err("an error frame completes the call as an error");
    assert_eq!(
        error.code(),
        figma_dev_mcp_protocol::error::ErrorCode::NodeNotFound,
        "the caller must see the plugin's own code, not a connection failure"
    );
    task.abort();
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

#[tokio::test]
async fn an_accepted_session_is_pinged_without_waiting_a_full_heartbeat() {
    // Task 3 moves the plugin's "Connected" status onto the first inbound
    // frame, so a session that is accepted but not spoken to for a whole
    // heartbeat would read "Connecting…" for seconds on a healthy link.
    // `ws.rs:138` builds the heartbeat with `time::interval`, whose first tick
    // completes immediately; this pins that so a later change to the heartbeat
    // cannot silently regress the plugin's status.
    //
    // Real time on purpose: a paused clock would make any delay free and the
    // assertion meaningless.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let limits = Limits::checked(
        64 * 1024,
        64 * 1024,
        4,
        Duration::from_secs(HEARTBEAT_SECS),
        Duration::from_secs(STALE_SESSION_SECS),
    )
    .unwrap();
    let broker = Broker::new(BrokerConfig::for_test(limits).unwrap());
    let server = broker.clone();
    let task = tokio::spawn(async move {
        server.serve(listener).await.unwrap();
    });

    let (mut socket, _) = connect_async(request(address, Some("null"))).await.unwrap();
    socket
        .send(Message::Text(
            serde_json::to_string(&json!({
                "type": "hello", "protocolVersion": PLUGIN_PROTOCOL_VERSION,
                "connectionId": "123e4567-e89b-42d3-a456-426614174000",
                "displayName": "File", "fileName": "File",
                "currentPage": {"id": "0:1", "name": "Page"},
                "editorType": "dev", "pluginVersion": "0.1.0", "capabilities": {}
            }))
            .unwrap()
            .into(),
        ))
        .await
        .unwrap();

    let frame = tokio::time::timeout(Duration::from_millis(500), socket.next())
        .await
        .expect("an accepted session must be spoken to well inside one 5s heartbeat");
    let Some(Ok(Message::Text(frame))) = frame else {
        panic!("the broker must send the session a ping frame rather than closing it");
    };
    let frame: serde_json::Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(
        frame.get("type"),
        Some(&json!("ping")),
        "an accepted session with no other traffic can only receive the heartbeat ping"
    );

    socket.close(None).await.unwrap();
    task.abort();
}
