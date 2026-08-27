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
