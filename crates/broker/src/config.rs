use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use figma_dev_mcp_protocol::limits::{
    HEARTBEAT_SECS, MAX_ENVELOPE_BYTES, MAX_QUEUE, STALE_SESSION_SECS,
};

pub const PLUGIN_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3056);
pub const FRONTEND_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 3057);
// Bumped to "2" because the wire changed four times without it moving:
// visitedNodes on three per-node results, the SVG rejection reason, that
// reason moving onto the screenshot asset while leaving the shared ToolError,
// and the EMPTY_NODE_BOUNDS error code. A plugin announcing "1" cannot talk to
// today's decoders, and being refused at connect time is strictly better than
// the silent session drop it replaces.
pub const PLUGIN_PROTOCOL_VERSION: &str = "2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    pub max_frame_bytes: usize,
    pub max_message_bytes: usize,
    pub outbound_queue: usize,
    pub heartbeat_interval: Duration,
    pub stale_after: Duration,
}

impl Limits {
    pub fn production() -> Self {
        Self {
            max_frame_bytes: MAX_ENVELOPE_BYTES,
            max_message_bytes: MAX_ENVELOPE_BYTES,
            outbound_queue: MAX_QUEUE,
            heartbeat_interval: Duration::from_secs(HEARTBEAT_SECS),
            stale_after: Duration::from_secs(STALE_SESSION_SECS),
        }
    }

    pub fn reduced_for_test() -> Self {
        Self {
            max_frame_bytes: 64 * 1024,
            max_message_bytes: 64 * 1024,
            outbound_queue: 4,
            heartbeat_interval: Duration::from_millis(25),
            stale_after: Duration::from_millis(100),
        }
    }

    pub fn checked(
        max_frame_bytes: usize,
        max_message_bytes: usize,
        outbound_queue: usize,
        heartbeat_interval: Duration,
        stale_after: Duration,
    ) -> Result<Self, ConfigError> {
        let production = Self::production();
        if max_frame_bytes == 0
            || max_frame_bytes > production.max_frame_bytes
            || max_message_bytes == 0
            || max_message_bytes > production.max_message_bytes
            || outbound_queue == 0
            || outbound_queue > production.outbound_queue
            || heartbeat_interval.is_zero()
            || heartbeat_interval > production.heartbeat_interval
            || stale_after.is_zero()
            || stale_after > production.stale_after
        {
            return Err(ConfigError::AboveProductionCeiling);
        }
        Ok(Self {
            max_frame_bytes,
            max_message_bytes,
            outbound_queue,
            heartbeat_interval,
            stale_after,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerConfig {
    pub plugin_address: SocketAddr,
    pub frontend_address: SocketAddr,
    pub limits: Limits,
}

impl BrokerConfig {
    pub fn production() -> Self {
        Self {
            plugin_address: PLUGIN_ADDRESS,
            frontend_address: FRONTEND_ADDRESS,
            limits: Limits::production(),
        }
    }

    pub fn for_test(limits: Limits) -> Result<Self, ConfigError> {
        Limits::checked(
            limits.max_frame_bytes,
            limits.max_message_bytes,
            limits.outbound_queue,
            limits.heartbeat_interval,
            limits.stale_after,
        )?;
        Ok(Self {
            plugin_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            frontend_address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            limits,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("test limits must be positive and may not exceed production ceilings")]
    AboveProductionCeiling,
}
