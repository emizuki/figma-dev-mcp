//! Role supervision.
//!
//! Leader election used to run once per process, which meant a follower whose
//! leader died stayed a follower forever and nothing ever reopened the plugin's
//! port. The supervisor holds the current role, watches its backend, and
//! re-enters election when that backend dies.

use std::time::Duration;

use tokio::{net::TcpListener, task::JoinSet, time::Instant};

use crate::{
    Broker, BrokerClient, BrokerConfig, BrokerError, ElectionOutcome, FrontendClient,
    FrontendLease, client::Backend, elect,
};

/// First delay after a failed election. Doubles up to `RETRY_DELAY_CAP`.
const RETRY_DELAY_BASE: Duration = Duration::from_millis(100);
/// Ceiling for the election retry delay, matching where the plugin's own
/// reconnect backoff saturates. Recovering slower than the plugin retries would
/// leave it looping for no reason.
const RETRY_DELAY_CAP: Duration = Duration::from_secs(5);
/// Minimum interval between the start of one role and the start of the next.
///
/// A *successful* election that immediately dies again contains no sleep and no
/// network round trip — `elect()` is a refused loopback connect plus two or
/// three binds — so a listener that panics on spawn, or an `accept()` that keeps
/// failing instantly under fd exhaustion, would spin thousands of times a
/// second, logging on every pass and unbinding and rebinding the plugin port
/// each time, which would churn the plugin's own connection. The supervisor
/// exists to survive bugs in the thing it supervises, so it floors the cycle
/// rate rather than assuming the listeners never misbehave.
const MIN_ELECTION_INTERVAL: Duration = RETRY_DELAY_BASE;

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
    /// When the current role was entered, so `elect_next` can refuse to cycle
    /// faster than `MIN_ELECTION_INTERVAL`. `None` before the first election,
    /// where there is no previous role to rate-limit against.
    elected_at: Option<Instant>,
}

impl Supervisor {
    /// Elect eagerly and enter the role it produces before returning.
    ///
    /// This is **not** what the binary uses. Production builds a `Supervisor`
    /// with `new`, which starts unattached, and runs `supervise()` to perform
    /// the first election as just another iteration of the same loop that
    /// handles every later one. That is deliberate: this branch's whole reason
    /// for existing is that an eager, blocking first election — this one — used
    /// to sit in front of the MCP service, so a first election that never
    /// succeeded hung the process before it could even answer `initialize`.
    /// `new` plus `supervise()` removed that window by not having a separate
    /// startup path at all.
    ///
    /// `start` still exists because some tests need a `Supervisor` that has
    /// already elected before they can proceed, and blocking here is simpler
    /// than driving `supervise()` to its first iteration by hand. It goes
    /// through the same retrying `elect_until_entered` as `supervise()` does,
    /// so it is not a second election *policy* — but it is a second entry
    /// point, and reaching for it from `runtime.rs` would silently reintroduce
    /// the hang this branch fixed. Do not use it there.
    #[doc(hidden)]
    pub async fn start(config: BrokerConfig) -> Self {
        tracing::info!("running the first broker election");
        let (role, backend) = elect_until_entered(&config).await;
        Self {
            config,
            client: BrokerClient::new(backend),
            role: Some(role),
            elected_at: Some(Instant::now()),
        }
    }

    /// A supervisor that has not elected anything yet.
    ///
    /// Its `client()` is unattached, so the MCP service can be built and served
    /// immediately; `supervise()` runs the first election as its first
    /// iteration. The binary uses this rather than `start` so that nothing —
    /// not even a first election that never succeeds — can stop the process
    /// from answering its client.
    pub fn new(config: BrokerConfig) -> Self {
        Self {
            config,
            client: BrokerClient::unattached(),
            role: None,
            elected_at: None,
        }
    }

    /// A handle to the current backend. Clone it for the MCP service; it stays
    /// valid across every role change.
    pub fn client(&self) -> BrokerClient {
        self.client.clone()
    }

    pub fn is_leader(&self) -> bool {
        matches!(self.role, Some(Role::Leader { .. }))
    }

    /// Enter a role and install the backend that serves it.
    ///
    /// One of the two mutators of `self.role` and `self.client` after
    /// construction. They exist as a pair because the two fields carry one
    /// coupled meaning — a role and the backend that serves it — and nothing
    /// previously required a detach to accompany an install, which is exactly
    /// how a dead broker came to stay installed across a re-election.
    fn install_role(&mut self, role: Role, backend: Backend) {
        let role_name = if matches!(role, Role::Leader { .. }) {
            "leader"
        } else {
            "follower"
        };
        self.client.install(backend);
        tracing::info!(role = role_name, "installed a new broker backend");
        self.role = Some(role);
        self.elected_at = Some(Instant::now());
    }

    /// Leave the current role and detach the client, so calls arriving before
    /// the next election fail retryably rather than reaching a dead backend.
    ///
    /// Returns the role that was current, so the caller can wind it down.
    fn clear_role(&mut self) -> Option<Role> {
        self.client.detach();
        self.role.take()
    }

    /// Elect a role if there is none, then watch the current backend and
    /// re-elect whenever it dies.
    ///
    /// Never returns. Run it alongside the service that uses `client()`; when
    /// that service ends, drop this future and call `shutdown`.
    ///
    /// Re-election is eager rather than lazy — it fires on the death signal, not
    /// on the next call — because the plugin needs someone listening on the
    /// plugin port even while no tool call is happening. That is the whole point.
    pub async fn supervise(&mut self) {
        loop {
            let Some(role) = self.role.as_mut() else {
                // No role: elect one. On the first iteration after `new` this
                // IS the first election — that is the whole point. There is no
                // separate startup election path to drift out of step with
                // this one.
                let (role, backend) = self.elect_next().await;
                self.install_role(role, backend);
                continue;
            };
            let death = role.death().await;
            match &death {
                // A panic is a genuine internal bug, not the routine death a
                // dropped connection or a rejected accept represents. Log it
                // loudly even though re-election swallows it the same way.
                Death::ListenerPanicked(_) => {
                    tracing::error!(cause = %death, "broker backend died, re-electing")
                }
                Death::LeaderGone | Death::ListenerStopped(_) => {
                    tracing::warn!(cause = %death, "broker backend died, re-electing")
                }
            }

            // Drop the old role's sockets before re-electing, and wait for it.
            // `JoinSet`'s `Drop` only calls `abort()` on each task, which merely
            // schedules cancellation; the listener future (and the TcpListener
            // it owns) isn't actually dropped until the scheduler next polls it.
            // `elect()`'s first move is a loopback connect to the frontend
            // address, which can land in that still-open listener's backlog
            // before it closes — the ex-leader would then connect to itself.
            // `JoinSet::shutdown` aborts and drains, returning only once every
            // listener future has actually been dropped, so the ports are
            // guaranteed free before `elect_next` runs. A follower has no
            // listeners to wait on; `self.role.take()` still drops its
            // `FrontendClient` either way.
            if let Some(Role::Leader {
                broker,
                mut listeners,
                lease,
            }) = self.clear_role()
            {
                // Fail the in-flight calls before letting go of the role. Only
                // one death path does this for us: `ws::serve` cancels the
                // shared token on an accept error, and a follower's
                // `client_loop` drains its pending map with `ConnectionLost`.
                // `rpc::serve` propagates its accept error with a bare `?` and
                // a panicked listener cancels nothing at all, and on those an
                // in-flight `BrokerClient::call` holds its own clone of this
                // very `Broker` — so the state cannot drop, and the call waits
                // on a oneshot whose sender only that state holds until its
                // tool deadline expires. Cancelling the token here drains the
                // pending map, which makes every death path uniform.
                broker.shutdown().await;
                drop(lease);
                drop(broker);
                listeners.shutdown().await;
            }
        }
    }

    /// Re-elect after a death, never cycling faster than
    /// `MIN_ELECTION_INTERVAL`.
    ///
    /// The floor is measured from when the *previous* role was entered, so a
    /// role that lived a while re-elects immediately and only a role that died
    /// on arrival waits. See `MIN_ELECTION_INTERVAL` for why the floor exists.
    /// This is also the very first election when `elected_at` is `None` — there
    /// is no previous role to rate-limit against, so the floor is skipped and
    /// election runs immediately. The caller's `install_role` is what stamps
    /// `elected_at` for the role this call produces — this function only reads
    /// the stamp left by the *previous* one.
    async fn elect_next(&mut self) -> (Role, Backend) {
        if let Some(elected_at) = self.elected_at {
            let alive_for = elected_at.elapsed();
            if let Some(floor) = MIN_ELECTION_INTERVAL.checked_sub(alive_for) {
                tracing::warn!(
                    alive_ms = alive_for.as_millis() as u64,
                    floor_ms = floor.as_millis() as u64,
                    "the previous role died immediately, delaying re-election"
                );
                tokio::time::sleep(floor).await;
            }
        }
        elect_until_entered(&self.config).await
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
                Err(error) => tracing::error!(%error, "broker listener task panicked"),
            }
        }
        result
    }
}

/// Elect until it works, backing off between attempts.
///
/// Retries forever on purpose. Whatever is wrong — a port held by something
/// else, a leader running an incompatible build, a leader that is mid-shutdown
/// and refusing handshakes — is usually transient, and giving up would brick
/// this session permanently, which is the bug being fixed. Meanwhile calls fail
/// with a retryable error, never worse than before.
async fn elect_until_entered(config: &BrokerConfig) -> (Role, Backend) {
    let mut delay = RETRY_DELAY_BASE;
    let mut attempt = 1_u32;
    loop {
        tracing::info!(attempt, "starting broker election");
        match elect(config.clone()).await {
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
            // Read the bound addresses before the listeners move into their
            // tasks: the goal is that the next incident is diagnosable from the
            // log directory alone, without reaching for `lsof`.
            let plugin_address = bound_address(&leader.plugin_listener);
            let plugin_address_v6 = leader.plugin_listener_v6.as_ref().map(bound_address);
            let frontend_address = bound_address(&leader.frontend_listener);
            let mut listeners = JoinSet::new();
            spawn_listener(&mut listeners, &broker, leader.plugin_listener);
            if let Some(listener) = leader.plugin_listener_v6 {
                spawn_listener(&mut listeners, &broker, listener);
            }
            let frontend_broker = broker.clone();
            listeners.spawn(frontend_broker.serve_frontends(leader.frontend_listener));
            tracing::info!(
                role = "leader",
                plugin_address,
                plugin_address_v6 = plugin_address_v6.as_deref().unwrap_or("none"),
                frontend_address,
                "entered broker role"
            );
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
            let leader_address = follower.stream.peer_addr().map_or_else(
                |error| format!("unknown ({error})"),
                |address| address.to_string(),
            );
            match FrontendClient::from_stream(follower.stream).await {
                Ok(client) => {
                    tracing::info!(role = "follower", leader_address, "entered broker role");
                    let backend = Backend::remote(client.clone());
                    Some((Role::Follower { client }, backend))
                }
                Err(error) => {
                    tracing::warn!(%error, leader_address, "frontend handshake failed");
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

/// A listener's bound address, rendered for the log.
fn bound_address(listener: &TcpListener) -> String {
    listener.local_addr().map_or_else(
        |error| format!("unknown ({error})"),
        |address| address.to_string(),
    )
}
