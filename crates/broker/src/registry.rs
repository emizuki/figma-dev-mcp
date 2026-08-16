use std::{collections::HashMap, time::SystemTime};

use figma_dev_mcp_protocol::{
    domain::{CapabilitySet, ConnectionId, DisplayText, FileKey, LiveFile, PageSummary},
    wire::{BrokerToPlugin, Hello},
};
use tokio::{sync::mpsc, time::Instant};
use uuid::Uuid;

#[derive(Debug)]
pub struct Session {
    pub connection_id: ConnectionId,
    pub socket_id: Uuid,
    pub display_name: DisplayText,
    pub file_key: Option<FileKey>,
    pub file_name: DisplayText,
    pub current_page: PageSummary,
    pub editor_type: DisplayText,
    pub plugin_version: DisplayText,
    pub capabilities: CapabilitySet,
    pub connected_at: SystemTime,
    pub last_seen_at: SystemTime,
    pub last_seen: Instant,
    pub outbound: mpsc::Sender<BrokerToPlugin>,
}

impl Session {
    pub fn from_hello(
        hello: Hello,
        socket_id: Uuid,
        connected_at: SystemTime,
        outbound: mpsc::Sender<BrokerToPlugin>,
    ) -> Self {
        Self {
            connection_id: hello.connection_id,
            socket_id,
            display_name: hello.display_name,
            file_key: hello.file_key,
            file_name: hello.file_name,
            current_page: hello.current_page,
            editor_type: hello.editor_type,
            plugin_version: hello.plugin_version,
            capabilities: hello.capabilities,
            connected_at,
            last_seen_at: connected_at,
            last_seen: Instant::now(),
            outbound,
        }
    }

    fn live_file(&self) -> LiveFile {
        LiveFile {
            connection_id: self.connection_id.clone(),
            display_name: self.display_name.clone(),
            file_key: self.file_key.clone(),
            file_name: self.file_name.clone(),
            current_page: self.current_page.clone(),
            editor_type: self.editor_type.clone(),
            capabilities: self.capabilities.clone(),
            connected_at: timestamp(self.connected_at),
            last_seen_at: timestamp(self.last_seen_at),
        }
    }
}

fn timestamp(value: SystemTime) -> DisplayText {
    let millis = value
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    DisplayText::try_from(millis.to_string()).expect("system timestamp is bounded")
}

#[derive(Debug, Clone, Copy)]
pub enum Selection<'a> {
    One(&'a Session),
    None,
    Ambiguous,
    Missing,
}

#[derive(Debug)]
pub struct SessionRegistry {
    sessions: HashMap<ConnectionId, Session>,
    sockets: HashMap<Uuid, ConnectionId>,
    insertion_time: Instant,
}

impl SessionRegistry {
    pub fn new(now: Instant) -> Self {
        Self {
            sessions: HashMap::new(),
            sockets: HashMap::new(),
            insertion_time: now,
        }
    }

    pub fn insert(&mut self, mut session: Session) -> Result<(), RegistryError> {
        if self.sessions.contains_key(&session.connection_id) {
            return Err(RegistryError::DuplicateConnectionId);
        }
        if self.sockets.contains_key(&session.socket_id) {
            return Err(RegistryError::DuplicateSocketId);
        }
        session.last_seen = self.insertion_time.max(Instant::now());
        self.sockets
            .insert(session.socket_id, session.connection_id.clone());
        self.sessions.insert(session.connection_id.clone(), session);
        Ok(())
    }

    pub fn select(&self, connection_id: Option<&ConnectionId>) -> Selection<'_> {
        if let Some(connection_id) = connection_id {
            return self
                .sessions
                .get(connection_id)
                .map_or(Selection::Missing, Selection::One);
        }
        let mut sessions = self.sessions.values();
        match (sessions.next(), sessions.next()) {
            (None, _) => Selection::None,
            (Some(session), None) => Selection::One(session),
            (Some(_), Some(_)) => Selection::Ambiguous,
        }
    }

    pub fn list_files(&self) -> Vec<LiveFile> {
        let mut sessions: Vec<_> = self.sessions.values().collect();
        sessions.sort_by(|left, right| {
            left.connected_at.cmp(&right.connected_at).then_with(|| {
                left.connection_id
                    .as_str()
                    .cmp(right.connection_id.as_str())
            })
        });
        sessions.into_iter().map(Session::live_file).collect()
    }

    pub fn touch_socket(&mut self, socket_id: Uuid, now: Instant) -> bool {
        let Some(connection_id) = self.sockets.get(&socket_id) else {
            return false;
        };
        let Some(session) = self.sessions.get_mut(connection_id) else {
            return false;
        };
        session.last_seen = now;
        session.last_seen_at = SystemTime::now();
        true
    }

    pub fn expire_stale(&mut self, cutoff: Instant) -> Vec<Uuid> {
        let stale: Vec<_> = self
            .sessions
            .values()
            .filter(|session| session.last_seen <= cutoff)
            .map(|session| session.socket_id)
            .collect();
        for socket_id in &stale {
            self.remove_socket(*socket_id);
        }
        stale
    }

    pub fn remove_socket(&mut self, socket_id: Uuid) -> bool {
        let Some(connection_id) = self.sockets.remove(&socket_id) else {
            return false;
        };
        self.sessions.remove(&connection_id).is_some()
    }

    pub fn outbound_for_socket(&self, socket_id: Uuid) -> Option<mpsc::Sender<BrokerToPlugin>> {
        let connection_id = self.sockets.get(&socket_id)?;
        self.sessions
            .get(connection_id)
            .map(|session| session.outbound.clone())
    }

    pub fn route_for(
        &self,
        connection_id: &ConnectionId,
    ) -> Option<(Uuid, mpsc::Sender<BrokerToPlugin>)> {
        self.sessions
            .get(connection_id)
            .map(|session| (session.socket_id, session.outbound.clone()))
    }

    pub fn try_send(
        &self,
        connection_id: &ConnectionId,
        message: BrokerToPlugin,
    ) -> Result<Uuid, RouteError> {
        let socket_id = self
            .sessions
            .get(connection_id)
            .map(|session| session.socket_id)
            .ok_or(RouteError::ConnectionNotFound)?;
        self.try_send_to(connection_id, socket_id, message)
    }

    pub fn try_send_to(
        &self,
        connection_id: &ConnectionId,
        expected_socket_id: Uuid,
        message: BrokerToPlugin,
    ) -> Result<Uuid, RouteError> {
        let session = self
            .sessions
            .get(connection_id)
            .ok_or(RouteError::ConnectionNotFound)?;
        if session.socket_id != expected_socket_id {
            return Err(RouteError::ConnectionChanged);
        }
        session
            .outbound
            .try_send(message)
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => RouteError::QueueFull,
                mpsc::error::TrySendError::Closed(_) => RouteError::ConnectionClosed,
            })?;
        Ok(session.socket_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("connection identifier is already live")]
    DuplicateConnectionId,
    #[error("socket identifier is already registered")]
    DuplicateSocketId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RouteError {
    #[error("connection is not live")]
    ConnectionNotFound,
    #[error("connection outbound queue is full")]
    QueueFull,
    #[error("connection outbound queue is closed")]
    ConnectionClosed,
    #[error("connection socket changed during routing")]
    ConnectionChanged,
}
