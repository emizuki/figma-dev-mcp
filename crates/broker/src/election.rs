use std::{
    io::ErrorKind,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use tokio::{
    net::{TcpListener, TcpStream},
    time::{Instant, sleep},
};

use crate::{Broker, BrokerConfig};

const ELECTION_TIMEOUT: Duration = Duration::from_secs(2);
const MIN_RETRY_DELAY_MS: u64 = 5;
const MAX_RETRY_DELAY_MS: u64 = 100;

fn retry_delay(attempt: u32) -> Duration {
    let base = MIN_RETRY_DELAY_MS
        .saturating_mul(1_u64 << attempt.min(4))
        .min(MAX_RETRY_DELAY_MS);
    let seed =
        u64::from(std::process::id()) ^ u64::from(attempt).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    let jitter = seed % (base / 2 + 1);
    Duration::from_millis((base + jitter).min(MAX_RETRY_DELAY_MS))
}

#[derive(Debug)]
pub struct LeaderElection {
    pub broker: Broker,
    pub plugin_listener: TcpListener,
    pub plugin_listener_v6: Option<TcpListener>,
    pub frontend_listener: TcpListener,
}

#[derive(Debug)]
pub struct FollowerElection {
    pub stream: TcpStream,
}

#[derive(Debug)]
pub enum ElectionOutcome {
    Leader(LeaderElection),
    Follower(FollowerElection),
}

pub async fn elect(config: BrokerConfig) -> Result<ElectionOutcome, ElectionError> {
    match TcpStream::connect(config.frontend_address).await {
        Ok(stream) => return Ok(ElectionOutcome::Follower(FollowerElection { stream })),
        Err(error) if error.kind() == ErrorKind::ConnectionRefused => {}
        Err(error) => return Err(ElectionError::Connect(error)),
    }

    let deadline = Instant::now() + ELECTION_TIMEOUT;
    let mut attempt = 0_u32;
    loop {
        match TcpListener::bind(config.frontend_address).await {
            Ok(frontend_listener) => {
                let plugin_listener = TcpListener::bind(config.plugin_address)
                    .await
                    .map_err(ElectionError::PluginBind)?;
                let plugin_listener_v6 = match plugin_listener.local_addr() {
                    Ok(SocketAddr::V4(v4)) if v4.ip().is_loopback() => TcpListener::bind(
                        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), v4.port()),
                    )
                    .await
                    .ok(),
                    _ => None,
                };
                return Ok(ElectionOutcome::Leader(LeaderElection {
                    broker: Broker::new(config),
                    plugin_listener,
                    plugin_listener_v6,
                    frontend_listener,
                }));
            }
            Err(error) if error.kind() == ErrorKind::AddrInUse => {
                match TcpStream::connect(config.frontend_address).await {
                    Ok(stream) => {
                        return Ok(ElectionOutcome::Follower(FollowerElection { stream }));
                    }
                    Err(connect_error)
                        if connect_error.kind() == ErrorKind::ConnectionRefused
                            && Instant::now() < deadline =>
                    {
                        let delay = retry_delay(attempt);
                        attempt = attempt.wrapping_add(1);
                        sleep(delay).await;
                    }
                    Err(connect_error) => return Err(ElectionError::Connect(connect_error)),
                }
            }
            Err(error) => return Err(ElectionError::FrontendBind(error)),
        }
        if Instant::now() >= deadline {
            return Err(ElectionError::TimedOut);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ElectionError {
    #[error("failed to connect to the frontend leader: {0}")]
    Connect(std::io::Error),
    #[error("failed to bind the frontend RPC listener: {0}")]
    FrontendBind(std::io::Error),
    #[error("failed to bind the plugin WebSocket listener: {0}")]
    PluginBind(std::io::Error),
    #[error("the elected role could not be entered")]
    RoleUnavailable,
    #[error("leader election did not settle within two seconds")]
    TimedOut,
}
