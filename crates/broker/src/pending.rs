use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use figma_dev_mcp_protocol::{
    domain::RequestId,
    error::{ErrorCode, ToolError},
    limits::INACTIVITY_TIMEOUT_SECS,
    wire::{Progress, ReadResult},
};
use tokio::{sync::mpsc, sync::oneshot, time::Instant};
use uuid::Uuid;

use crate::queue::QueueTicket;

pub type PendingResult = Result<ReadResult, ToolError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressPhase {
    Queued,
    Reading,
    Serializing,
    Encoding,
    Completing,
}

impl ProgressPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Reading => "reading",
            Self::Serializing => "serializing",
            Self::Encoding => "encoding",
            Self::Completing => "completing",
        }
    }

    pub fn from_message(message: Option<&str>) -> Self {
        match message {
            Some("queued") => Self::Queued,
            Some("serializing") => Self::Serializing,
            Some("encoding") => Self::Encoding,
            Some("completing") => Self::Completing,
            _ => Self::Reading,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedProgress {
    pub completed: u32,
    pub total: Option<u32>,
    pub phase: ProgressPhase,
}

pub fn normalize_progress(progress: &Progress) -> NormalizedProgress {
    NormalizedProgress {
        completed: progress.completed,
        total: progress.total,
        phase: ProgressPhase::from_message(progress.message.as_ref().map(|value| value.as_str())),
    }
}

pub struct PendingAdmission {
    pub socket_id: Uuid,
    pub request_id: RequestId,
    pub started_at: SystemTime,
    pub total_deadline: Instant,
    pub inactivity_deadline: Instant,
    pub progress: Option<mpsc::Sender<NormalizedProgress>>,
    pub ticket: Option<QueueTicket>,
}

#[derive(Debug)]
pub struct PendingRequest {
    pub socket_id: Uuid,
    pub request_id: RequestId,
    pub started_at: SystemTime,
    pub total_deadline: Instant,
    pub inactivity_deadline: Instant,
    sender: oneshot::Sender<PendingResult>,
    progress: Option<mpsc::Sender<NormalizedProgress>>,
    _ticket: Option<QueueTicket>,
}

#[derive(Debug, Default)]
pub struct PendingMap {
    requests: HashMap<(Uuid, RequestId), PendingRequest>,
}

impl PendingMap {
    pub fn insert(
        &mut self,
        socket_id: Uuid,
        request_id: RequestId,
        started_at: SystemTime,
        deadline: Instant,
    ) -> Result<oneshot::Receiver<PendingResult>, PendingError> {
        self.admit(PendingAdmission {
            socket_id,
            request_id,
            started_at,
            total_deadline: deadline,
            inactivity_deadline: deadline,
            progress: None,
            ticket: None,
        })
    }

    pub fn admit(
        &mut self,
        admission: PendingAdmission,
    ) -> Result<oneshot::Receiver<PendingResult>, PendingError> {
        let key = (admission.socket_id, admission.request_id.clone());
        if self.requests.contains_key(&key) {
            return Err(PendingError::DuplicateRequest);
        }
        let (sender, receiver) = oneshot::channel();
        self.requests.insert(
            key,
            PendingRequest {
                socket_id: admission.socket_id,
                request_id: admission.request_id,
                started_at: admission.started_at,
                total_deadline: admission.total_deadline,
                inactivity_deadline: admission.inactivity_deadline,
                sender,
                progress: admission.progress,
                _ticket: admission.ticket,
            },
        );
        Ok(receiver)
    }

    pub fn contains(&self, socket_id: Uuid, request_id: &RequestId) -> bool {
        self.requests.contains_key(&(socket_id, request_id.clone()))
    }

    pub fn note_progress(
        &mut self,
        socket_id: Uuid,
        request_id: &RequestId,
        progress: &Progress,
    ) -> bool {
        let Some(pending) = self.requests.get_mut(&(socket_id, request_id.clone())) else {
            return false;
        };
        pending.inactivity_deadline = Instant::now() + Duration::from_secs(INACTIVITY_TIMEOUT_SECS);
        let normalized = normalize_progress(progress);
        if let Some(sender) = &pending.progress {
            let _ = sender.try_send(normalized);
        }
        true
    }

    pub fn complete(
        &mut self,
        socket_id: Uuid,
        request_id: &RequestId,
        result: PendingResult,
    ) -> bool {
        let Some(pending) = self.requests.remove(&(socket_id, request_id.clone())) else {
            return false;
        };
        let _ = pending.sender.send(result);
        true
    }

    pub fn cancel(&mut self, socket_id: Uuid, request_id: &RequestId) -> bool {
        self.complete(
            socket_id,
            request_id,
            Err(ToolError::new(ErrorCode::Cancelled, false)),
        )
    }

    pub fn remove_socket(&mut self, socket_id: Uuid) -> usize {
        self.remove_matching(
            |pending| pending.socket_id == socket_id,
            ErrorCode::ConnectionLost,
        )
    }

    pub fn expire(&mut self, now: Instant) -> usize {
        self.take_expired(now).len()
    }

    pub fn take_expired(&mut self, now: Instant) -> Vec<(Uuid, RequestId)> {
        let keys: Vec<_> = self
            .requests
            .iter()
            .filter_map(|(key, pending)| {
                (pending.total_deadline <= now || pending.inactivity_deadline <= now)
                    .then_some(key.clone())
            })
            .collect();
        for key in &keys {
            if let Some(pending) = self.requests.remove(key) {
                let _ = pending
                    .sender
                    .send(Err(ToolError::new(ErrorCode::Timeout, true)));
            }
        }
        keys
    }

    pub fn shutdown(&mut self) -> usize {
        self.remove_matching(|_| true, ErrorCode::ConnectionLost)
    }

    fn remove_matching(
        &mut self,
        predicate: impl Fn(&PendingRequest) -> bool,
        code: ErrorCode,
    ) -> usize {
        let keys: Vec<_> = self
            .requests
            .iter()
            .filter_map(|(key, pending)| predicate(pending).then_some(key.clone()))
            .collect();
        for key in &keys {
            if let Some(pending) = self.requests.remove(key) {
                let _ = pending.sender.send(Err(ToolError::new(code, true)));
            }
        }
        keys.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PendingError {
    #[error("request is already pending on this socket")]
    DuplicateRequest,
}
