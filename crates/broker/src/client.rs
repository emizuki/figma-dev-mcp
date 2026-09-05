use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use figma_dev_mcp_protocol::{
    domain::{ConnectionId, DisplayText, ObservationWindow, RequestId, ReturnedList},
    error::{ErrorCode, ToolError},
    limits::BACKEND_READY_MS,
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
    /// The task watching `abort` for a remote call, so it can be stopped when
    /// it can no longer be needed.
    ///
    /// `FrontendClient::open` spawns a task that waits on `abort` and then
    /// sends a `Cancel` to the leader. On the success path `abort` is never
    /// cancelled, and that task holds a clone of the token, so dropping this
    /// `OpenCall` does not resolve its future — it would wait forever. `None`
    /// for locally-served calls, which cancel through `owner` instead.
    pub watcher: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for OpenCall {
    fn drop(&mut self) {
        // Only abort a watcher that can no longer do its job. If the token was
        // cancelled, the watcher is on its way to sending the `Cancel` frame —
        // and on the remote path it is the only thing that can send it — so
        // aborting here would swallow a real cancellation. It ends on its own
        // once that send completes.
        if let Some(watcher) = self.watcher.take()
            && !self.abort.is_cancelled()
        {
            watcher.abort();
        }
    }
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
    // Fires on every `install`. `open`/`call` wait on this rather than polling,
    // so a call that arrives before the first election is served the moment the
    // backend lands instead of failing in zero milliseconds.
    installed: tokio::sync::watch::Sender<u64>,
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
    /// the MCP service can be up and answering before a leader exists. A call
    /// made in that window waits up to `BACKEND_READY_MS` for the election to
    /// install a backend and then fails retryably — it never hangs, which is
    /// what lets the first election stay off the startup path.
    pub fn unattached() -> Self {
        Self {
            backend: Arc::new(RwLock::new(None)),
            installed: tokio::sync::watch::Sender::new(0),
        }
    }

    pub(crate) fn new(backend: Backend) -> Self {
        Self {
            backend: Arc::new(RwLock::new(Some(backend))),
            installed: tokio::sync::watch::Sender::new(0),
        }
    }

    /// Replace the backend. In-flight calls already hold their own clone and
    /// run to completion against the old backend; new calls see the new one.
    pub(crate) fn install(&self, backend: Backend) {
        *self
            .backend
            .write()
            .expect("broker client backend lock poisoned") = Some(backend);
        self.installed.send_modify(|generation| *generation += 1);
    }

    /// Swap in a local backend, for tests that need to prove a call cancels
    /// through the `Broker` it was opened against rather than whichever one is
    /// current.
    ///
    /// `install`'s public face, deliberately narrowed: it takes a `Broker` rather
    /// than a `Backend`, so the backend enum stays private to this crate. Only
    /// tests should call it — the supervisor installs through `install_role`,
    /// which keeps `self.role` and this cell in step. A stray call from anywhere
    /// else desynchronises them, which is the defect that mutator pair exists to
    /// prevent.
    ///
    /// This is the third `#[doc(hidden)]` test-only entry point in this crate,
    /// after `detach` and `Supervisor::start`. If a fourth is ever wanted, the
    /// right answer is probably a `test-support` feature rather than a fourth
    /// door.
    #[doc(hidden)]
    pub fn install_local(&self, broker: Broker) {
        self.install(Backend::local(broker));
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
    /// Only the supervisor should call this: `supervise` installs a new
    /// backend only after `role.death()` fires, so nothing re-fills the cell
    /// until the current role actually dies. A stray `detach()` while a role
    /// is still alive leaves every call in the process waiting out
    /// `BACKEND_READY_MS` and then returning `ConnectionLost { retryable:
    /// true }` forever — "retryable" becomes a lie told indefinitely, which is
    /// the same confident-wrong-answer class this type exists to remove, just
    /// inverted, and now slower besides.
    ///
    /// `pub` only so the integration test that proves the detached state
    /// stops answering can reach it directly; `#[doc(hidden)]` keeps it out
    /// of the crate's public-facing docs, the way `Supervisor::start` is
    /// hidden for the same "public only for a test" reason.
    #[doc(hidden)]
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

    /// The current backend, waiting up to `BACKEND_READY_MS` for the first
    /// election to install one.
    ///
    /// Returns immediately once a backend exists, which is every call after
    /// startup. Only the calls racing the first election pay anything, and the
    /// measured race is ~80µs.
    async fn backend_ready(&self) -> Option<Backend> {
        if let Some(backend) = self.backend() {
            return Some(backend);
        }
        let mut changed = self.installed.subscribe();
        let deadline = Duration::from_millis(BACKEND_READY_MS);
        // The backend may land between the read above and the subscribe, so the
        // loop re-reads rather than trusting the notification alone.
        tokio::time::timeout(deadline, async {
            loop {
                if changed.changed().await.is_err() {
                    return None;
                }
                if let Some(backend) = self.backend() {
                    return Some(backend);
                }
            }
        })
        .await
        .ok()
        .flatten()
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
        match self.backend_ready().await {
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
        let broker = match self.backend_ready().await {
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
                watcher: None,
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
