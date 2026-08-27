//! Role supervision.
//!
//! Leader election used to run once per process, which meant a follower whose
//! leader died stayed a follower forever and nothing ever reopened the plugin's
//! port. The supervisor holds the current role, watches its backend, and
//! re-enters election when that backend dies.

use std::time::Duration;

use tokio::{net::TcpListener, task::JoinSet};

use crate::{
    Broker, BrokerClient, BrokerConfig, BrokerError, ElectionError, ElectionOutcome,
    FrontendClient, FrontendLease, client::Backend, elect,
};

/// The role this process currently holds.
enum Role {
    Leader {
        broker: Broker,
        listeners: JoinSet<Result<(), BrokerError>>,
        lease: Option<FrontendLease>,
    },
    Follower {
        #[allow(dead_code, reason = "read by Role::death, wired up in Task 4")]
        client: FrontendClient,
    },
}

impl Role {
    /// Resolves when this role's backend dies.
    ///
    /// For a leader that is any listener task ending: the three tasks share one
    /// shutdown token, so the first to finish means the broker is gone. For a
    /// follower it is the RPC connection to the leader closing.
    #[allow(dead_code, reason = "wired up by Supervisor::supervise in Task 4")]
    async fn death(&mut self) -> Death {
        match self {
            Role::Leader { listeners, .. } => match listeners.join_next().await {
                Some(Ok(Ok(()))) => Death::ListenerStopped(None),
                Some(Ok(Err(error))) => Death::ListenerStopped(Some(error)),
                Some(Err(error)) => Death::ListenerPanicked(error.to_string()),
                None => Death::ListenerStopped(None),
            },
            Role::Follower { client } => {
                client.closed().await;
                Death::LeaderGone
            }
        }
    }
}

/// Why a backend stopped being usable. Carried only so the log line can say.
#[allow(dead_code, reason = "produced by Role::death, consumed in Task 4")]
enum Death {
    LeaderGone,
    ListenerStopped(Option<BrokerError>),
    ListenerPanicked(String),
}

impl std::fmt::Display for Death {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Death::LeaderGone => write!(formatter, "the leader's RPC connection closed"),
            Death::ListenerStopped(None) => write!(formatter, "a listener task stopped"),
            Death::ListenerStopped(Some(error)) => {
                write!(formatter, "a listener task failed: {error}")
            }
            Death::ListenerPanicked(error) => write!(formatter, "a listener task panicked: {error}"),
        }
    }
}

/// Owns the process's role and keeps it alive across leader deaths.
pub struct Supervisor {
    #[allow(dead_code, reason = "re-elected against in Supervisor::supervise, added in Task 4")]
    config: BrokerConfig,
    client: BrokerClient,
    role: Option<Role>,
}

impl Supervisor {
    /// Run the first election and enter the role it produces.
    ///
    /// This happens before the MCP service starts, exactly as it did before the
    /// supervisor existed, so there is no window where the service is up with no
    /// backend behind it.
    pub async fn start(config: BrokerConfig) -> Result<Self, ElectionError> {
        let outcome = elect(config.clone()).await?;
        let (role, backend) = enter_role(outcome)
            .await
            .ok_or(ElectionError::RoleUnavailable)?;
        Ok(Self {
            config,
            client: BrokerClient::new(backend),
            role: Some(role),
        })
    }

    /// A handle to the current backend. Clone it for the MCP service; it stays
    /// valid across every role change.
    pub fn client(&self) -> BrokerClient {
        self.client.clone()
    }

    pub fn is_leader(&self) -> bool {
        matches!(self.role, Some(Role::Leader { .. }))
    }

    /// Release the frontend lease, drain, and stop.
    ///
    /// A follower has nothing to wind down. A leader keeps the old ordering:
    /// drop its own lease, wait for the others to go idle, then shut down and
    /// surface any listener error.
    pub async fn shutdown(mut self, grace: Duration) -> Result<(), BrokerError> {
        let Some(Role::Leader {
            broker,
            mut listeners,
            lease,
        }) = self.role.take()
        else {
            return Ok(());
        };
        drop(lease);
        broker.wait_until_idle(grace).await;
        broker.shutdown().await;
        let mut result = Ok(());
        while let Some(joined) = listeners.join_next().await {
            match joined {
                Ok(Err(error)) if result.is_ok() => result = Err(error),
                Ok(_) => {}
                Err(error) => tracing::warn!(%error, "broker listener task panicked"),
            }
        }
        result
    }
}

/// Turn an election outcome into a role plus the backend that serves it.
///
/// Returns `None` when the outcome cannot be entered — a follower handshake that
/// fails, or a leader that cannot take its own frontend lease. The caller treats
/// that as a reason to elect again rather than a fatal error.
async fn enter_role(outcome: ElectionOutcome) -> Option<(Role, Backend)> {
    match outcome {
        ElectionOutcome::Leader(leader) => {
            let broker = leader.broker;
            let Some(lease) = broker.frontend_lease() else {
                tracing::warn!("leader frontend lease was unavailable");
                return None;
            };
            let mut listeners = JoinSet::new();
            spawn_listener(&mut listeners, &broker, leader.plugin_listener);
            if let Some(listener) = leader.plugin_listener_v6 {
                spawn_listener(&mut listeners, &broker, listener);
            }
            let frontend_broker = broker.clone();
            listeners.spawn(frontend_broker.serve_frontends(leader.frontend_listener));
            tracing::info!(role = "leader", "entered broker role");
            let backend = Backend::local(broker.clone());
            Some((
                Role::Leader {
                    broker,
                    listeners,
                    lease: Some(lease),
                },
                backend,
            ))
        }
        ElectionOutcome::Follower(follower) => match FrontendClient::from_stream(follower.stream)
            .await
        {
            Ok(client) => {
                tracing::info!(role = "follower", "entered broker role");
                let backend = Backend::remote(client.clone());
                Some((Role::Follower { client }, backend))
            }
            Err(error) => {
                tracing::warn!(%error, "frontend handshake failed");
                None
            }
        },
    }
}

fn spawn_listener(
    listeners: &mut JoinSet<Result<(), BrokerError>>,
    broker: &Broker,
    listener: TcpListener,
) {
    let broker = broker.clone();
    listeners.spawn(broker.serve(listener));
}
