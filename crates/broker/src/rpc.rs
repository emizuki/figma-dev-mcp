use std::{net::SocketAddr, time::Duration};

use figma_dev_mcp_protocol::{
    domain::ProtocolVersion,
    error::{ErrorCode, ToolError},
    limits::MAX_ENVELOPE_BYTES,
    rpc::{
        FRONTEND_PROTOCOL_VERSION, FrontendHandshake, FrontendHello, FrontendToLeader,
        LeaderToFrontend, RpcRequestId, encode_frame,
    },
    wire::{BrokerCall, BrokerResult},
};
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::JoinSet,
};
use tokio_util::codec::LengthDelimitedCodec;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{Broker, BrokerClient, BrokerError, NormalizedProgress, OpenCall, ProgressPhase};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(2);

fn framed(stream: TcpStream) -> tokio_util::codec::Framed<TcpStream, LengthDelimitedCodec> {
    LengthDelimitedCodec::builder()
        .length_field_length(4)
        .big_endian()
        .max_frame_length(MAX_ENVELOPE_BYTES)
        .new_framed(stream)
}

fn body<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, BrokerError> {
    let frame = encode_frame(value).map_err(BrokerError::FrontendFrame)?;
    Ok(frame[4..].to_vec())
}

pub(crate) async fn serve(broker: Broker, listener: TcpListener) -> Result<(), BrokerError> {
    let mut frontends = JoinSet::new();
    loop {
        tokio::select! {
            _ = broker.state.shutdown.cancelled() => break,
            accepted = listener.accept() => {
                let (stream, peer) = accepted?;
                if !peer.ip().is_loopback() {
                    continue;
                }
                let broker = broker.clone();
                frontends.spawn(async move {
                    if let Err(error) = serve_frontend(broker, stream).await {
                        tracing::debug!(%error, "frontend RPC connection closed");
                    }
                });
            }
            Some(_) = frontends.join_next(), if !frontends.is_empty() => {}
        }
    }
    while frontends.join_next().await.is_some() {}
    Ok(())
}

async fn serve_frontend(broker: Broker, stream: TcpStream) -> Result<(), BrokerError> {
    let mut transport = framed(stream);
    let first = tokio::select! {
        _ = broker.state.shutdown.cancelled() => return Ok(()),
        frame = tokio::time::timeout(HANDSHAKE_TIMEOUT, transport.next()) => {
            frame.map_err(|_| BrokerError::FrontendHandshakeTimedOut)?
                .ok_or(BrokerError::FrontendFirstFrame)??
        }
    };
    let hello: FrontendHello = serde_json::from_slice(&first)?;
    if hello.protocol_version.as_str() != FRONTEND_PROTOCOL_VERSION {
        transport
            .send(
                body(&FrontendHandshake::Rejected {
                    error: ToolError::new(ErrorCode::ProtocolMismatch, false),
                })?
                .into(),
            )
            .await?;
        return Err(BrokerError::FrontendProtocolMismatch);
    }
    let Some(_lease) = broker.frontend_lease() else {
        return Ok(());
    };
    transport
        .send(body(&FrontendHandshake::Ready)?.into())
        .await?;
    let shutdown = broker.state.shutdown.clone();
    let client = BrokerClient::local(broker);
    let mut active = std::collections::HashMap::new();
    let mut calls = JoinSet::new();
    let mut terminal_error = None;
    let (progress_tx, mut progress_rx) = mpsc::channel::<(RpcRequestId, NormalizedProgress)>(32);

    let connection_result: Result<(), BrokerError> = async {
        'connection: loop {
            tokio::select! {
                _ = shutdown.cancelled() => break,
                frame = transport.next() => {
                    let Some(frame) = frame else { break };
                    let message: FrontendToLeader = serde_json::from_slice(&frame?)?;
                    match message {
                        FrontendToLeader::Request { rpc_request_id, call } => {
                            if active.contains_key(&rpc_request_id) {
                                terminal_error = Some(BrokerError::DuplicateFrontendRequest);
                                break 'connection;
                            }
                            let cancellation = CancellationToken::new();
                            active.insert(rpc_request_id.clone(), cancellation.clone());
                            let client = client.clone();
                            let progress_tx = progress_tx.clone();
                            calls.spawn(async move {
                                let result = match client.open(*call).await {
                                    Ok(mut open) => loop {
                                        tokio::select! {
                                            result = &mut open.result => {
                                                break result.map_err(|_| {
                                                    ToolError::new(ErrorCode::ConnectionLost, true)
                                                }).and_then(std::convert::identity);
                                            }
                                            frame = open.progress.recv() => {
                                                if let Some(frame) = frame {
                                                    let _ = progress_tx.send((rpc_request_id.clone(), frame)).await;
                                                }
                                            }
                                            _ = cancellation.cancelled() => {
                                                open.abort.cancel();
                                                if let (
                                                    Some(connection_id),
                                                    Some(request_id),
                                                    Some(broker),
                                                ) = (
                                                    &open.connection_id,
                                                    &open.request_id,
                                                    &open.owner,
                                                ) {
                                                    let _ = broker.cancel(connection_id, request_id).await;
                                                }
                                                break Err(ToolError::new(ErrorCode::Cancelled, false));
                                            }
                                        }
                                    },
                                    Err(error) => Err(error),
                                };
                                (rpc_request_id, result)
                            });
                        }
                        FrontendToLeader::Cancel { rpc_request_id } => {
                            if let Some(cancellation) = active.get(&rpc_request_id) {
                                cancellation.cancel();
                            }
                        }
                    }
                }
                Some((rpc_request_id, frame)) = progress_rx.recv() => {
                    let message = LeaderToFrontend::Progress {
                        rpc_request_id,
                        progress: figma_dev_mcp_protocol::rpc::RpcProgress {
                            completed: frame.completed,
                            total: frame.total,
                            message: figma_dev_mcp_protocol::domain::DisplayText::try_from(
                                frame.phase.as_str(),
                            )
                            .ok(),
                        },
                    };
                    if let Ok(body) = body(&message) {
                        transport.send(body.into()).await?;
                    }
                }
                completion = calls.join_next(), if !calls.is_empty() => {
                    let Some(Ok((rpc_request_id, result))) = completion else { continue };
                    active.remove(&rpc_request_id);
                    let message = match result {
                        Ok(result) => LeaderToFrontend::Response {
                            rpc_request_id: rpc_request_id.clone(),
                            result,
                        },
                        Err(error) => LeaderToFrontend::Error {
                            rpc_request_id: rpc_request_id.clone(),
                            error,
                        },
                    };
                    match body(&message) {
                        Ok(encoded) => transport.send(encoded.into()).await?,
                        Err(_) => {
                            let error = LeaderToFrontend::Error {
                                rpc_request_id,
                                error: ToolError::new(ErrorCode::LimitExceeded, false),
                            };
                            transport.send(body(&error)?.into()).await?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    .await;
    for cancellation in active.into_values() {
        cancellation.cancel();
    }
    while calls.join_next().await.is_some() {}
    if let Some(error) = terminal_error {
        Err(error)
    } else {
        connection_result
    }
}

#[derive(Clone, Debug)]
pub struct FrontendClient {
    commands: mpsc::Sender<ClientCommandMessage>,
}

#[derive(Debug)]
struct ClientCommand {
    rpc_request_id: RpcRequestId,
    call: BrokerCall,
    response: oneshot::Sender<Result<BrokerResult, ToolError>>,
    progress: mpsc::Sender<NormalizedProgress>,
}

#[derive(Debug)]
enum ClientCommandMessage {
    Request(ClientCommand),
    Cancel(RpcRequestId),
}

impl FrontendClient {
    pub async fn connect(address: SocketAddr) -> Result<Self, BrokerError> {
        let stream = TcpStream::connect(address).await?;
        Self::from_stream(stream).await
    }

    pub async fn from_stream(stream: TcpStream) -> Result<Self, BrokerError> {
        let mut transport = framed(stream);
        let frontend_id = RpcRequestId::try_from(Uuid::new_v4().to_string())
            .expect("UUID is a bounded RPC identifier");
        transport
            .send(
                body(&FrontendHello {
                    protocol_version: ProtocolVersion::try_from(FRONTEND_PROTOCOL_VERSION)
                        .expect("frontend protocol version is bounded"),
                    frontend_id,
                })?
                .into(),
            )
            .await?;
        let response = tokio::time::timeout(HANDSHAKE_TIMEOUT, transport.next())
            .await
            .map_err(|_| BrokerError::FrontendHandshakeTimedOut)?
            .ok_or(BrokerError::FrontendFirstFrame)??;
        match serde_json::from_slice::<FrontendHandshake>(&response)? {
            FrontendHandshake::Ready => {}
            FrontendHandshake::Rejected { .. } => {
                return Err(BrokerError::FrontendProtocolMismatch);
            }
        }
        let (commands, receiver) = mpsc::channel(16);
        tokio::spawn(client_loop(transport, receiver));
        Ok(Self { commands })
    }

    pub async fn open(&self, call: BrokerCall) -> Result<OpenCall, ToolError> {
        let rpc_request_id = RpcRequestId::try_from(Uuid::new_v4().to_string())
            .expect("UUID is a bounded RPC identifier");
        let (response, receiver) = oneshot::channel();
        let (progress_tx, progress) = mpsc::channel(8);
        self.commands
            .send(ClientCommandMessage::Request(ClientCommand {
                rpc_request_id: rpc_request_id.clone(),
                call,
                response,
                progress: progress_tx,
            }))
            .await
            .map_err(|_| ToolError::new(ErrorCode::ConnectionLost, true))?;
        let abort = CancellationToken::new();
        let abort_watch = abort.clone();
        let commands = self.commands.clone();
        let cancel_id = rpc_request_id.clone();
        tokio::spawn(async move {
            abort_watch.cancelled().await;
            let _ = commands.send(ClientCommandMessage::Cancel(cancel_id)).await;
        });
        Ok(OpenCall {
            result: receiver,
            progress,
            total_deadline: tokio::time::Instant::now()
                + std::time::Duration::from_secs(
                    figma_dev_mcp_protocol::limits::TOTAL_TIMEOUT_SECS,
                ),
            inactivity_timeout: std::time::Duration::from_secs(
                figma_dev_mcp_protocol::limits::INACTIVITY_TIMEOUT_SECS,
            ),
            connection_id: None,
            request_id: None,
            abort,
            owner: None,
        })
    }

    pub async fn cancel_rpc(&self, rpc_request_id: RpcRequestId) {
        let _ = self
            .commands
            .send(ClientCommandMessage::Cancel(rpc_request_id))
            .await;
    }

    pub async fn list_files(
        &self,
    ) -> Result<Vec<figma_dev_mcp_protocol::domain::LiveFile>, ToolError> {
        match self
            .call(BrokerCall::ListFiles {}, &CancellationToken::new())
            .await?
        {
            BrokerResult::Files { result } => Ok(result.files.as_slice().to_vec()),
            BrokerResult::Invocation { .. } => Err(ToolError::new(ErrorCode::InternalError, false)),
        }
    }

    pub async fn call(
        &self,
        call: BrokerCall,
        cancellation: &CancellationToken,
    ) -> Result<BrokerResult, ToolError> {
        let rpc_request_id = RpcRequestId::try_from(Uuid::new_v4().to_string())
            .expect("UUID is a bounded RPC identifier");
        let (response, receiver) = oneshot::channel();
        let (progress, _progress_rx) = mpsc::channel(8);
        self.commands
            .send(ClientCommandMessage::Request(ClientCommand {
                rpc_request_id: rpc_request_id.clone(),
                call,
                response,
                progress,
            }))
            .await
            .map_err(|_| ToolError::new(ErrorCode::ConnectionLost, true))?;
        tokio::select! {
            result = receiver => result
                .map_err(|_| ToolError::new(ErrorCode::ConnectionLost, true))?,
            _ = cancellation.cancelled() => {
                let _ = self.commands.send(ClientCommandMessage::Cancel(rpc_request_id)).await;
                Err(ToolError::new(ErrorCode::Cancelled, false))
            }
        }
    }

    /// Resolves when the RPC connection to the leader is gone.
    ///
    /// `client_loop` drops the command receiver when the leader's stream ends,
    /// which is the same event that makes every subsequent call fail with
    /// `ConnectionLost`. Awaiting it turns leader death into a signal the
    /// supervisor can act on instead of an error each call rediscovers.
    pub async fn closed(&self) {
        self.commands.closed().await;
    }
}

async fn client_loop(
    transport: tokio_util::codec::Framed<TcpStream, LengthDelimitedCodec>,
    mut commands: mpsc::Receiver<ClientCommandMessage>,
) {
    let (mut sink, mut stream) = transport.split();
    let mut pending = std::collections::HashMap::new();
    let mut progress = std::collections::HashMap::new();
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else { break };
                match command {
                    ClientCommandMessage::Request(command) => {
                        let message = FrontendToLeader::Request {
                            rpc_request_id: command.rpc_request_id.clone(),
                            call: Box::new(command.call),
                        };
                        pending.insert(command.rpc_request_id.clone(), command.response);
                        progress.insert(command.rpc_request_id, command.progress);
                        match body(&message) {
                            Ok(body) => {
                                if sink.send(body.into()).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    ClientCommandMessage::Cancel(rpc_request_id) => {
                        let message = FrontendToLeader::Cancel { rpc_request_id };
                        match body(&message) {
                            Ok(body) => {
                                if sink.send(body.into()).await.is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
            }
            frame = stream.next() => {
                let Some(Ok(frame)) = frame else { break };
                let Ok(message) = serde_json::from_slice::<LeaderToFrontend>(&frame) else { break };
                match message {
                    LeaderToFrontend::Response { rpc_request_id, result } => {
                        progress.remove(&rpc_request_id);
                        if let Some(response) = pending.remove(&rpc_request_id) {
                            let _ = response.send(Ok(result));
                        }
                    }
                    LeaderToFrontend::Error { rpc_request_id, error } => {
                        progress.remove(&rpc_request_id);
                        if let Some(response) = pending.remove(&rpc_request_id) {
                            let _ = response.send(Err(error));
                        }
                    }
                    LeaderToFrontend::Progress { rpc_request_id, progress: frame } => {
                        if let Some(sender) = progress.get(&rpc_request_id) {
                            let _ = sender.try_send(NormalizedProgress {
                                completed: frame.completed,
                                total: frame.total,
                                phase: ProgressPhase::from_message(
                                    frame.message.as_ref().map(|value| value.as_str()),
                                ),
                            });
                        }
                    }
                }
            }
        }
    }
    for (_, response) in pending {
        let _ = response.send(Err(ToolError::new(ErrorCode::ConnectionLost, true)));
    }
}
