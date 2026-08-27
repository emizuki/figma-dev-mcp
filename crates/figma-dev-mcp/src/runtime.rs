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
    let running = service.serve(stdio()).await?;

    let service_result = tokio::select! {
        result = running.waiting() => result.context("stdio service failed").map(|_| ()),
        // Never completes; it is here to keep re-electing while stdio is served.
        () = supervisor.supervise() => unreachable!("supervise never returns"),
    };

    supervisor
        .shutdown(Duration::from_secs(IDLE_GRACE_SECS))
        .await
        .context("broker shutdown failed")?;
    service_result
}
