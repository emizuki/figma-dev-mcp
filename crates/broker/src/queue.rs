//! One bounded admission queue per live Figma connection.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use figma_dev_mcp_protocol::limits::{MAX_IN_FLIGHT, MAX_QUEUE};
use tokio::sync::oneshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    Full,
    Cancelled,
}

#[derive(Debug)]
struct Waiter {
    id: u64,
    notify: oneshot::Sender<Result<(), QueueError>>,
}

#[derive(Debug)]
struct Inner {
    next_id: u64,
    in_flight: usize,
    waiting: VecDeque<Waiter>,
    active: HashMap<u64, ()>,
}

#[derive(Debug)]
pub struct SessionQueue {
    inner: Mutex<Inner>,
}

#[derive(Debug)]
pub struct QueueTicket {
    id: u64,
    queue: Arc<SessionQueue>,
}

pub struct DispatchWait {
    receiver: oneshot::Receiver<Result<(), QueueError>>,
}

impl SessionQueue {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                next_id: 1,
                in_flight: 0,
                waiting: VecDeque::new(),
                active: HashMap::new(),
            }),
        })
    }

    pub fn try_enqueue(self: &Arc<Self>) -> Result<(QueueTicket, DispatchWait), QueueError> {
        let mut inner = self.inner.lock().expect("session queue mutex");
        let id = inner.next_id;
        inner.next_id = inner.next_id.saturating_add(1);
        let (sender, receiver) = oneshot::channel();
        if inner.in_flight < MAX_IN_FLIGHT && inner.waiting.is_empty() {
            inner.in_flight += 1;
            inner.active.insert(id, ());
            let _ = sender.send(Ok(()));
        } else if inner.waiting.len() >= MAX_QUEUE {
            return Err(QueueError::Full);
        } else {
            inner.waiting.push_back(Waiter { id, notify: sender });
        }
        Ok((
            QueueTicket {
                id,
                queue: Arc::clone(self),
            },
            DispatchWait { receiver },
        ))
    }

    pub fn snapshot(&self) -> (usize, usize) {
        let inner = self.inner.lock().expect("session queue mutex");
        (inner.in_flight, inner.waiting.len())
    }

    fn release(&self, id: u64) {
        let mut inner = self.inner.lock().expect("session queue mutex");
        if let Some(index) = inner.waiting.iter().position(|waiter| waiter.id == id) {
            inner.waiting.remove(index);
            return;
        }
        if inner.active.remove(&id).is_some() {
            inner.in_flight = inner.in_flight.saturating_sub(1);
            promote(&mut inner);
        }
    }
}

fn promote(inner: &mut Inner) {
    while inner.in_flight < MAX_IN_FLIGHT {
        let Some(next) = inner.waiting.pop_front() else {
            return;
        };
        inner.in_flight += 1;
        inner.active.insert(next.id, ());
        if next.notify.send(Ok(())).is_err() {
            inner.in_flight = inner.in_flight.saturating_sub(1);
            inner.active.remove(&next.id);
        }
    }
}

impl DispatchWait {
    pub async fn wait(self) -> Result<(), QueueError> {
        self.receiver.await.unwrap_or(Err(QueueError::Cancelled))
    }
}

impl Drop for QueueTicket {
    fn drop(&mut self) {
        self.queue.release(self.id);
    }
}
