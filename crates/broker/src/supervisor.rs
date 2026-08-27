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

/// First delay after a failed election. Doubles up to `RETRY_DELAY_CAP`.
const RETRY_DELAY_BASE: Duration = Duration::from_millis(100);
/// Ceiling for the election retry delay, matching where the plugin's own
/// reconnect backoff saturates. Recovering slower than the plugin retries would
/// leave it looping for no reason.
const RETRY_DELAY_CAP: Duration = Duration::from_secs(5);

/// The role this process currently holds.
enum Role {
    Leader {
        broker: Broker,
        listeners: JoinSet<Result<(), BrokerError>>,
        lease: Option<FrontendLease>,
    },
    Follower {
        client: FrontendClient,
    },
}

impl Role {
    /// Resolves when this role's backend dies.
    ///
    /// For a leader that is any one of the listener tasks ending. They do not
    /// all share a single point of failure: `ws::serve` cancels the shared
    /// shutdown token on an accept error, which cascades to the others, but
    /// `rpc::serve` propagates its accept error and cancels nothing, so a
    /// frontend accept failure can end only that one task while the plugin
    /// listeners keep running. Either way, `join_next()` firing means some
    /// listener stopped, which is sufficient to treat this role as dead. For a
    /// follower it is the RPC connection to the leader closing.
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
            Death::ListenerPanicked(error) => {
                write!(formatter, "a listener task panicked: {error}")
            }
        }
    }
}

/// Owns the process's role and keeps it alive across leader deaths.
pub struct Supervisor {
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

    /// Watch the current backend and re-elect whenever it dies.
    ///
    /// Never returns. Run it alongside the service that uses `client()`; when
    /// that service ends, drop this future and call `shutdown`.
    ///
    /// Re-election is eager rather than lazy — it fires on the death signal, not
    /// on the next call — because the plugin needs someone listening on the
    /// plugin port even while no tool call is happening. That is the whole point.
    pub async fn supervise(&mut self) {
        loop {
            let death = match self.role.as_mut() {
                Some(role) => role.death().await,
                None => return,
            };
            tracing::warn!(cause = %death, "broker backend died, re-electing");

            // Drop the old role's sockets before re-electing, and wait for it.
            // `JoinSet`'s `Drop` only calls `abort()` on each task, which merely
            // schedules cancellation; the listener future (and the TcpListener
            // it owns) isn't actually dropped until the scheduler next polls it.
            // `elect()`'s first move is a loopback connect to the frontend
            // address, which can land in that still-open listener's backlog
            // before it closes — the ex-leader would then connect to itself.
            // `JoinSet::shutdown` aborts and drains, returning only once every
            // listener future has actually been dropped, so the ports are
            // guaranteed free before `elect_again` runs. A follower has no
            // listeners to wait on; `self.role.take()` still drops its
            // `FrontendClient` either way.
            if let Some(Role::Leader {
                broker,
                mut listeners,
                lease,
            }) = self.role.take()
            {
                drop(lease);
                drop(broker);
                listeners.shutdown().await;
            }

            let (role, backend) = self.elect_again().await;
            self.client.install(backend);
            tracing::info!("installed a new broker backend");
            self.role = Some(role);
        }
    }

    /// Elect until it works, backing off between attempts.
    ///
    /// Retries forever on purpose. Whatever is wrong — a port held by something
    /// else, a leader running an incompatible build — is usually transient, and
    /// giving up would brick this session permanently, which is the bug being
    /// fixed. Meanwhile calls fail with a retryable error, never worse than
    /// before.
    async fn elect_again(&self) -> (Role, Backend) {
        let mut delay = RETRY_DELAY_BASE;
        let mut attempt = 1_u32;
        loop {
            tracing::info!(attempt, "starting broker election");
            match elect(self.config.clone()).await {
                Ok(outcome) => {
                    if let Some(entered) = enter_role(outcome).await {
                        return entered;
                    }
                }
                Err(error) => tracing::warn!(%error, attempt, "broker election failed"),
            }
            tracing::warn!(
                attempt,
                delay_ms = delay.as_millis() as u64,
                "retrying election"
            );
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(RETRY_DELAY_CAP);
            attempt = attempt.wrapping_add(1);
        }
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
        ElectionOutcome::Follower(follower) => {
            match FrontendClient::from_stream(follower.stream).await {
                Ok(client) => {
                    tracing::info!(role = "follower", "entered broker role");
                    let backend = Backend::remote(client.clone());
                    Some((Role::Follower { client }, backend))
                }
                Err(error) => {
                    tracing::warn!(%error, "frontend handshake failed");
                    None
                }
            }
        }
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
