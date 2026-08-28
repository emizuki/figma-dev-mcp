use std::time::Duration;

use anyhow::Context;
use figma_dev_mcp_broker::{BrokerConfig, Supervisor};
use figma_dev_mcp_protocol::limits::IDLE_GRACE_SECS;
use figma_dev_mcp_tools::McpService;
use rmcp::{ServiceExt, transport::stdio};

pub(crate) async fn run() -> anyhow::Result<()> {
    let mut supervisor = Supervisor::start(BrokerConfig::production())
        .await
        .context("broker leader election failed")?;

    // One service for the life of the process. Its client follows the
    // supervisor through every role change, so a leader dying no longer ends
    // this session — it just costs the calls that were in flight.
    let service = McpService::new(supervisor.client());

    // The handshake itself can fail (e.g. the client hangs up before
    // `initialize`). That must not skip `shutdown` below: the frontend
    // listener is already accepting followers and the plugin port is already
    // bound the moment `Supervisor::start` returns, so any early return here
    // has to still drain the leader's listeners and cancel the shutdown
    // token. Capture the outcome instead of using `?` and run `shutdown`
    // unconditionally before propagating it.
    let service_result = match service.serve(stdio()).await {
        Ok(running) => tokio::select! {
            result = running.waiting() => result.context("stdio service failed").map(|_| ()),
            // Never completes under the current implementation of
            // `supervise`; if that ever changes, fall through to the
            // shutdown below rather than panicking.
            () = supervisor.supervise() => Ok(()),
        },
        Err(error) => Err(anyhow::Error::new(error).context("stdio service failed to start")),
    };

    supervisor
        .shutdown(Duration::from_secs(IDLE_GRACE_SECS))
        .await
        .context("broker shutdown failed")?;
    service_result
}
