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
}

#[derive(Clone, Debug)]
pub enum BrokerClient {
    Local(Broker),
    Remote(FrontendClient),
}

impl BrokerClient {
    pub fn local(broker: Broker) -> Self {
        Self::Local(broker)
    }

    pub fn remote(client: FrontendClient) -> Self {
        Self::Remote(client)
    }

    pub async fn open(&self, call: BrokerCall) -> Result<OpenCall, ToolError> {
        match self {
            Self::Local(broker) => local_open(broker, call).await,
            Self::Remote(client) => client.open(call).await,
        }
    }

    pub async fn call(
        &self,
        call: BrokerCall,
        cancellation: &CancellationToken,
    ) -> Result<BrokerResult, ToolError> {
        match self {
            Self::Remote(client) => return client.call(call, cancellation).await,
            Self::Local(_) => {}
        }
        let mut open = self.open(call).await?;
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
                    if let (Some(connection_id), Some(request_id), Self::Local(broker)) =
                        (&open.connection_id, &open.request_id, self)
                    {
                        let _ = broker.cancel(connection_id, request_id).await;
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
