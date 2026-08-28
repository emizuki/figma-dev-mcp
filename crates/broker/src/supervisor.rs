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
/// Attempts reported in full before the election loop goes quiet. Six cover the
/// backoff growth from `RETRY_DELAY_BASE` to `RETRY_DELAY_CAP`; the extra two
/// put the ceiling itself in the log before it falls silent.
const FULL_LOG_ATTEMPTS: u32 = 8;
/// Once quiet, one `warn!` every this many attempts — about five minutes at
/// `RETRY_DELAY_CAP` — so a permanently stuck process stays visible without
/// flooding the host's captured log.
const QUIET_HEARTBEAT_ATTEMPTS: u32 = 60;
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
/// A role that lived at least this long counts as a genuine session rather than
/// a failure to start, and resets the recycle escalation.
///
/// Matched to `RETRY_DELAY_CAP` so that a role which outlives the longest delay
/// the supervisor will ever impose is treated as healthy. A role that keeps
/// dying faster than this is not recovering, it is thrashing.
const HEALTHY_ROLE_LIFETIME: Duration = RETRY_DELAY_CAP;
/// Recycles reported in full before the loop goes quiet. Three is enough to show
/// the death cause, the election, and the role that replaced it — the whole
/// diagnosis. Past that the same lines repeat verbatim.
const FULL_LOG_RECYCLES: u32 = 3;
/// Once quiet, one `warn!` every this many recycles — about one a minute once the
/// delay has escalated to `RETRY_DELAY_CAP` — so a permanently spinning process
/// stays visible without flooding the host's captured log.
const QUIET_HEARTBEAT_RECYCLES: u32 = 12;

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
    /// Consecutive role deaths that arrived sooner than `HEALTHY_ROLE_LIFETIME`.
    ///
    /// Zero whenever the last role lived a normal life. `MIN_ELECTION_INTERVAL`
    /// alone floors the cycle at ten per second, which a role that dies on
    /// arrival will sustain forever — rebinding the plugin port and dropping the
    /// plugin's WebSocket ten times a second. This counter is what turns that
    /// into a curve that settles.
    consecutive_immediate_deaths: u32,
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
        let (role, backend) = elect_until_entered(&config, ElectionLog::Full).await;
        Self {
            config,
            client: BrokerClient::new(backend),
            role: Some(role),
            elected_at: Some(Instant::now()),
            consecutive_immediate_deaths: 0,
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
            consecutive_immediate_deaths: 0,
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
    /// construction, alongside `clear_role` — except for `shutdown`, which
    /// unbundles the pair on purpose: it takes `self.role` up front because it
    /// must destructure the `Leader` to wind it down, and detaches the client
    /// only after the idle grace has elapsed, so calls arriving during that
    /// grace are still served by a live broker. The exception is deliberate and
    /// bounded, not an oversight — but it is a real exception, and this pair is
    /// not exhaustive.
    ///
    /// They exist as a pair because the two fields carry one coupled meaning — a
    /// role and the backend that serves it — and nothing previously required a
    /// detach to accompany an install, which is exactly how a dead broker came
    /// to stay installed across a re-election.
    fn install_role(&mut self, role: Role, backend: Backend) {
        let role_name = if matches!(role, Role::Leader { .. }) {
            "leader"
        } else {
            "follower"
        };
        self.client.install(backend);
        match recycle_log(self.consecutive_immediate_deaths) {
            ElectionLog::Full | ElectionLog::Heartbeat => {
                tracing::info!(role = role_name, "installed a new broker backend")
            }
            ElectionLog::Quiet => {
                tracing::debug!(role = role_name, "installed a new broker backend")
            }
        }
        self.role = Some(role);
        self.elected_at = Some(Instant::now());
    }

    /// Leave the current role and detach the client, so calls arriving before
    /// the next election fail retryably rather than reaching a dead backend.
    ///
    /// Returns the role that was current, so the caller can wind it down.
    fn clear_role(&mut self) -> Option<Role> {
        self.client.detach();
        match recycle_log(self.consecutive_immediate_deaths) {
            ElectionLog::Full | ElectionLog::Heartbeat => {
                tracing::info!("detached the broker backend")
            }
            ElectionLog::Quiet => tracing::debug!("detached the broker backend"),
        }
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
            // Read the count after the death, before `elect_next` updates it, so
            // this line reports how many immediate deaths preceded this one. The
            // bands therefore lag by one cycle. That is deliberate: the count is
            // genuinely "how many had happened before this", and shifting the
            // band by one cycle costs nothing. Do not "fix" it by incrementing
            // early — the update belongs with the `alive_for` measurement that
            // decides whether to reset it.
            let verbosity = recycle_log(self.consecutive_immediate_deaths);
            match (&death, verbosity) {
                // A panic is a genuine internal bug, not the routine death a
                // dropped connection or a rejected accept represents. Loud while
                // the cycle is still being reported at all.
                (Death::ListenerPanicked(_), ElectionLog::Full | ElectionLog::Heartbeat) => {
                    tracing::error!(cause = %death, "broker backend died, re-electing")
                }
                (_, ElectionLog::Full | ElectionLog::Heartbeat) => {
                    tracing::warn!(cause = %death, "broker backend died, re-electing")
                }
                // A spin repeats the same cause verbatim; the recycle heartbeat
                // is what keeps the condition visible.
                (_, ElectionLog::Quiet) => {
                    tracing::debug!(cause = %death, "broker backend died, re-electing")
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
            // listeners to wait on, but `clear_role()` runs unconditionally for
            // either role: it always drops the outgoing role's `FrontendClient`
            // and detaches `self.client`, so calls arriving before the next
            // election fail retryably instead of reaching a dead backend.
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

    /// Re-elect after a death, never cycling faster than `recycle_delay` allows.
    ///
    /// The delay is measured from when the *previous* role was entered, so a role
    /// that lived a while re-elects immediately and only a role that died on
    /// arrival waits. Consecutive immediate deaths escalate that wait from
    /// `MIN_ELECTION_INTERVAL` toward `RETRY_DELAY_CAP`; see `recycle_delay`.
    /// This is also the very first election when `elected_at` is `None` — there
    /// is no previous role to rate-limit against, so it runs immediately. The
    /// caller's `install_role` is what stamps `elected_at` for the role this call
    /// produces — this function only reads the stamp left by the previous one.
    async fn elect_next(&mut self) -> (Role, Backend) {
        if let Some(elected_at) = self.elected_at {
            let alive_for = elected_at.elapsed();
            // Update before computing the delay: the count includes this death.
            self.consecutive_immediate_deaths = if alive_for >= HEALTHY_ROLE_LIFETIME {
                0
            } else {
                self.consecutive_immediate_deaths.saturating_add(1)
            };
            let delay = recycle_delay(self.consecutive_immediate_deaths);
            // `checked_sub` keeps the floor measured from role entry, as before:
            // a role that lived 60ms against a 100ms delay sleeps the remaining
            // 40ms. `filter` drops the zero-length sleep a healthy role produces.
            if let Some(floor) = delay
                .checked_sub(alive_for)
                .filter(|floor| !floor.is_zero())
            {
                match recycle_log(self.consecutive_immediate_deaths) {
                    ElectionLog::Full => tracing::warn!(
                        alive_ms = alive_for.as_millis() as u64,
                        floor_ms = floor.as_millis() as u64,
                        consecutive = self.consecutive_immediate_deaths,
                        "the previous role died immediately, delaying re-election"
                    ),
                    // The heartbeat names the condition rather than the cycle.
                    // One line a minute saying the role keeps dying is worth more
                    // to whoever reads this log than a verbatim transcript.
                    ElectionLog::Heartbeat => tracing::warn!(
                        consecutive = self.consecutive_immediate_deaths,
                        delay_ms = delay.as_millis() as u64,
                        "the broker role keeps dying on arrival; recycling slowly"
                    ),
                    ElectionLog::Quiet => tracing::debug!(
                        alive_ms = alive_for.as_millis() as u64,
                        floor_ms = floor.as_millis() as u64,
                        consecutive = self.consecutive_immediate_deaths,
                        "the previous role died immediately, delaying re-election"
                    ),
                }
                tokio::time::sleep(floor).await;
            }
        }
        elect_until_entered(&self.config, recycle_log(self.consecutive_immediate_deaths)).await
    }

    /// Release the frontend lease, drain, and stop.
    ///
    /// A follower has nothing to wind down. A leader keeps the old ordering:
    /// drop its own lease, wait for the others to go idle, then shut down and
    /// surface any listener error — but not the old exit code. A panicked
    /// listener is logged at `error!` here rather than turned into an `Err`;
    /// see the comment on that arm below for why.
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
        // Detach here, not at the top. A call arriving during the grace is still
        // served by a live broker, which is the behaviour this path has always
        // had and which `wait_until_idle` exists to preserve. What must not
        // survive is the window AFTER `broker.shutdown()`, where the cell would
        // otherwise still point at a broker answering from a registry it never
        // cleared — `Ok` with stale files, or a non-retryable
        // `ConnectionNotFound`.
        self.client.detach();
        broker.shutdown().await;
        let mut result = Ok(());
        while let Some(joined) = listeners.join_next().await {
            match joined {
                Ok(Err(error)) if result.is_ok() => result = Err(error),
                Ok(_) => {}
                // Deliberate: a panicked listener does not fail the process.
                // With `supervise` racing in the same select, a mid-session
                // listener panic is consumed by re-election and also exits 0,
                // so restoring a non-zero exit only on the shutdown path would
                // make the exit code depend on which side of a sub-millisecond
                // race the same panic landed on. The signal a supervisor wants
                // is the `error!` line, which is why it is at that level.
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
async fn elect_until_entered(config: &BrokerConfig, ceiling: ElectionLog) -> (Role, Backend) {
    let mut delay = RETRY_DELAY_BASE;
    let mut attempt = 1_u32;
    loop {
        let verbosity = match (election_log(attempt), ceiling) {
            // Not spinning: the election reports at its own discretion, as before.
            (level, ElectionLog::Full) => level,
            // Spinning, and this election is now stuck too. Its own heartbeat
            // must survive the ceiling: a spin whose next election never returns
            // stops recycling, so no recycle heartbeat can fire either, and
            // suppressing this one as well would take the process silent
            // in the worst state it can reach.
            (ElectionLog::Heartbeat, _) => ElectionLog::Heartbeat,
            // Spinning and electing normally: the fresh `attempt = 1` on every
            // cycle would otherwise keep these lines at full volume forever.
            _ => ElectionLog::Quiet,
        };
        if verbosity == ElectionLog::Full {
            tracing::info!(attempt, "starting broker election");
        }
        match elect(config.clone()).await {
            Ok(outcome) => {
                if let Some(entered) = enter_role(outcome, verbosity).await {
                    return entered;
                }
            }
            Err(error) => match verbosity {
                ElectionLog::Full => tracing::warn!(%error, attempt, "broker election failed"),
                _ => tracing::debug!(%error, attempt, "broker election failed"),
            },
        }
        let delay_ms = delay.as_millis() as u64;
        match verbosity {
            ElectionLog::Full => tracing::warn!(attempt, delay_ms, "retrying election"),
            ElectionLog::Heartbeat => tracing::warn!(
                attempt,
                delay_ms,
                "broker election is still failing; retrying at the backoff ceiling"
            ),
            ElectionLog::Quiet => tracing::debug!(attempt, delay_ms, "retrying election"),
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(RETRY_DELAY_CAP);
        attempt = attempt.wrapping_add(1);
    }
}

/// How loudly one election attempt should be reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ElectionLog {
    /// Every line, as before. The transient failures this loop exists for
    /// resolve inside this band.
    Full,
    /// Nothing above `debug!`, which the default `info` filter drops.
    Quiet,
    /// One `warn!` so a permanently stuck process stays visible.
    Heartbeat,
}

/// Decide how loudly to report one attempt.
///
/// Since `elect_until_entered` retries forever, logging every attempt at
/// `info!`/`warn!` means a permanently unavailable election — a foreign process
/// holding the frontend port, say — writes roughly three lines every
/// `RETRY_DELAY_CAP` into the MCP host's captured log, indefinitely. Before this
/// loop retried forever the process exited once with a single error, so the
/// volume was bounded by construction; now it has to be bounded deliberately.
///
/// Full logging covers the whole backoff growth phase — 100ms doubling to the
/// 5s cap takes six attempts — plus two at the ceiling, so the log always shows
/// the delay reaching its cap before it falls silent. That band is where a
/// transient failure lives, and it is exactly the diagnosability the logging was
/// added for. Past it the process is stuck rather than starting up, and a
/// heartbeat is worth more than a transcript.
fn election_log(attempt: u32) -> ElectionLog {
    if attempt <= FULL_LOG_ATTEMPTS {
        ElectionLog::Full
    } else if attempt.is_multiple_of(QUIET_HEARTBEAT_ATTEMPTS) {
        ElectionLog::Heartbeat
    } else {
        ElectionLog::Quiet
    }
}

/// How long to wait before re-electing, given how many roles in a row have died
/// on arrival.
///
/// Zero for the first death after a healthy role — a leader that ran for hours
/// and then lost a listener should reclaim its ports at once — then the same
/// doubling curve `elect_until_entered` uses for its own retries, capped at
/// `RETRY_DELAY_CAP`. The supervisor's two loops therefore back off identically.
///
/// This is the whole escalation policy, extracted from `elect_next` so it can be
/// tested without driving the loop.
fn recycle_delay(consecutive: u32) -> Duration {
    if consecutive == 0 {
        return Duration::ZERO;
    }
    // Clamp before shifting: anything past a handful of doublings saturates at
    // the cap anyway, and `1u32 << 32` is undefined behaviour territory the
    // compiler rejects outright at runtime.
    let doublings = (consecutive - 1).min(16);
    MIN_ELECTION_INTERVAL
        .saturating_mul(1u32 << doublings)
        .min(RETRY_DELAY_CAP)
}

/// How loudly one recycle should be reported.
///
/// The counterpart to `election_log`, for the outer loop. `election_log` bounds a
/// single election that never succeeds; this bounds a succession of elections
/// that each succeed and then immediately die. They are separate counters
/// because they are separate failures — and because `attempt` resets on every
/// re-election, which is exactly why the existing gate cannot see a spin.
fn recycle_log(consecutive: u32) -> ElectionLog {
    if consecutive <= FULL_LOG_RECYCLES {
        ElectionLog::Full
    } else if consecutive.is_multiple_of(QUIET_HEARTBEAT_RECYCLES) {
        ElectionLog::Heartbeat
    } else {
        ElectionLog::Quiet
    }
}

/// Turn an election outcome into a role plus the backend that serves it.
///
/// Returns `None` when the outcome cannot be entered — a follower handshake that
/// fails, or a leader that cannot take its own frontend lease. The caller treats
/// that as a reason to elect again rather than a fatal error.
async fn enter_role(outcome: ElectionOutcome, verbosity: ElectionLog) -> Option<(Role, Backend)> {
    match outcome {
        ElectionOutcome::Leader(leader) => {
            let broker = leader.broker;
            let Some(lease) = broker.frontend_lease() else {
                // Gated like the rest of the attempt: a role that cannot be
                // entered fails once per retry, so an unbounded loop would
                // otherwise report it forever. See `election_log`.
                match verbosity {
                    ElectionLog::Full => {
                        tracing::warn!("leader frontend lease was unavailable")
                    }
                    _ => tracing::debug!("leader frontend lease was unavailable"),
                }
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
            match verbosity {
                ElectionLog::Full | ElectionLog::Heartbeat => tracing::info!(
                    role = "leader",
                    plugin_address,
                    plugin_address_v6 = plugin_address_v6.as_deref().unwrap_or("none"),
                    frontend_address,
                    "entered broker role"
                ),
                ElectionLog::Quiet => tracing::debug!(
                    role = "leader",
                    plugin_address,
                    plugin_address_v6 = plugin_address_v6.as_deref().unwrap_or("none"),
                    frontend_address,
                    "entered broker role"
                ),
            }
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
                    match verbosity {
                        ElectionLog::Full | ElectionLog::Heartbeat => {
                            tracing::info!(role = "follower", leader_address, "entered broker role")
                        }
                        ElectionLog::Quiet => {
                            tracing::debug!(
                                role = "follower",
                                leader_address,
                                "entered broker role"
                            )
                        }
                    }
                    let backend = Backend::remote(client.clone());
                    Some((Role::Follower { client }, backend))
                }
                Err(error) => {
                    // The loudest line in the permanently-failing case: a
                    // foreign process holding the frontend port accepts the
                    // connect and never handshakes, so this fires once per
                    // retry forever. Gated with the rest of the attempt.
                    match verbosity {
                        ElectionLog::Full => {
                            tracing::warn!(%error, leader_address, "frontend handshake failed")
                        }
                        _ => tracing::debug!(%error, leader_address, "frontend handshake failed"),
                    }
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

#[cfg(test)]
mod tests {
    use super::{
        ElectionLog, FULL_LOG_ATTEMPTS, FULL_LOG_RECYCLES, HEALTHY_ROLE_LIFETIME,
        MIN_ELECTION_INTERVAL, QUIET_HEARTBEAT_ATTEMPTS, QUIET_HEARTBEAT_RECYCLES, RETRY_DELAY_CAP,
        election_log, recycle_delay, recycle_log,
    };
    use std::time::Duration;

    #[test]
    fn the_backoff_growth_phase_is_logged_in_full() {
        for attempt in 1..=FULL_LOG_ATTEMPTS {
            assert_eq!(
                election_log(attempt),
                ElectionLog::Full,
                "attempt {attempt} is still in the growth phase and must be logged in full"
            );
        }
    }

    #[test]
    fn a_permanently_failing_election_goes_quiet() {
        assert_eq!(election_log(FULL_LOG_ATTEMPTS + 1), ElectionLog::Quiet);
        assert_eq!(election_log(FULL_LOG_ATTEMPTS + 2), ElectionLog::Quiet);
    }

    #[test]
    fn a_stuck_election_still_reports_periodically() {
        assert_eq!(
            election_log(QUIET_HEARTBEAT_ATTEMPTS),
            ElectionLog::Heartbeat,
            "a process stuck forever must stay visible in the log"
        );
        assert_eq!(
            election_log(QUIET_HEARTBEAT_ATTEMPTS * 2),
            ElectionLog::Heartbeat
        );
        assert_eq!(
            election_log(QUIET_HEARTBEAT_ATTEMPTS + 1),
            ElectionLog::Quiet,
            "only the heartbeat attempt itself is loud"
        );
    }

    #[test]
    fn a_role_that_lived_a_healthy_life_re_elects_immediately() {
        assert_eq!(
            recycle_delay(0),
            Duration::ZERO,
            "a leader that ran for hours and then lost a listener must not be delayed"
        );
    }

    #[test]
    fn the_first_immediate_death_waits_the_old_floor() {
        assert_eq!(
            recycle_delay(1),
            MIN_ELECTION_INTERVAL,
            "the escalation must start where the old fixed floor was, so a single \
             transient death behaves exactly as it did before"
        );
    }

    #[test]
    fn consecutive_immediate_deaths_double_the_delay() {
        assert_eq!(recycle_delay(2), MIN_ELECTION_INTERVAL * 2);
        assert_eq!(recycle_delay(3), MIN_ELECTION_INTERVAL * 4);
        assert_eq!(recycle_delay(4), MIN_ELECTION_INTERVAL * 8);
    }

    #[test]
    fn the_recycle_delay_saturates_at_the_backoff_cap() {
        // 100ms doubling reaches 5s on the seventh consecutive death.
        assert_eq!(recycle_delay(7), RETRY_DELAY_CAP);
        for consecutive in [8_u32, 64, 1_000, u32::MAX] {
            assert_eq!(
                recycle_delay(consecutive),
                RETRY_DELAY_CAP,
                "recycle_delay({consecutive}) must saturate, not overflow"
            );
        }
    }

    #[test]
    fn the_recycle_delay_never_decreases() {
        let mut previous = Duration::ZERO;
        for consecutive in 0..40_u32 {
            let delay = recycle_delay(consecutive);
            assert!(
                delay >= previous,
                "recycle_delay must be monotonic; {consecutive} gave {delay:?} after {previous:?}"
            );
            previous = delay;
        }
    }

    #[test]
    fn a_healthy_role_outlives_the_longest_delay_the_supervisor_imposes() {
        assert!(
            HEALTHY_ROLE_LIFETIME >= RETRY_DELAY_CAP,
            "a role that survived the maximum backoff must count as healthy, or the \
             escalation could never reset once it reached the cap"
        );
    }

    #[test]
    fn the_first_recycles_are_reported_in_full() {
        for consecutive in 0..=FULL_LOG_RECYCLES {
            assert_eq!(
                recycle_log(consecutive),
                ElectionLog::Full,
                "recycle {consecutive} is still inside the diagnosable band"
            );
        }
    }

    #[test]
    fn a_sustained_spin_goes_quiet() {
        assert_eq!(recycle_log(FULL_LOG_RECYCLES + 1), ElectionLog::Quiet);
        assert_eq!(recycle_log(FULL_LOG_RECYCLES + 2), ElectionLog::Quiet);
    }

    #[test]
    fn a_sustained_spin_still_reports_periodically() {
        assert_eq!(
            recycle_log(QUIET_HEARTBEAT_RECYCLES),
            ElectionLog::Heartbeat,
            "a process whose role keeps dying must stay visible in the log"
        );
        assert_eq!(
            recycle_log(QUIET_HEARTBEAT_RECYCLES * 2),
            ElectionLog::Heartbeat
        );
        assert_eq!(
            recycle_log(QUIET_HEARTBEAT_RECYCLES + 1),
            ElectionLog::Quiet,
            "only the heartbeat recycle itself is loud"
        );
    }
}
