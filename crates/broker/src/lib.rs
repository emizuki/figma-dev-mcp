//! Transport broker for `figma-dev-mcp`.

use std::sync::Arc;
use std::time::Duration;

use figma_dev_mcp_protocol::{
    domain::{ConnectionId, LiveFile, RequestId},
    error::{ErrorCode, ToolError},
    limits::{INACTIVITY_TIMEOUT_SECS, TOTAL_TIMEOUT_SECS},
    wire::{BrokerToPlugin, Request},
};
use tokio::{
    net::TcpListener,
    sync::{Mutex, mpsc, oneshot},
    time::{Duration as TokioDuration, Instant},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
mod client;
pub mod config;
mod election;
mod lifecycle;
pub mod pending;
pub mod queue;
pub mod registry;
mod rpc;
mod supervisor;
mod ws;

pub use client::{BrokerClient, OpenCall};
pub use config::{BrokerConfig, Limits, PLUGIN_PROTOCOL_VERSION};
pub use election::{ElectionError, ElectionOutcome, FollowerElection, LeaderElection, elect};
pub use lifecycle::FrontendLease;
pub use pending::{
    NormalizedProgress, PendingError, PendingMap, PendingResult, ProgressPhase, normalize_progress,
};
pub use queue::{QueueError, QueueTicket, SessionQueue};
pub use registry::{RegistryError, RouteError, Selection, Session, SessionRegistry};
pub use rpc::FrontendClient;
pub use supervisor::Supervisor;

#[derive(Clone, Debug)]
pub struct Broker {
    state: Arc<BrokerState>,
}

#[derive(Debug)]
pub(crate) struct BrokerState {
    pub(crate) config: BrokerConfig,
    pub(crate) registry: Mutex<SessionRegistry>,
    pub(crate) pending: Mutex<PendingMap>,
    pub(crate) queues: Mutex<std::collections::HashMap<Uuid, Arc<SessionQueue>>>,
    pub(crate) activity: Arc<lifecycle::Activity>,
    pub(crate) shutdown: CancellationToken,
}

impl Broker {
    pub fn new(config: BrokerConfig) -> Self {
        Self {
            state: Arc::new(BrokerState {
                config,
                registry: Mutex::new(SessionRegistry::new(Instant::now())),
                pending: Mutex::new(PendingMap::default()),
                queues: Mutex::new(std::collections::HashMap::new()),
                activity: Arc::new(lifecycle::Activity::default()),
                shutdown: CancellationToken::new(),
            }),
        }
    }

    pub async fn serve(self, listener: TcpListener) -> Result<(), BrokerError> {
        ws::serve(self.state, listener).await
    }

    pub async fn serve_frontends(self, listener: TcpListener) -> Result<(), BrokerError> {
        rpc::serve(self, listener).await
    }

    pub async fn invoke(
        &self,
        connection_id: &ConnectionId,
        request: Request,
    ) -> Result<OpenCall, ToolError> {
        let (socket_id, outbound) = self
            .state
            .registry
            .lock()
            .await
            .route_for(connection_id)
            .ok_or_else(|| ToolError::new(ErrorCode::ConnectionNotFound, false))?;
        let queue = self
            .state
            .queues
            .lock()
            .await
            .get(&socket_id)
            .cloned()
            .ok_or_else(|| ToolError::new(ErrorCode::ConnectionNotFound, false))?;
        let (ticket, wait) = queue
            .try_enqueue()
            .map_err(|_| ToolError::new(ErrorCode::LimitExceeded, true))?;
        let (in_flight, queued) = queue.snapshot();
        tracing::debug!(
            %connection_id,
            request_id = %request.request_id,
            in_flight,
            queue_depth = queued,
            inactivity_timeout_secs = INACTIVITY_TIMEOUT_SECS,
            total_timeout_secs = TOTAL_TIMEOUT_SECS,
            "admitted broker request"
        );

        let now = Instant::now();
        let total_deadline = now + TokioDuration::from_secs(TOTAL_TIMEOUT_SECS);
        let inactivity_timeout = TokioDuration::from_secs(INACTIVITY_TIMEOUT_SECS);
        let (progress_tx, progress_rx) = mpsc::channel(8);
        let request_id = request.request_id.clone();
        let receiver = self
            .state
            .pending
            .lock()
            .await
            .admit(pending::PendingAdmission {
                socket_id,
                request_id: request_id.clone(),
                started_at: std::time::SystemTime::now(),
                total_deadline,
                inactivity_deadline: now + inactivity_timeout,
                progress: Some(progress_tx),
                ticket: Some(ticket),
            })
            .map_err(|_| ToolError::new(ErrorCode::InternalError, false))?;

        let state = Arc::clone(&self.state);
        let dispatched_id = request_id.clone();
        tokio::spawn(async move {
            if wait.wait().await.is_err() {
                return;
            }
            let mut pending = state.pending.lock().await;
            if !pending.contains(socket_id, &dispatched_id) {
                return;
            }
            if let Err(error) = encoded_plugin_send(&outbound, BrokerToPlugin::Request(request)) {
                pending.complete(socket_id, &dispatched_id, Err(error));
            }
        });

        let (sender, mapped) = oneshot::channel();
        tokio::spawn(async move {
            let mapped_result =
                receiver
                    .await
                    .map_err(|_| ToolError::new(ErrorCode::ConnectionLost, true))
                    .and_then(|pending| {
                        pending.map(|value| {
                            figma_dev_mcp_protocol::wire::BrokerResult::Invocation { result: value }
                        })
                    });
            let _ = sender.send(mapped_result);
        });
        Ok(OpenCall {
            result: mapped,
            progress: progress_rx,
            total_deadline,
            inactivity_timeout,
            connection_id: Some(connection_id.clone()),
            request_id: Some(request_id),
            abort: CancellationToken::new(),
            owner: Some(self.clone()),
            watcher: None,
        })
    }

    pub async fn cancel(&self, connection_id: &ConnectionId, request_id: &RequestId) -> bool {
        let Some((socket_id, outbound)) = self.state.registry.lock().await.route_for(connection_id)
        else {
            return false;
        };
        if !self
            .state
            .pending
            .lock()
            .await
            .cancel(socket_id, request_id)
        {
            return false;
        }
        try_send_cancel(&outbound, request_id);
        true
    }

    pub async fn live_file_count(&self) -> usize {
        self.state.registry.lock().await.list_files().len()
    }

    pub fn frontend_lease(&self) -> Option<FrontendLease> {
        self.state.activity.frontend_lease()
    }

    /// Resolves when this broker has been shut down.
    ///
    /// For a listener task that is not yet serving a socket and so has no other
    /// way to notice — the IPv6 retry in `supervisor`. Everything else observes
    /// the same token from inside `ws::serve`/`rpc::serve`.
    pub(crate) async fn cancelled(&self) {
        self.state.shutdown.cancelled().await;
    }

    pub async fn wait_until_idle(&self, grace: Duration) {
        let mut changes = self.state.activity.subscribe();
        loop {
            let counts = self.state.activity.counts();
            if counts.frontends == 0 && counts.plugins == 0 {
                tokio::select! {
                    _ = tokio::time::sleep(grace) => {
                        if self.state.activity.begin_closing_if_idle() {
                            return;
                        }
                    }
                    result = changes.changed() => {
                        if result.is_err() {
                            return;
                        }
                    }
                }
            } else {
                if changes.changed().await.is_err() {
                    return;
                }
            }
        }
    }

    pub async fn list_files(&self) -> Vec<LiveFile> {
        self.state.registry.lock().await.list_files()
    }

    pub async fn resolve_connection(
        &self,
        connection_id: Option<&ConnectionId>,
    ) -> Result<ConnectionId, ToolError> {
        match self.state.registry.lock().await.select(connection_id) {
            Selection::One(session) => Ok(session.connection_id.clone()),
            Selection::None => Err(ToolError::new(ErrorCode::NoFigmaConnection, true)),
            Selection::Ambiguous => Err(ToolError::new(ErrorCode::AmbiguousConnection, false)),
            Selection::Missing => Err(ToolError::new(ErrorCode::ConnectionNotFound, false)),
        }
    }

    pub async fn bind_and_serve(self) -> Result<(), BrokerError> {
        let listener = TcpListener::bind(self.state.config.plugin_address).await?;
        self.serve(listener).await
    }

    pub async fn shutdown(&self) {
        self.state.shutdown.cancel();
        self.state.pending.lock().await.shutdown();
    }
}

fn encoded_plugin_send(
    outbound: &mpsc::Sender<BrokerToPlugin>,
    message: BrokerToPlugin,
) -> Result<(), ToolError> {
    let encoded = serde_json::to_vec(&message)
        .map_err(|_| ToolError::new(ErrorCode::InternalError, false))?;
    if encoded.len() > figma_dev_mcp_protocol::limits::MAX_ENVELOPE_BYTES {
        return Err(ToolError::new(ErrorCode::LimitExceeded, false));
    }
    outbound.try_send(message).map_err(|error| match error {
        mpsc::error::TrySendError::Full(_) => ToolError::new(ErrorCode::LimitExceeded, true),
        mpsc::error::TrySendError::Closed(_) => ToolError::new(ErrorCode::ConnectionLost, true),
    })
}

fn try_send_cancel(outbound: &mpsc::Sender<BrokerToPlugin>, request_id: &RequestId) {
    let cancel = BrokerToPlugin::Cancel(figma_dev_mcp_protocol::wire::Cancel {
        request_id: request_id.clone(),
    });
    if let Err(error) = outbound.try_send(cancel) {
        tracing::debug!(%request_id, %error, "cancel could not be delivered");
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("invalid JSON protocol frame: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error("the first plugin frame must be hello")]
    FirstFrameNotHello,
    #[error("plugin protocol version mismatch")]
    ProtocolMismatch,
    #[error("hello is immutable after registration")]
    SecondHello,
    #[error("plugin transport only accepts JSON text protocol frames")]
    NonTextProtocolFrame,
    #[error("plugin heartbeat expired")]
    HeartbeatExpired,
    #[error("plugin socket closed")]
    Closed,
    #[error("frontend RPC first frame must be hello")]
    FrontendFirstFrame,
    #[error("frontend RPC protocol version mismatch")]
    FrontendProtocolMismatch,
    #[error("frontend RPC handshake timed out")]
    FrontendHandshakeTimedOut,
    #[error("frontend RPC frame failed: {0}")]
    FrontendFrame(#[source] figma_dev_mcp_protocol::rpc::FrameError),
    #[error("frontend RPC tool call failed: {0:?}")]
    FrontendTool(ToolError),
    #[error("duplicate active frontend RPC request")]
    DuplicateFrontendRequest,
}
