use std::{sync::Arc, time::SystemTime};

use figma_dev_mcp_protocol::{
    error::ToolError,
    wire::{BrokerToPlugin, Ping, PluginToBroker},
};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinSet,
    time::{self, Instant},
};
use tokio_tungstenite::{
    accept_hdr_async_with_config,
    tungstenite::{
        Message,
        handshake::server::{ErrorResponse, Request, Response},
        http::StatusCode,
        protocol::WebSocketConfig,
    },
};
use uuid::Uuid;

use crate::{
    BrokerError, BrokerState, config::PLUGIN_PROTOCOL_VERSION, lifecycle::cleanup_socket,
    registry::Session,
};

pub async fn serve(state: Arc<BrokerState>, listener: TcpListener) -> Result<(), BrokerError> {
    let mut sockets = JoinSet::new();
    let mut terminal_error = None;
    let mut cleanup_tick = time::interval(std::time::Duration::from_millis(10));
    cleanup_tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = state.shutdown.cancelled() => break,
            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        terminal_error = Some(BrokerError::Io(error));
                        state.shutdown.cancel();
                        break;
                    }
                };
                if !peer.ip().is_loopback() {
                    continue;
                }
                let state = Arc::clone(&state);
                sockets.spawn(async move {
                    if let Err(error) = accept_socket(state, stream).await {
                        tracing::debug!(error = %error, "plugin socket closed during setup or transport");
                    }
                });
            }
            Some(_) = sockets.join_next(), if !sockets.is_empty() => {}
            _ = cleanup_tick.tick() => {
                let expired = state.pending.lock().await.take_expired(Instant::now());
                for (socket_id, request_id) in expired {
                    if let Some(outbound) = state.registry.lock().await.outbound_for_socket(socket_id) {
                        let _ = outbound.try_send(BrokerToPlugin::Cancel(
                            figma_dev_mcp_protocol::wire::Cancel { request_id },
                        ));
                    }
                }
            }
        }
    }
    state.pending.lock().await.shutdown();
    while sockets.join_next().await.is_some() {}
    terminal_error.map_or(Ok(()), Err)
}

async fn accept_socket(state: Arc<BrokerState>, stream: TcpStream) -> Result<(), BrokerError> {
    let limits = &state.config.limits;
    let config = WebSocketConfig::default()
        .max_message_size(Some(limits.max_message_bytes))
        .max_frame_size(Some(limits.max_frame_bytes));
    let socket = tokio::select! {
        _ = state.shutdown.cancelled() => return Ok(()),
        result = accept_hdr_async_with_config(stream, require_null_origin, Some(config)) => result?,
    };
    handle_socket(state, socket).await
}

#[allow(clippy::result_large_err)]
fn require_null_origin(request: &Request, response: Response) -> Result<Response, ErrorResponse> {
    if request
        .headers()
        .get("Origin")
        .map(|value| value.as_bytes())
        == Some(b"null".as_slice())
    {
        return Ok(response);
    }
    let mut response = ErrorResponse::new(Some("origin rejected".to_owned()));
    *response.status_mut() = StatusCode::FORBIDDEN;
    Err(response)
}

async fn handle_socket(
    state: Arc<BrokerState>,
    mut socket: tokio_tungstenite::WebSocketStream<TcpStream>,
) -> Result<(), BrokerError> {
    let first = tokio::select! {
        _ = state.shutdown.cancelled() => return Ok(()),
        first = socket.next() => first.ok_or(BrokerError::FirstFrameNotHello)??,
    };
    let Message::Text(text) = first else {
        return Err(BrokerError::FirstFrameNotHello);
    };
    let PluginToBroker::Hello(hello) = serde_json::from_str::<PluginToBroker>(&text)? else {
        return Err(BrokerError::FirstFrameNotHello);
    };
    if hello.protocol_version.as_str() != PLUGIN_PROTOCOL_VERSION {
        return Err(BrokerError::ProtocolMismatch);
    }

    let socket_id = Uuid::new_v4();
    let Some(_plugin_lease) = state.activity.plugin_lease() else {
        return Ok(());
    };
    let (outbound, mut outbound_rx) = mpsc::channel(state.config.limits.outbound_queue);
    state.registry.lock().await.insert(Session::from_hello(
        hello,
        socket_id,
        SystemTime::now(),
        outbound,
    ))?;
    state
        .queues
        .lock()
        .await
        .insert(socket_id, crate::SessionQueue::new());
    state.activity.changed();

    let mut heartbeat = time::interval(state.config.limits.heartbeat_interval);
    heartbeat.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    let stale_after = state.config.limits.stale_after;
    let mut last_seen = Instant::now();
    let mut nonce = 0_u64;

    let result = loop {
        tokio::select! {
            biased;
            _ = state.shutdown.cancelled() => break Ok(()),
            outbound = outbound_rx.recv() => {
                let Some(outbound) = outbound else { break Ok(()); };
                let encoded = match serde_json::to_string(&outbound) {
                    Ok(encoded) => encoded,
                    Err(error) => break Err(BrokerError::Json(error)),
                };
                let send_result = send_with_shutdown(
                    &state,
                    &mut socket,
                    Message::Text(encoded.into()),
                ).await;
                if let Err(error) = send_result {
                    break Err(error);
                }
            }
            incoming = socket.next() => {
                match incoming {
                    Some(Ok(message)) => {
                        if let Err(error) = handle_incoming(
                            &state,
                            socket_id,
                            message,
                            &mut socket,
                            &mut last_seen,
                        ).await {
                            break Err(error);
                        }
                    }
                    Some(Err(error)) => break Err(BrokerError::WebSocket(error)),
                    None => break Ok(()),
                }
            }
            _ = heartbeat.tick() => {
                nonce = nonce.wrapping_add(1);
                let ping = BrokerToPlugin::Ping(Ping { nonce });
                let encoded = match serde_json::to_string(&ping) {
                    Ok(encoded) => encoded,
                    Err(error) => break Err(BrokerError::Json(error)),
                };
                let send_result = send_with_shutdown(
                    &state,
                    &mut socket,
                    Message::Text(encoded.into()),
                ).await;
                if let Err(error) = send_result {
                    break Err(error);
                }
            }
            _ = time::sleep_until(last_seen + stale_after) => {
                break Err(BrokerError::HeartbeatExpired);
            }
        }
    };

    cleanup_socket(&state, socket_id).await;
    result
}

async fn send_with_shutdown(
    state: &BrokerState,
    socket: &mut tokio_tungstenite::WebSocketStream<TcpStream>,
    message: Message,
) -> Result<(), BrokerError> {
    tokio::select! {
        _ = state.shutdown.cancelled() => Ok(()),
        result = socket.send(message) => result.map_err(BrokerError::from),
    }
}

async fn handle_incoming(
    state: &BrokerState,
    socket_id: Uuid,
    message: Message,
    socket: &mut tokio_tungstenite::WebSocketStream<TcpStream>,
    last_seen: &mut Instant,
) -> Result<(), BrokerError> {
    match message {
        Message::Text(text) => {
            let message = serde_json::from_str::<PluginToBroker>(&text)?;
            *last_seen = Instant::now();
            state
                .registry
                .lock()
                .await
                .touch_socket(socket_id, *last_seen);
            match message {
                PluginToBroker::Hello(_) => return Err(BrokerError::SecondHello),
                PluginToBroker::Progress(progress) => {
                    if !state.pending.lock().await.note_progress(
                        socket_id,
                        &progress.request_id,
                        &progress,
                    ) {
                        tracing::debug!(
                            socket_id = %socket_id,
                            request_id = %progress.request_id,
                            "discarding late or foreign progress"
                        );
                    }
                }
                PluginToBroker::Response(response) => {
                    if !state.pending.lock().await.complete(
                        socket_id,
                        &response.request_id,
                        Ok(response.result),
                    ) {
                        tracing::debug!(
                            socket_id = %socket_id,
                            request_id = %response.request_id,
                            "discarding late or foreign response"
                        );
                    }
                }
                PluginToBroker::Error(error) => {
                    let result: Result<_, ToolError> = Err(error.error.into());
                    if !state
                        .pending
                        .lock()
                        .await
                        .complete(socket_id, &error.request_id, result)
                    {
                        tracing::debug!(
                            socket_id = %socket_id,
                            request_id = %error.request_id,
                            "discarding late or foreign error"
                        );
                    }
                }
                PluginToBroker::Pong(_) => {}
            }
        }
        Message::Pong(_) => {
            *last_seen = Instant::now();
            state
                .registry
                .lock()
                .await
                .touch_socket(socket_id, *last_seen);
        }
        Message::Ping(payload) => {
            *last_seen = Instant::now();
            state
                .registry
                .lock()
                .await
                .touch_socket(socket_id, *last_seen);
            send_with_shutdown(state, socket, Message::Pong(payload)).await?;
        }
        Message::Close(_) => return Err(BrokerError::Closed),
        Message::Binary(_) | Message::Frame(_) => return Err(BrokerError::NonTextProtocolFrame),
    }
    Ok(())
}
