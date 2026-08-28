use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use figma_dev_mcp_protocol::{
    domain::{ConnectionId, DisplayText, ObservationWindow, RequestId, ReturnedList},
    error::{ErrorCode, ToolError},
    wire::{BrokerCall, BrokerResult, Request, RequestTarget},
};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{Broker, FrontendClient, NormalizedProgress};

#[derive(Debug)]
pub struct OpenCall {
    pub result: tokio::sync::oneshot::Receiver<Result<BrokerResult, ToolError>>,
    pub progress: mpsc::Receiver<NormalizedProgress>,
    pub total_deadline: Instant,
    pub inactivity_timeout: Duration,
    pub connection_id: Option<ConnectionId>,
    pub request_id: Option<RequestId>,
    pub abort: CancellationToken,
    /// The `Broker` this call was opened against, or `None` when it was opened
    /// over RPC against a remote leader.
    ///
    /// Cancellation must reach *this* broker. The client's backend can be
    /// swapped mid-call by a re-election, and a `Cancel` sent to a different
    /// broker finds nothing in its registry, silently does nothing, and leaves
    /// the plugin executing an abandoned request until its own deadline.
    pub owner: Option<Broker>,
}

#[derive(Clone, Debug)]
pub(crate) enum Backend {
    Local(Broker),
    Remote(FrontendClient),
}

impl Backend {
    pub(crate) fn local(broker: Broker) -> Self {
        Self::Local(broker)
    }

    pub(crate) fn remote(client: FrontendClient) -> Self {
        Self::Remote(client)
    }
}

/// A broker handle whose backend can be replaced while calls are in flight.
///
/// The backend changes when the process changes role — a follower whose leader
/// died re-elects and may become the leader itself. Holding the client rather
/// than the backend is what lets the stdio MCP service survive that transition.
#[derive(Clone, Debug)]
pub struct BrokerClient {
    backend: Arc<RwLock<Option<Backend>>>,
}

impl BrokerClient {
    pub fn local(broker: Broker) -> Self {
        Self::new(Backend::local(broker))
    }

    pub fn remote(client: FrontendClient) -> Self {
        Self::new(Backend::remote(client))
    }

    /// A client with no backend yet.
    ///
    /// The process holds one of these between start and its first election, so
    /// the MCP service can be up and answering before a leader exists. Calls
    /// made in that window fail retryably rather than hanging, which is the
    /// whole reason the first election is no longer allowed to block startup.
    pub fn unattached() -> Self {
        Self {
            backend: Arc::new(RwLock::new(None)),
        }
    }

    pub(crate) fn new(backend: Backend) -> Self {
        Self {
            backend: Arc::new(RwLock::new(Some(backend))),
        }
    }

    /// Replace the backend. In-flight calls already hold their own clone and
    /// run to completion against the old backend; new calls see the new one.
    pub(crate) fn install(&self, backend: Backend) {
        *self
            .backend
            .write()
            .expect("broker client backend lock poisoned") = Some(backend);
    }

    /// Drop the current backend, returning to the unattached state.
    ///
    /// `install`'s counterpart. The supervisor calls this the moment a role
    /// dies, so that calls arriving before the next election fail retryably
    /// instead of being answered by a `Broker` that has been shut down —
    /// `Broker::shutdown` cancels its token and drains its pending map, but it
    /// does not clear the `SessionRegistry`, so a dead-but-installed broker
    /// keeps returning stale answers.
    ///
    /// Unlike `install` this is `pub`: installing the wrong backend would
    /// corrupt routing, but detaching can only turn an answer into a retryable
    /// error, so it is safe for anyone to call.
    pub fn detach(&self) {
        *self
            .backend
            .write()
            .expect("broker client backend lock poisoned") = None;
    }

    /// Clone the current backend out, if there is one. The guard is dropped
    /// before returning, so it can never be held across an await.
    fn backend(&self) -> Option<Backend> {
        self.backend
            .read()
            .expect("broker client backend lock poisoned")
            .clone()
    }

    /// The local `Broker`, when this process is currently the leader.
    ///
    /// `None` when this process is a follower, or when it has not yet elected.
    pub fn local_broker(&self) -> Option<Broker> {
        match self.backend()? {
            Backend::Local(broker) => Some(broker),
            Backend::Remote(_) => None,
        }
    }

    pub async fn open(&self, call: BrokerCall) -> Result<OpenCall, ToolError> {
        match self.backend() {
            Some(Backend::Local(broker)) => local_open(&broker, call).await,
            Some(Backend::Remote(client)) => client.open(call).await,
            None => Err(ToolError::new(ErrorCode::ConnectionLost, true)),
        }
    }

    pub async fn call(
        &self,
        call: BrokerCall,
        cancellation: &CancellationToken,
    ) -> Result<BrokerResult, ToolError> {
        // Read the backend cell exactly once, rather than going back through
        // `self.open` (which would read it a second time): this is still the
        // read that decides which backend opens the call. The cancellation
        // branch below no longer depends on this read staying in sync with
        // it — it cancels through `open.owner`, the `Broker` recorded at open
        // time, so a swap landing between the two reads can no longer strand
        // the `Cancel` frame on a backend that never opened the request.
        let broker = match self.backend() {
            Some(Backend::Remote(client)) => return client.call(call, cancellation).await,
            Some(Backend::Local(broker)) => broker,
            None => return Err(ToolError::new(ErrorCode::ConnectionLost, true)),
        };
        let mut open = local_open(&broker, call).await?;
        loop {
            tokio::select! {
                result = &mut open.result => {
                    return result.map_err(|_| ToolError::new(ErrorCode::ConnectionLost, true))?;
                }
                progress = open.progress.recv() => {
                    if progress.is_none() {
                        continue;
                    }
                }
                _ = cancellation.cancelled() => {
                    open.abort.cancel();
                    if let (Some(connection_id), Some(request_id), Some(owner)) =
                        (&open.connection_id, &open.request_id, &open.owner)
                    {
                        let _ = owner.cancel(connection_id, request_id).await;
                    }
                    return Err(ToolError::new(ErrorCode::Cancelled, false));
                }
            }
        }
    }
}

impl From<Broker> for BrokerClient {
    fn from(value: Broker) -> Self {
        Self::local(value)
    }
}

fn observation() -> ObservationWindow {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string();
    let timestamp = DisplayText::try_from(now).expect("epoch timestamp is bounded");
    ObservationWindow {
        started_at: timestamp.clone(),
        completed_at: timestamp,
    }
}

async fn local_open(broker: &Broker, call: BrokerCall) -> Result<OpenCall, ToolError> {
    match call {
        BrokerCall::ListFiles {} => {
            let files = ReturnedList::try_from(broker.list_files().await)
                .map_err(|_| ToolError::new(ErrorCode::LimitExceeded, false))?;
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let (_, progress) = mpsc::channel(1);
            let _ = sender.send(Ok(BrokerResult::Files {
                result: figma_dev_mcp_protocol::domain::ListFilesResult {
                    files,
                    truncated: false,
                    truncation: None,
                    observation: observation(),
                },
            }));
            Ok(OpenCall {
                result: receiver,
                progress,
                total_deadline: Instant::now(),
                inactivity_timeout: Duration::from_secs(0),
                connection_id: None,
                request_id: None,
                abort: CancellationToken::new(),
                owner: None,
            })
        }
        BrokerCall::Invoke {
            connection_id,
            invocation,
        } => {
            let connection_id = broker.resolve_connection(connection_id.as_ref()).await?;
            let request_id = RequestId::try_from(Uuid::new_v4().to_string())
                .expect("UUID is a bounded plugin request identifier");
            broker
                .invoke(
                    &connection_id,
                    Request {
                        request_id,
                        deadline_ms: figma_dev_mcp_protocol::limits::TOTAL_TIMEOUT_SECS * 1_000,
                        target: RequestTarget { file_key: None },
                        operation: invocation.operation,
                    },
                )
                .await
        }
    }
}
