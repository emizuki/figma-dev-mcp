use std::time::Duration;

use anyhow::Context;
use figma_dev_mcp_broker::{BrokerConfig, Supervisor};
use figma_dev_mcp_protocol::limits::IDLE_GRACE_SECS;
use figma_dev_mcp_tools::McpService;
use rmcp::{ServiceExt, transport::stdio};

pub(crate) async fn run() -> anyhow::Result<()> {
    // Unattached: the first election happens inside `supervise`, below. Election
    // must not gate startup — a leader that has just gone idle refuses the
    // frontend handshake while it closes, and an election that cannot succeed
    // would otherwise leave the process reading stdin and answering nothing.
    let mut supervisor = Supervisor::new(BrokerConfig::production());

    // One service for the life of the process. Its client follows the
    // supervisor through every role change, so a leader dying no longer ends
    // this session — it just costs the calls that were in flight.
    let service = McpService::new(supervisor.client());

    // `serve` blocks until the client's `initialize`, so it must be awaited
    // INSIDE the select rather than before it. Awaited before, `supervise`
    // would not be polled until the client initialized, and a follower whose
    // client had not yet done so would never notice its leader dying. Gathering
    // the whole MCP lifecycle into one block also keeps `supervise`'s mutable
    // borrow of `supervisor` the only one, which is what makes this compile.
    let mcp = async {
        let running = service
            .serve(stdio())
            .await
            .map_err(|error| anyhow::Error::new(error).context("stdio service failed to start"))?;
        running.waiting().await.context("stdio service failed")?;
        Ok::<(), anyhow::Error>(())
    };

    let service_result = tokio::select! {
        result = mcp => result,
        // Never completes under the current implementation of `supervise`; if
        // that ever changes, fall through to the shutdown below rather than
        // panicking.
        () = supervisor.supervise() => Ok(()),
    };

    // Runs on every exit path, including a `serve` that failed to start: the
    // frontend listener may already be accepting followers and the plugin port
    // may already be bound, so an early return here would leave them undrained.
    if let Err(error) = supervisor
        .shutdown(Duration::from_secs(IDLE_GRACE_SECS))
        .await
    {
        // A failing shutdown must not bury why the session actually ended. Both
        // can fail at once — a broken stdout pipe ends the service while the
        // leader's `rpc::serve` propagates an accept error — and returning the
        // shutdown error there would report a consequence as the cause, leaving
        // the real one in neither the exit code nor the log.
        if service_result.is_ok() {
            return Err(anyhow::Error::new(error).context("broker shutdown failed"));
        }
        tracing::error!(%error, "broker shutdown failed");
    }
    service_result
}
