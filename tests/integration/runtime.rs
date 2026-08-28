//! How the process assembles its role.
//!
//! These tests drive `Supervisor` rather than building a leader by hand, because
//! the supervisor is what `figma-dev-mcp`'s `runtime::run` actually composes: an
//! unattached client, an MCP service built over it, and `supervise()` racing the
//! service in one `select!`. `broker_restart.rs` covers `Broker` restart
//! behaviour directly; this file covers the composition around it.

use std::time::Duration;

use figma_dev_mcp_broker::{BrokerConfig, Limits, Supervisor};
use figma_dev_mcp_tools::McpService;
use rmcp::ServiceExt;
use tokio::net::TcpListener;

use super::multi_client::connect_plugin;

async fn free_config() -> BrokerConfig {
    let plugin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = BrokerConfig {
        plugin_address: plugin.local_addr().unwrap(),
        frontend_address: frontend.local_addr().unwrap(),
        limits: Limits::reduced_for_test(),
    };
    drop(plugin);
    drop(frontend);
    config
}

#[tokio::test]
async fn leader_stdio_closure_keeps_shared_runtime_alive_for_follower_and_plugin() {
    let config = free_config().await;

    let leader = Supervisor::start(config.clone()).await;
    assert!(leader.is_leader(), "the first supervisor must lead");
    let broker = leader
        .client()
        .local_broker()
        .expect("a leading supervisor's client resolves to a local Broker");

    let follower = Supervisor::start(config.clone()).await;
    assert!(
        !follower.is_leader(),
        "the second supervisor must follow the first"
    );

    let plugin = connect_plugin(
        config.plugin_address,
        "123e4567-e89b-42d3-a456-426614174030",
        "Runtime shared file",
    )
    .await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    // Each supervisor's client feeds one MCP service, exactly as `runtime::run`
    // wires it.
    let (leader_server_io, leader_client_io) = tokio::io::duplex(64 * 1024);
    let leader_service = tokio::spawn({
        let client = leader.client();
        async move {
            McpService::new(client)
                .serve(leader_server_io)
                .await
                .unwrap()
        }
    });
    let leader_client = ().serve(leader_client_io).await.unwrap();
    let leader_service = leader_service.await.unwrap();

    let (follower_server_io, follower_client_io) = tokio::io::duplex(64 * 1024);
    let follower_service = tokio::spawn({
        let client = follower.client();
        async move {
            McpService::new(client)
                .serve(follower_server_io)
                .await
                .unwrap()
        }
    });
    let follower_client = ().serve(follower_client_io).await.unwrap();
    let follower_service = follower_service.await.unwrap();

    let leader_files = leader_client
        .call_tool(rmcp::model::CallToolRequestParams::new("list_files"))
        .await
        .unwrap();
    assert_eq!(
        leader_files.structured_content.unwrap()["files"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // The leader's own stdio session ends. Under `Supervisor` the role — and its
    // frontend lease — outlives that session; only `shutdown` releases it. The
    // follower and the plugin must be untouched.
    leader_client.cancel().await.unwrap();
    leader_service.waiting().await.unwrap();
    tokio::task::yield_now().await;

    let follower_files = follower_client
        .call_tool(rmcp::model::CallToolRequestParams::new("list_files"))
        .await
        .unwrap();
    assert_eq!(
        follower_files.structured_content.unwrap()["files"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "the follower reaches the plugin through the leader it is still attached to"
    );

    follower_client.cancel().await.unwrap();
    follower_service.waiting().await.unwrap();

    // Tear down in dependency order: the plugin and the follower each hold a
    // lease on the leader's broker, and `shutdown` waits for both.
    drop(plugin);
    drop(follower);
    leader
        .shutdown(Duration::from_millis(0))
        .await
        .expect("the leader shuts down cleanly once nothing is leasing it");
}

#[tokio::test]
async fn the_service_is_up_before_any_election_has_happened() {
    // The composition `runtime::run` relies on: `Supervisor::new` leaves the
    // client unattached so the MCP service can be built and served immediately,
    // and `supervise()` runs the first election as just another iteration of its
    // loop. An eager first election used to sit in front of the service, so an
    // election that could not succeed hung the process before it could answer
    // `initialize`.
    let config = free_config().await;
    let mut supervisor = Supervisor::new(config.clone());
    let client = supervisor.client();

    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    let service = tokio::spawn({
        let client = client.clone();
        async move { McpService::new(client).serve(server_io).await.unwrap() }
    });
    let mcp_client = ().serve(client_io).await.unwrap();
    let service = service.await.unwrap();

    assert!(
        client.local_broker().is_none(),
        "the MCP handshake must complete before any election has run"
    );

    let supervising = tokio::spawn(async move {
        supervisor.supervise().await;
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        while client.local_broker().is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("supervise() must run the first election as its first iteration");

    let plugin = connect_plugin(
        config.plugin_address,
        "123e4567-e89b-42d3-a456-426614174031",
        "Late election file",
    )
    .await;
    let broker = client.local_broker().expect("the elected leader is local");
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let files = mcp_client
        .call_tool(rmcp::model::CallToolRequestParams::new("list_files"))
        .await
        .unwrap();
    assert_eq!(
        files.structured_content.unwrap()["files"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "the same service, built before the election, serves the elected leader"
    );

    mcp_client.cancel().await.unwrap();
    service.waiting().await.unwrap();
    drop(plugin);
    supervising.abort();
}
