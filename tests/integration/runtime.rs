use figma_dev_mcp_broker::{
    BrokerClient, BrokerConfig, ElectionOutcome, FrontendClient, Limits, elect,
};
use figma_dev_mcp_tools::McpService;
use rmcp::ServiceExt;
use tokio::net::TcpListener;

use super::multi_client::connect_plugin;

#[tokio::test]
async fn leader_stdio_closure_keeps_shared_runtime_alive_for_follower_and_plugin() {
    let plugin_reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_reservation = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let config = BrokerConfig {
        plugin_address: plugin_reservation.local_addr().unwrap(),
        frontend_address: frontend_reservation.local_addr().unwrap(),
        limits: Limits::reduced_for_test(),
    };
    drop(plugin_reservation);
    drop(frontend_reservation);

    let leader = elect(config.clone()).await.unwrap();
    let ElectionOutcome::Leader(leader) = leader else {
        panic!("first runtime must lead")
    };
    let broker = leader.broker.clone();
    let plugin_task = tokio::spawn(broker.clone().serve(leader.plugin_listener));
    let frontend_task = tokio::spawn(broker.clone().serve_frontends(leader.frontend_listener));
    let own_lease = broker.frontend_lease().unwrap();

    let plugin = connect_plugin(
        config.plugin_address,
        "123e4567-e89b-42d3-a456-426614174030",
        "Runtime shared file",
    )
    .await;
    while broker.live_file_count().await != 1 {
        tokio::task::yield_now().await;
    }

    let follower = elect(config).await.unwrap();
    let ElectionOutcome::Follower(stream) = follower else {
        panic!("second runtime must follow")
    };
    let frontend = FrontendClient::from_stream(stream.stream).await.unwrap();

    let (leader_server_io, leader_client_io) = tokio::io::duplex(64 * 1024);
    let leader_broker = broker.clone();
    let leader_service = tokio::spawn(async move {
        McpService::new(BrokerClient::local(leader_broker))
            .serve(leader_server_io)
            .await
            .unwrap()
    });
    let leader_client = ().serve(leader_client_io).await.unwrap();
    let leader_service = leader_service.await.unwrap();

    let (follower_server_io, follower_client_io) = tokio::io::duplex(64 * 1024);
    let follower_service = tokio::spawn(async move {
        McpService::new(BrokerClient::remote(frontend))
            .serve(follower_server_io)
            .await
            .unwrap()
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

    leader_client.cancel().await.unwrap();
    leader_service.waiting().await.unwrap();
    drop(own_lease);
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
        1
    );

    follower_client.cancel().await.unwrap();
    follower_service.waiting().await.unwrap();
    drop(plugin);
    broker.shutdown().await;
    frontend_task.await.unwrap().unwrap();
    plugin_task.await.unwrap().unwrap();
}
