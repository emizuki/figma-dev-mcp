use std::time::Duration;

use anyhow::Context;
use figma_dev_mcp_broker::{BrokerClient, BrokerConfig, ElectionOutcome, FrontendClient, elect};
use figma_dev_mcp_protocol::limits::IDLE_GRACE_SECS;
use figma_dev_mcp_tools::McpService;
use rmcp::{ServiceExt, transport::stdio};

pub(crate) async fn run() -> anyhow::Result<()> {
    match elect(BrokerConfig::production()).await? {
        ElectionOutcome::Follower(follower) => run_follower(follower.stream).await,
        ElectionOutcome::Leader(leader) => run_leader(leader).await,
    }
}

async fn run_follower(stream: tokio::net::TcpStream) -> anyhow::Result<()> {
    let frontend = FrontendClient::from_stream(stream)
        .await
        .context("frontend handshake failed")?;
    let service = McpService::new(BrokerClient::remote(frontend));
    let running = service.serve(stdio()).await?;
    running
        .waiting()
        .await
        .context("stdio frontend service failed")?;
    Ok(())
}

async fn run_leader(leader: figma_dev_mcp_broker::LeaderElection) -> anyhow::Result<()> {
    let broker = leader.broker;
    let plugin_task = tokio::spawn(broker.clone().serve(leader.plugin_listener));
    let plugin_v6_task = leader
        .plugin_listener_v6
        .map(|listener| tokio::spawn(broker.clone().serve(listener)));
    let frontend_task = tokio::spawn(broker.clone().serve_frontends(leader.frontend_listener));
    let own_frontend = broker
        .frontend_lease()
        .context("leader frontend lease was unavailable")?;

    let service_result = async {
        let service = McpService::new(BrokerClient::local(broker.clone()));
        let running = service.serve(stdio()).await?;
        running
            .waiting()
            .await
            .context("stdio leader service failed")?;
        anyhow::Result::<()>::Ok(())
    }
    .await;
    drop(own_frontend);

    broker
        .wait_until_idle(Duration::from_secs(IDLE_GRACE_SECS))
        .await;
    broker.shutdown().await;
    plugin_task.await.context("plugin broker task panicked")??;
    if let Some(plugin_v6_task) = plugin_v6_task {
        plugin_v6_task
            .await
            .context("plugin IPv6 broker task panicked")??;
    }
    frontend_task
        .await
        .context("frontend broker task panicked")??;
    service_result
}
