//! Leader failover: backend swapping and automatic re-election.

use std::time::Duration;

use figma_dev_mcp_broker::{Broker, BrokerClient, BrokerConfig, Limits};

#[tokio::test]
async fn local_broker_resolves_through_the_swappable_cell() {
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let client = BrokerClient::local(broker.clone());

    assert!(
        client.local_broker().is_some(),
        "a local client must expose its Broker so cancellation can reach the plugin"
    );

    broker.shutdown().await;
}

#[tokio::test]
async fn a_remote_client_has_no_local_broker() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let server = tokio::spawn(broker.clone().serve_frontends(listener));

    let frontend = figma_dev_mcp_broker::FrontendClient::connect(address)
        .await
        .unwrap();
    let client = BrokerClient::remote(frontend);
    assert!(
        client.local_broker().is_none(),
        "a follower has no local Broker to cancel through"
    );

    broker.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(1), server).await;
}

#[tokio::test]
async fn frontend_client_closed_resolves_when_the_leader_goes_away() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let broker = Broker::new(BrokerConfig::for_test(Limits::reduced_for_test()).unwrap());
    let server = tokio::spawn(broker.clone().serve_frontends(listener));

    let frontend = figma_dev_mcp_broker::FrontendClient::connect(address)
        .await
        .unwrap();

    // Still alive: closed() must not resolve.
    assert!(
        tokio::time::timeout(Duration::from_millis(50), frontend.closed())
            .await
            .is_err(),
        "closed() must not resolve while the leader is serving"
    );

    // Kill the leader.
    broker.shutdown().await;
    let _ = tokio::time::timeout(Duration::from_secs(1), server).await;

    tokio::time::timeout(Duration::from_secs(2), frontend.closed())
        .await
        .expect("closed() must resolve once the leader's RPC connection ends");
}

use figma_dev_mcp_broker::Supervisor;

/// Reserve two ports, then release them, so the supervisor can bind them.
async fn free_addresses() -> (std::net::SocketAddr, std::net::SocketAddr) {
    let plugin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addresses = (plugin.local_addr().unwrap(), frontend.local_addr().unwrap());
    drop(plugin);
    drop(frontend);
    addresses
}

fn test_config(
    plugin_address: std::net::SocketAddr,
    frontend_address: std::net::SocketAddr,
) -> BrokerConfig {
    BrokerConfig {
        plugin_address,
        frontend_address,
        limits: Limits::reduced_for_test(),
    }
}

#[tokio::test]
async fn the_first_supervisor_leads_and_the_second_follows() {
    let (plugin_address, frontend_address) = free_addresses().await;
    let config = test_config(plugin_address, frontend_address);

    let leader = Supervisor::start(config.clone()).await.unwrap();
    assert!(leader.is_leader(), "the first process must lead");
    assert!(
        leader.client().local_broker().is_some(),
        "a leader's client must be backed by a local Broker"
    );

    let follower = Supervisor::start(config).await.unwrap();
    assert!(!follower.is_leader(), "the second process must follow");
    assert!(
        follower.client().local_broker().is_none(),
        "a follower's client must be backed by an RPC connection"
    );

    follower.shutdown(Duration::from_millis(0)).await.unwrap();
    leader.shutdown(Duration::from_millis(0)).await.unwrap();
}

#[tokio::test]
async fn a_leading_supervisor_binds_both_ports() {
    let (plugin_address, frontend_address) = free_addresses().await;
    let leader = Supervisor::start(test_config(plugin_address, frontend_address))
        .await
        .unwrap();

    assert!(
        tokio::net::TcpStream::connect(plugin_address).await.is_ok(),
        "the leader must be listening on the plugin port"
    );
    assert!(
        tokio::net::TcpStream::connect(frontend_address).await.is_ok(),
        "the leader must be listening on the frontend port"
    );

    leader.shutdown(Duration::from_millis(0)).await.unwrap();
}
