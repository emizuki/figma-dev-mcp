use figma_dev_mcp_broker::{BrokerConfig, ElectionOutcome, Limits, elect};
use std::net::SocketAddr;
use tokio::net::TcpListener;

async fn reserve_addresses() -> (SocketAddr, SocketAddr) {
    let plugin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend = TcpListener::bind("127.0.0.1:0").await.unwrap();
    (plugin.local_addr().unwrap(), frontend.local_addr().unwrap())
}

#[tokio::test]
async fn one_process_owns_both_listeners_and_later_processes_follow() {
    let (plugin_address, frontend_address) = reserve_addresses().await;
    let config = BrokerConfig {
        plugin_address,
        frontend_address,
        limits: Limits::reduced_for_test(),
    };

    let leader = elect(config.clone()).await.unwrap();
    assert!(matches!(leader, ElectionOutcome::Leader(_)));

    let follower = elect(config).await.unwrap();
    assert!(matches!(follower, ElectionOutcome::Follower(_)));
}

#[tokio::test]
async fn concurrent_elections_produce_exactly_one_leader() {
    let (plugin_address, frontend_address) = reserve_addresses().await;
    let config = BrokerConfig {
        plugin_address,
        frontend_address,
        limits: Limits::reduced_for_test(),
    };

    let (first, second) = tokio::join!(elect(config.clone()), elect(config));
    let outcomes = [first.unwrap(), second.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ElectionOutcome::Leader(_)))
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(outcome, ElectionOutcome::Follower(_)))
            .count(),
        1
    );
}

#[tokio::test]
async fn plugin_bind_failure_releases_the_frontend_listener() {
    let occupied_plugin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let plugin_address = occupied_plugin.local_addr().unwrap();
    let frontend = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let frontend_address = frontend.local_addr().unwrap();
    drop(frontend);
    let config = BrokerConfig {
        plugin_address,
        frontend_address,
        limits: Limits::reduced_for_test(),
    };

    assert!(elect(config).await.is_err());
    TcpListener::bind(frontend_address)
        .await
        .expect("failed leader election must release the frontend port");
}
